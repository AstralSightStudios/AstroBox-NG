// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Streaming custom-scheme responses.
//!
//! Tauri's `register_uri_scheme_protocol` buffers a whole `http::Response`
//! before CEF sees a byte, so a scheme handler cannot stream (SSE, chunked
//! downloads, a response whose length is unknown up front). This module adds a
//! parallel registration whose handler is handed a [`StreamResponder`]: it
//! sends the status + headers once, then writes body chunks on a
//! [`StreamWriter`] that applies real backpressure. CEF pulls those chunks
//! through its `CefResourceHandler::read` contract.
//!
//! The pull side ([`StreamBody`]) is a plain state machine with no CEF types,
//! so it is unit-tested directly; `request_handler.rs` owns the glue that
//! drives it from `read()` and parks/wakes CEF's `ResourceReadCallback`.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};

/// Bounded in-flight chunk budget. Backpressure: a producer blocks on the
/// ninth un-read chunk until CEF drains one. Small on purpose — the renderer
/// is the pace-setter, not this buffer.
const CHANNEL_DEPTH: usize = 8;

/// The trusted origin that initiated a request, as tracked by the browser
/// process — NOT a caller-supplied header. Set by the runtime as an
/// `http::Request` extension so a handler can make same-origin decisions the
/// webview label cannot express. `None` when the initiator is not a
/// non-opaque known frame (e.g. a cross-origin subframe, whose `Origin` the
/// runtime does not repair); a handler MUST treat `None` as "not same-origin"
/// and reject, never as same-origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitiatorOrigin(pub Option<String>);

/// Registered per scheme, process-global, consulted BEFORE tauri's buffered
/// uri-scheme protocols. Runs on a runtime-managed thread, one per request.
/// The `http::Request` carries the request body and, as an extension, the
/// [`InitiatorOrigin`].
pub type StreamingSchemeHandler =
  Box<dyn Fn(&str, http::Request<Vec<u8>>, StreamResponder) + Send + Sync>;

static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<StreamingSchemeHandler>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Arc<StreamingSchemeHandler>>> {
  REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a streaming handler for `scheme` (e.g. `"duck"`). Later
/// registrations for the same scheme replace earlier ones. The scheme must
/// still be declared in `CefConfig::custom_schemes` so Chromium knows it.
pub fn register_streaming_scheme_handler(scheme: &str, handler: StreamingSchemeHandler) {
  registry()
    .lock()
    .expect("streaming registry poisoned")
    .insert(scheme.to_string(), Arc::new(handler));
}

/// The handler registered for `scheme`, if any. Used by the resource handler
/// to decide whether a request streams or falls through to the buffered path.
pub(crate) fn streaming_handler_for(scheme: &str) -> Option<Arc<StreamingSchemeHandler>> {
  registry()
    .lock()
    .expect("streaming registry poisoned")
    .get(scheme)
    .cloned()
}

/// Returned by [`StreamWriter::write`] when the renderer cancelled the request
/// (the pull side was dropped). Stop producing.
#[derive(Debug, PartialEq, Eq)]
pub struct StreamClosed;

type WakeSlot = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

/// Handed to a streaming handler. Send the head exactly once with
/// [`respond`](Self::respond); that unblocks CEF and yields the body writer.
pub struct StreamResponder {
  head_slot: Arc<Mutex<Option<http::Response<()>>>>,
  on_head: Box<dyn FnOnce() + Send>,
  body_tx: SyncSender<Vec<u8>>,
  wake: WakeSlot,
}

impl StreamResponder {
  /// Publish status + headers and return the body writer. CEF reports the
  /// body length as unknown; the body ends when the writer is dropped or
  /// [`finish`](StreamWriter::finish)ed.
  pub fn respond(self, head: http::Response<()>) -> StreamWriter {
    *self.head_slot.lock().expect("head slot poisoned") = Some(head);
    (self.on_head)();
    StreamWriter {
      body_tx: self.body_tx,
      wake: self.wake,
    }
  }
}

/// The producer side of a streaming body. Each [`write`](Self::write) is one
/// chunk; the call blocks when the bounded in-flight budget is full.
pub struct StreamWriter {
  body_tx: SyncSender<Vec<u8>>,
  wake: WakeSlot,
}

impl StreamWriter {
  /// Enqueue one body chunk, blocking under backpressure. `Err(StreamClosed)`
  /// means the renderer cancelled — stop producing. Empty chunks are dropped
  /// (they would look like EOF to a naive reader).
  pub fn write(&mut self, chunk: Vec<u8>) -> Result<(), StreamClosed> {
    if chunk.is_empty() {
      return Ok(());
    }
    self.body_tx.send(chunk).map_err(|_| StreamClosed)?;
    self.take_wake();
    Ok(())
  }

  /// End the body. Equivalent to dropping the writer; explicit for clarity.
  pub fn finish(self) {}

  fn take_wake(&self) {
    // Release the wake-slot lock BEFORE running the waker: the waker
    // (request_handler.rs's read glue) re-locks the shared StreamBody to pull
    // the now-available chunk, and the CEF read thread parks under the same
    // pair of locks in the opposite order. Invoking the waker while still
    // holding the wake slot would close that lock cycle into a deadlock.
    let wake = self.wake.lock().expect("wake slot poisoned").take();
    if let Some(wake) = wake {
      wake();
    }
  }
}

impl Drop for StreamWriter {
  fn drop(&mut self) {
    // Dropping body_tx disconnects the channel so the reader observes EOF;
    // wake it in case it is parked on an empty-but-open stream.
    self.take_wake();
  }
}

/// Outcome of one [`StreamBody::read`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadOutcome {
  /// `n` bytes copied into the output buffer.
  Copied(usize),
  /// No bytes available yet; the wake closure was parked and fires on the
  /// next write. The CEF caller returns "keep reading" with 0 bytes.
  Pending,
  /// Producer finished and the buffer is drained. EOF.
  Done,
}

/// The pull side CEF reads through. Holds a leftover cursor for a chunk that
/// did not fit the last output buffer.
pub(crate) struct StreamBody {
  body_rx: Receiver<Vec<u8>>,
  leftover: Vec<u8>,
  cursor: usize,
  wake: WakeSlot,
}

impl StreamBody {
  /// Copy as much as fits into `out`. When nothing is buffered, park `wake`
  /// (fired by the next write) and return [`ReadOutcome::Pending`]; once the
  /// producer is gone and nothing is buffered, return [`ReadOutcome::Done`].
  pub(crate) fn read(
    &mut self,
    out: &mut [u8],
    wake: impl FnOnce() + Send + 'static,
  ) -> ReadOutcome {
    if out.is_empty() {
      return ReadOutcome::Pending;
    }
    if self.cursor >= self.leftover.len() {
      match self.body_rx.try_recv() {
        Ok(chunk) => {
          self.leftover = chunk;
          self.cursor = 0;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
          // Park, then re-check once: a write between the failed try_recv and
          // storing the waker would otherwise be a lost wakeup.
          *self.wake.lock().expect("wake slot poisoned") = Some(Box::new(wake));
          match self.body_rx.try_recv() {
            Ok(chunk) => {
              self.wake.lock().expect("wake slot poisoned").take();
              self.leftover = chunk;
              self.cursor = 0;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return ReadOutcome::Pending,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
              self.wake.lock().expect("wake slot poisoned").take();
              return ReadOutcome::Done;
            }
          }
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => return ReadOutcome::Done,
      }
    }
    let n = (self.leftover.len() - self.cursor).min(out.len());
    out[..n].copy_from_slice(&self.leftover[self.cursor..self.cursor + n]);
    self.cursor += n;
    ReadOutcome::Copied(n)
  }
}

/// Build a responder/body pair sharing one bounded channel and wake slot. The
/// resource handler stores `head_slot`/`StreamBody`, spawns the handler with
/// the `StreamResponder`, and wires `on_head` to `callback.cont()`.
pub(crate) fn make_stream(
  on_head: Box<dyn FnOnce() + Send>,
) -> (
  StreamResponder,
  Arc<Mutex<Option<http::Response<()>>>>,
  StreamBody,
) {
  let (body_tx, body_rx) = sync_channel::<Vec<u8>>(CHANNEL_DEPTH);
  let head_slot = Arc::new(Mutex::new(None));
  let wake: WakeSlot = Arc::new(Mutex::new(None));
  let responder = StreamResponder {
    head_slot: head_slot.clone(),
    on_head,
    body_tx,
    wake: wake.clone(),
  };
  let body = StreamBody {
    body_rx,
    leftover: Vec::new(),
    cursor: 0,
    wake,
  };
  (responder, head_slot, body)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::thread;
  use std::time::Duration;

  fn noop_wake() {}

  #[test]
  fn drains_two_chunks_across_small_buffers_then_done() {
    let (responder, _head, mut body) = make_stream(Box::new(|| {}));
    let mut writer = responder.respond(http::Response::new(()));
    writer.write(b"hello".to_vec()).unwrap();
    writer.write(b"world".to_vec()).unwrap();
    writer.finish();

    let mut out = [0u8; 3];
    let mut got = Vec::new();
    loop {
      match body.read(&mut out, noop_wake) {
        ReadOutcome::Copied(n) => got.extend_from_slice(&out[..n]),
        ReadOutcome::Done => break,
        ReadOutcome::Pending => panic!("producer already finished, must not park"),
      }
    }
    assert_eq!(got, b"helloworld");
  }

  #[test]
  fn read_on_empty_open_stream_parks_and_a_write_wakes_it() {
    let (responder, _head, mut body) = make_stream(Box::new(|| {}));
    let mut writer = responder.respond(http::Response::new(()));

    let woke = Arc::new(AtomicUsize::new(0));
    let w = woke.clone();
    let mut out = [0u8; 8];
    // Nothing written yet: must park and register the waker.
    assert_eq!(
      body.read(&mut out, move || {
        w.fetch_add(1, Ordering::SeqCst);
      }),
      ReadOutcome::Pending
    );
    assert_eq!(woke.load(Ordering::SeqCst), 0);

    // The write must fire the parked waker exactly once.
    writer.write(b"data".to_vec()).unwrap();
    assert_eq!(woke.load(Ordering::SeqCst), 1);

    match body.read(&mut out, noop_wake) {
      ReadOutcome::Copied(n) => assert_eq!(&out[..n], b"data"),
      other => panic!("expected Copied, got {other:?}"),
    }
  }

  #[test]
  fn write_after_reader_dropped_returns_stream_closed() {
    let (responder, _head, body) = make_stream(Box::new(|| {}));
    let mut writer = responder.respond(http::Response::new(()));
    drop(body);
    assert_eq!(writer.write(b"x".to_vec()), Err(StreamClosed));
  }

  #[test]
  fn bounded_channel_blocks_producer_until_reader_drains() {
    let (responder, _head, mut body) = make_stream(Box::new(|| {}));
    let mut writer = responder.respond(http::Response::new(()));

    let progressed = Arc::new(AtomicUsize::new(0));
    let p = progressed.clone();
    // CHANNEL_DEPTH accepted immediately, then two more must block.
    let producer = thread::spawn(move || {
      for i in 0..(CHANNEL_DEPTH + 2) {
        writer.write(vec![i as u8]).unwrap();
        p.fetch_add(1, Ordering::SeqCst);
      }
    });

    // Give the producer time to fill the buffer and block.
    thread::sleep(Duration::from_millis(50));
    let filled = progressed.load(Ordering::SeqCst);
    assert!(
      filled <= CHANNEL_DEPTH + 1,
      "producer should have blocked around the channel bound, got {filled}"
    );

    // Drain everything; the producer then completes.
    let mut out = [0u8; 1];
    let mut count = 0;
    loop {
      match body.read(&mut out, noop_wake) {
        ReadOutcome::Copied(_) => count += 1,
        ReadOutcome::Done => break,
        ReadOutcome::Pending => thread::sleep(Duration::from_millis(1)),
      }
    }
    producer.join().unwrap();
    assert_eq!(count, CHANNEL_DEPTH + 2);
    assert_eq!(progressed.load(Ordering::SeqCst), CHANNEL_DEPTH + 2);
  }

  #[test]
  fn empty_chunks_are_dropped_not_treated_as_eof() {
    let (responder, _head, mut body) = make_stream(Box::new(|| {}));
    let mut writer = responder.respond(http::Response::new(()));
    writer.write(Vec::new()).unwrap();
    writer.write(b"real".to_vec()).unwrap();
    writer.finish();

    let mut out = [0u8; 8];
    match body.read(&mut out, noop_wake) {
      ReadOutcome::Copied(n) => assert_eq!(&out[..n], b"real"),
      other => panic!("empty write must not end the stream, got {other:?}"),
    }
  }

  #[test]
  fn respond_publishes_head_and_fires_on_head_once() {
    let fired = Arc::new(AtomicUsize::new(0));
    let f = fired.clone();
    let (responder, head_slot, _body) = make_stream(Box::new(move || {
      f.fetch_add(1, Ordering::SeqCst);
    }));
    let resp = http::Response::builder()
      .status(201)
      .header("content-type", "text/event-stream")
      .body(())
      .unwrap();
    let _writer = responder.respond(resp);
    assert_eq!(fired.load(Ordering::SeqCst), 1);
    let stored = head_slot.lock().unwrap();
    let stored = stored.as_ref().expect("head published");
    assert_eq!(stored.status(), 201);
    assert_eq!(stored.headers()["content-type"], "text/event-stream");
  }
}
