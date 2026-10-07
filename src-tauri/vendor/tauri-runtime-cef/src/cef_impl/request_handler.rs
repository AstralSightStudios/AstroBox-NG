// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  borrow::Cow,
  io::{Cursor, Read},
  sync::{Arc, Mutex},
};

use cef::{rc::*, *};
use dioxus_debug_cell::RefCell;
use html5ever::{LocalName, interface::QualName, namespace_url, ns};
use http::{
  HeaderMap, HeaderName, HeaderValue,
  header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, ORIGIN},
};
use kuchiki::NodeRef;
use tauri_runtime::{UserEvent, window::WindowId};

use crate::compat::{NavigationHandler, UriSchemeProtocolHandler};
use tauri_utils::{
  config::{Csp, CspDirectiveSources},
  html::{parse as parse_html, serialize_node},
};
use url::Url;

use crate::{
  cef_impl::client::{DragDropEventTarget, DragDropState, WebDragDropResourceRequestHandler},
  runtime::RuntimeContext,
  streaming::{self, InitiatorOrigin, ReadOutcome, StreamBody},
  webview::{CefInitScript, INITIAL_LOAD_URL},
};

type HttpResponse = Arc<RefCell<Option<http::Response<Cursor<Vec<u8>>>>>>;

/// The pull side of a streaming custom-scheme response, installed by
/// `process_request` when the request's scheme has a streaming handler. `head`
/// is the shared slot the handler's `StreamResponder` publishes status +
/// headers into; `body` is drained by `read()`. Wrapped in `Arc<Mutex>` (not
/// the buffered path's `RefCell`) because the producer thread's wake closure
/// re-enters `body` to deliver chunks asynchronously.
type StreamCell = Arc<Mutex<Option<StreamState>>>;

struct StreamState {
  head: Arc<Mutex<Option<http::Response<()>>>>,
  body: StreamBody,
}
pub(crate) type SchemeRegistry = Arc<
  Mutex<
    std::collections::HashMap<
      (i32, String),
      (
        String,
        Arc<Box<UriSchemeProtocolHandler>>,
        Arc<Vec<CefInitScript>>,
      ),
    >,
  >,
>;

fn csp_inject_initialization_scripts_hashes(
  existing_csp: String,
  initialization_scripts: &[CefInitScript],
) -> String {
  if initialization_scripts.is_empty() {
    return existing_csp;
  }

  let script_hashes: Vec<String> = initialization_scripts
    .iter()
    .map(|s| s.hash.clone())
    .collect();

  if script_hashes.is_empty() {
    return existing_csp;
  }

  let mut csp_map: std::collections::HashMap<String, CspDirectiveSources> =
    Csp::Policy(existing_csp.to_string()).into();

  let script_src = csp_map
    .entry("script-src".to_string())
    .or_insert_with(|| CspDirectiveSources::List(vec!["'self'".to_string()]));

  script_src.extend(script_hashes);

  Csp::DirectiveMap(csp_map).to_string()
}

fn inject_scripts_into_html_body(
  body: &[u8],
  initialization_scripts: &[CefInitScript],
) -> Option<Vec<u8>> {
  let Ok(body_str) = std::str::from_utf8(body) else {
    return None;
  };

  let document = parse_html(body_str.to_string());

  let head = if let Ok(ref head_node) = document.select_first("head") {
    head_node.as_node().clone()
  } else {
    let head_node = NodeRef::new_element(
      QualName::new(None, ns!(html), LocalName::from("head")),
      None,
    );
    document.prepend(head_node.clone());
    head_node
  };

  for init_script in initialization_scripts.iter().rev() {
    let script_el = NodeRef::new_element(QualName::new(None, ns!(html), "script".into()), None);
    script_el.append(NodeRef::new_text(init_script.script.as_str()));
    head.prepend(script_el);
  }

  Some(serialize_node(&document))
}

wrap_request_handler! {
  pub struct WebRequestHandler<T: UserEvent> {
    navigation_handler: Option<Arc<NavigationHandler>>,
    context: RuntimeContext<T>,
    window_id: WindowId,
    webview_id: u32,
    drag_drop_event_target: DragDropEventTarget,
    drag_drop_handler_enabled: bool,
    drag_drop_state: Arc<Mutex<DragDropState>>,
    web_content_process_terminate_handler: Option<Arc<dyn Fn() + Send>>,
  }

  impl RequestHandler {
    fn on_render_process_terminated(
      &self,
      _browser: Option<&mut Browser>,
      _status: TerminationStatus,
      _error_code: ::std::os::raw::c_int,
      _error_string: Option<&CefString>,
    ) {
      if let Some(handler) = &self.web_content_process_terminate_handler {
        handler();
      }
    }

    fn on_before_browse(
      &self,
      _browser: Option<&mut Browser>,
      frame: Option<&mut Frame>,
      request: Option<&mut Request>,
      _user_gesture: ::std::os::raw::c_int,
      _is_redirect: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
      let _ = (&self.context, self.window_id, self.webview_id);

      let Some(frame) = frame else {
        return 0;
      };
      // we only fire main frame navigation events to match the behavior of the wry runtime
      if frame.is_main() == 0 {
        return 0;
      }
      let Some(request) = request else {
        return 0;
      };

      let url_str = CefString::from(&request.url()).to_string();

      if url_str == INITIAL_LOAD_URL {
        return 0;
      }

      let Ok(url) = url::Url::parse(&url_str) else {
        return 0;
      };

      let Some(handler) = &self.navigation_handler else {
        return 0;
      };

      let should_navigate = handler(&url);
      if should_navigate { 0 } else { 1 }
    }

    fn resource_request_handler(
      &self,
      _browser: Option<&mut Browser>,
      _frame: Option<&mut Frame>,
      _request: Option<&mut Request>,
      _is_navigation: ::std::os::raw::c_int,
      _is_download: ::std::os::raw::c_int,
      _request_initiator: Option<&CefString>,
      _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
    ) -> Option<ResourceRequestHandler> {
      // The handler only intercepts the drag-drop bridge requests; when the
      // bridge is disabled it would pass every request straight through, so skip
      // building (and cloning the context + state into) a handler that CEF calls
      // for every subresource/fetch/XHR the page makes.
      if !self.drag_drop_handler_enabled {
        return None;
      }

      Some(WebDragDropResourceRequestHandler::new(
        self.context.clone(),
        self.window_id,
        self.webview_id,
        self.drag_drop_event_target,
        self.drag_drop_handler_enabled,
        self.drag_drop_state.clone(),
      ))
    }
  }
}

/// Copy an `http` head onto CEF's `Response`, set `Cache-Control: no-store`,
/// derive the MIME from `Content-Type`, and mark the body length unknown
/// (`-1`). Shared by the buffered and streaming `response_headers` paths, which
/// differ only in the head's body type.
fn write_response_headers<B>(
  cef_response: &mut Response,
  head: &http::Response<B>,
  response_length: Option<&mut i64>,
  redirect_url: Option<&mut CefString>,
) {
  cef_response.set_status(head.status().as_u16() as i32);
  let mut content_type = None;

  // Apply via a multimap so REPEATED header names survive — `Set-Cookie` is
  // the common one, and a page that sets two cookies in a single response must
  // keep both. `set_header_by_name(.., overwrite=0)` per value silently drops
  // the second (it only sets when the name is absent), so build the whole map
  // and set it once. `http::HeaderMap`'s iterator yields (name, value) for
  // every value, so duplicates come through naturally.
  let mut map = CefStringMultimap::new();
  for (name, value) in head.headers() {
    let Ok(value) = value.to_str() else {
      continue;
    };
    map.append(name.as_str(), value);
    if name == CONTENT_TYPE {
      content_type.replace(value.to_string());
    }
  }
  cef_response.set_header_map(Some(&mut map));

  cef_response.set_header_by_name(Some(&"Cache-Control".into()), Some(&"no-store".into()), 1);

  let mime_type = content_type
    .as_ref()
    .and_then(|t| t.split(';').next())
    .map(str::trim)
    .unwrap_or("text/plain");
  cef_response.set_mime_type(Some(&mime_type.into()));

  if let Some(length) = response_length {
    *length = -1;
  }

  if let Some(redirect_url) = redirect_url {
    let _ = std::mem::take(redirect_url);
  }
}

wrap_resource_handler! {
  pub struct WebResourceHandler {
    webview_label: String,
    handler: Arc<Box<UriSchemeProtocolHandler>>,
    initialization_scripts: Arc<Vec<CefInitScript>>,
    // Serialized origin of the main frame that initiated this request, captured
    // browser-side in the scheme handler factory. The renderer can issue an IPC
    // request before its execution context is fully wired to the loader; in
    // that window Chromium tags the request with `Origin: null` even though the
    // document already has a proper origin. We use this to repair the `Origin`
    // header in that case. `None` when the initiator is not the (non-opaque)
    // main frame, so sandboxed/subframe `Origin: null` requests are left as-is.
    initiator_origin: Option<String>,
    // we clone response to send it to the handler thread
    response: HttpResponse,
    // Set only when this request's scheme has a registered streaming handler;
    // `response` stays empty in that case and the two paths never mix.
    stream: StreamCell,
  }

  impl ResourceHandler {
    fn process_request(
      &self,
      request: Option<&mut Request>,
      callback: Option<&mut Callback>,
    ) -> ::std::os::raw::c_int {
      let Some(request) = request else { return 0 };
      let Some(callback) = callback else { return 0 };

      let url = CefString::from(&request.url()).to_string();
      let url = Url::parse(&url).ok();

      let Some(url) = url else { return 0 };
      let scheme = url.scheme().to_string();

      // Extraction shared by both paths — reads `request` before it is dropped.
      let label = self.webview_label.clone();
      let data = read_request_body(request);
      let mut headers = get_request_headers(request);

      // The renderer can issue an IPC request before its execution context is
      // fully wired to the loader; in that window Chromium sends the request
      // with `Origin: null` even though the document already has a real
      // origin. Repair that from the initiating main frame's URL, which the
      // browser process tracks reliably.
      //
      // ONLY a literal `null` is repaired — an ABSENT `Origin` is left absent.
      // Absence is meaningful: a top-level navigation and a same-origin GET
      // carry no `Origin` by design, and inventing one turns a navigation into
      // what looks like a cross-origin call from the *previous* page — which a
      // server's origin check then rightly refuses. A correct renderer-sent
      // origin always wins.
      if let Some(initiator_origin) = &self.initiator_origin
        && headers
          .get(ORIGIN)
          .is_some_and(|value| value.as_bytes() == b"null")
        && let Ok(value) = HeaderValue::from_str(initiator_origin)
      {
        headers.insert(ORIGIN, value);
      }

      let method_str = CefString::from(&request.method()).to_string();
      let method = http::Method::from_bytes(method_str.as_bytes()).unwrap_or(http::Method::GET);

      // Streaming path: the scheme registered a streaming handler. The handler
      // publishes the head (which fires `callback.cont()`), then writes body
      // chunks that `read()` drains. Init-script HTML injection is intentionally
      // skipped — streaming bodies are never buffered or parsed.
      if let Some(stream_handler) = streaming::streaming_handler_for(&scheme) {
        let on_head = ThreadSafe(callback.clone());
        let (responder, head, body) = streaming::make_stream(Box::new(move || {
          on_head.into_owned().cont();
        }));
        *self.stream.lock().expect("stream slot poisoned") = Some(StreamState { head, body });

        let initiator = InitiatorOrigin(self.initiator_origin.clone());
        std::thread::spawn(move || {
          let mut http_request = http::Request::builder()
            .method(method)
            .uri(url.as_str())
            .body(data)
            .unwrap();
          *http_request.headers_mut() = headers;
          http_request.extensions_mut().insert(initiator);
          stream_handler(&label, http_request, responder);
        });
        return 1;
      }

      // Buffered path: tauri's uri-scheme protocol handler produces one whole
      // response, which `read()` streams out of a `Cursor`.
      let callback = ThreadSafe(callback.clone());
      let response_store = ThreadSafe(self.response.clone());
      let initialization_scripts = self.initialization_scripts.clone();
      let responder = Box::new(move |response: http::Response<Cow<'static, [u8]>>| {
        let is_html = response
          .headers()
          .get(CONTENT_TYPE)
          .and_then(|ct| ct.to_str().ok())
          .map(|ct| ct.to_lowercase().starts_with("text/html"))
          .unwrap_or(false);

        let (parts, body) = response.into_parts();
        let body_bytes = body.into_owned();
        let body_bytes = if is_html {
          inject_scripts_into_html_body(&body_bytes, &initialization_scripts).unwrap_or(body_bytes)
        } else {
          body_bytes
        };

        let mut response = http::Response::from_parts(parts, Cursor::new(body_bytes));

        if let Some(csp) = response.headers_mut().get_mut(CONTENT_SECURITY_POLICY) {
          let csp_string = csp.to_str().unwrap_or_default().to_string();
          let new_csp =
            csp_inject_initialization_scripts_hashes(csp_string, &initialization_scripts);
          if let Ok(new_csp) = HeaderValue::from_str(&new_csp) {
            *csp = new_csp;
          }
        }

        response_store.into_owned().borrow_mut().replace(response);

        let callback = callback.into_owned();
        callback.cont();
      });

      let handler = self.handler.clone();
      std::thread::spawn(move || {
        let mut http_request = http::Request::builder()
          .method(method)
          .uri(url.as_str())
          .body(data)
          .unwrap();
        *http_request.headers_mut() = headers;
        // handler is Arc<Box<UriSchemeProtocol>>, so we need to dereference to call it
        (**handler)(&label, http_request, responder);
      });
      1
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn read(
      &self,
      data_out: *mut u8,
      bytes_to_read: ::std::os::raw::c_int,
      bytes_read: Option<&mut ::std::os::raw::c_int>,
      callback: Option<&mut ResourceReadCallback>,
    ) -> ::std::os::raw::c_int {
      let Ok(bytes_to_read) = usize::try_from(bytes_to_read) else {
        return 0;
      };

      // Streaming path: drive the StreamBody. When a chunk is buffered we copy
      // it synchronously; when the producer has not written yet we retain
      // `data_out` + `callback`, park a wake, and return continue-with-0 — CEF's
      // async read contract. The wake (fired by the next `StreamWriter::write`
      // or writer drop, on the producer thread) copies the chunk into the
      // retained buffer and calls `callback.cont(n)`; `cont(0)` signals EOF.
      {
        let mut guard = self.stream.lock().expect("stream slot poisoned");
        if let Some(state) = guard.as_mut() {
          if bytes_to_read == 0 {
            if let Some(bytes_read) = bytes_read {
              *bytes_read = 0;
            }
            return 1;
          }
          let out = unsafe { std::slice::from_raw_parts_mut(data_out, bytes_to_read) };
          let stream = self.stream.clone();
          let callback = callback.map(|callback| ThreadSafe(callback.clone()));
          let retained = ThreadSafe((data_out, bytes_to_read));
          let wake = move || {
            let (ptr, len) = retained.into_owned();
            let out = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
            let mut guard = stream.lock().expect("stream slot poisoned");
            let count = match guard.as_mut() {
              // The wake only fires after a real write or the writer's drop, so
              // `Pending` cannot occur here; treat it (and `Done`) as EOF.
              Some(state) => match state.body.read(out, || {}) {
                ReadOutcome::Copied(count) => count as ::std::os::raw::c_int,
                ReadOutcome::Pending | ReadOutcome::Done => 0,
              },
              None => 0,
            };
            if let Some(callback) = callback {
              callback.into_owned().cont(count);
            }
          };
          return match state.body.read(out, wake) {
            ReadOutcome::Copied(count) => {
              if let Some(bytes_read) = bytes_read {
                *bytes_read = count as ::std::os::raw::c_int;
              }
              1
            }
            ReadOutcome::Pending => {
              if let Some(bytes_read) = bytes_read {
                *bytes_read = 0;
              }
              1
            }
            ReadOutcome::Done => {
              if let Some(bytes_read) = bytes_read {
                *bytes_read = 0;
              }
              0
            }
          };
        }
      }

      // Buffered path: copy out of the response `Cursor`.
      let data_out = unsafe { std::slice::from_raw_parts_mut(data_out, bytes_to_read) };
      let count = self
        .response
        .borrow_mut()
        .as_mut()
        .and_then(|response| response.body_mut().read(data_out).ok())
        .unwrap_or(0);
      if let Some(bytes_read) = bytes_read {
        let Ok(count) = count.try_into() else {
          return 0;
        };
        *bytes_read = count;
        if count > 0 {
          return 1;
        }
      }
      0
    }

    fn response_headers(
      &self,
      response: Option<&mut Response>,
      response_length: Option<&mut i64>,
      redirect_url: Option<&mut CefString>,
    ) {
      let Some(response) = response else {
        return;
      };

      // Streaming path: publish the head the `StreamResponder` stored. By the
      // time CEF calls this, the handler has already fired `callback.cont()`, so
      // the head slot is populated.
      {
        let guard = self.stream.lock().expect("stream slot poisoned");
        if let Some(state) = guard.as_ref() {
          if let Some(head) = state.head.lock().expect("stream head poisoned").as_ref() {
            write_response_headers(response, head, response_length, redirect_url);
          }
          return;
        }
      }

      // Buffered path.
      let Some(response_data) = &*self.response.borrow() else {
        return;
      };
      write_response_headers(response, response_data, response_length, redirect_url);
    }
  }
}

wrap_scheme_handler_factory! {
  pub struct UriSchemeHandlerFactory {
    registry: SchemeRegistry,
    scheme: String,
  }

  impl SchemeHandlerFactory {
    fn create(
      &self,
      browser: Option<&mut Browser>,
      frame: Option<&mut Frame>,
      _scheme_name: Option<&CefString>,
      _request: Option<&mut Request>,
    ) -> Option<ResourceHandler> {
      let browser = browser?;
      let id = browser.identifier();

      // get handler from our regsitry based on browser ID and scheme
      let (webview_label, handler, initialization_scripts) = self
        .registry
        .lock()
        .unwrap()
        .get(&(id, self.scheme.clone()))
        .cloned()?;

      // Capture the initiating main frame's origin so `process_request` can
      // repair a racy `Origin: null` header. Restricted to the main frame: it
      // is never an opaque-origin (sandboxed) document in a Tauri webview, so
      // upgrading its origin is safe; subframes are intentionally left alone.
      let initiator_origin = frame
        .filter(|frame| frame.is_main() == 1)
        .map(|frame| CefString::from(&frame.url()).to_string())
        .and_then(|url| Url::parse(&url).ok())
        .and_then(|url| tuple_origin(&url));

      Some(WebResourceHandler::new(
        webview_label,
        handler,
        initialization_scripts,
        initiator_origin,
        Arc::new(RefCell::new(None)),
        Arc::new(Mutex::new(None)),
      ))
    }
  }
}

pub(crate) struct ThreadSafe<T>(pub(crate) T);

impl<T> ThreadSafe<T> {
  pub(crate) fn into_owned(self) -> T {
    self.0
  }
}

unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}

fn read_request_body(request: &mut Request) -> Vec<u8> {
  let mut body = Vec::new();

  if let Some(post_data) = request.post_data() {
    let mut elements = vec![None; post_data.element_count()];
    post_data.elements(Some(&mut elements));
    for element in elements.into_iter().flatten() {
      match element.get_type().as_ref() {
        sys::cef_postdataelement_type_t::PDE_TYPE_BYTES => {
          let size = element.bytes_count();
          if size > 0 {
            let mut buf = vec![0u8; size];
            // Copy bytes into our buffer
            let copied = element.bytes(size, buf.as_mut_ptr());
            // Safety: CEF promises it wrote `copied` bytes into buf
            unsafe {
              buf.set_len(copied);
            }
            body.extend(buf);
          }
        }
        sys::cef_postdataelement_type_t::PDE_TYPE_FILE => {
          // Read file from disk
          let file_path = CefString::from(&element.file()).to_string();
          if let Ok(mut file) = std::fs::File::open(&file_path) {
            use std::io::Read;
            let mut buf = Vec::new();
            if file.read_to_end(&mut buf).is_ok() {
              body.extend(buf);
            }
          }
        }
        _ => {}
      }
    }
  }

  body
}

/// The tuple origin of `url`, as **Chromium** serializes it.
///
/// `Url::origin()` implements the URL spec, where only *special* schemes
/// (http, https, ws, wss, ftp, file) get a tuple origin and everything else is
/// opaque — serialized `"null"`. But a custom scheme registered with
/// `CEF_SCHEME_OPTION_STANDARD` (which is every scheme in
/// `CefConfig::custom_schemes`, see `register_tauri_schemes`) DOES get a real
/// `scheme://host[:port]` tuple origin inside Chromium, and that is the origin
/// the renderer actually enforces (CSP, CORS) — and the one a scheme handler
/// must compare against. Composing it by hand for the opaque case is what
/// makes `InitiatorOrigin` usable on custom schemes at all; without this it is
/// always `None` there, silently disabling both the `Origin: null` repair in
/// `process_request` and any same-origin check a handler builds on it.
fn tuple_origin(url: &Url) -> Option<String> {
  let spec_origin = url.origin().ascii_serialization();
  if spec_origin != "null" {
    return Some(spec_origin);
  }
  let host = url.host_str()?;
  Some(match url.port() {
    Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
    None => format!("{}://{}", url.scheme(), host),
  })
}

#[cfg(test)]
mod tests {
  use super::tuple_origin;
  use url::Url;

  #[test]
  fn tuple_origin_covers_special_and_custom_schemes() {
    let special = Url::parse("https://example.com:8443/x?y").unwrap();
    assert_eq!(
      tuple_origin(&special).as_deref(),
      Some("https://example.com:8443")
    );
    // A standard-registered custom scheme: the URL spec calls this opaque, but
    // Chromium gives it a tuple origin, so we must too.
    let custom = Url::parse("duck://site.alice.duck/index.html").unwrap();
    assert_eq!(
      tuple_origin(&custom).as_deref(),
      Some("duck://site.alice.duck")
    );
    // Genuinely origin-less: nothing to compare against, so no origin.
    let opaque = Url::parse("data:text/html,hi").unwrap();
    assert_eq!(tuple_origin(&opaque), None);
  }
}

fn get_request_headers(request: &mut Request) -> HeaderMap {
  let mut headers = HeaderMap::new();

  let mut map = CefStringMultimap::new();

  request.header_map(Some(&mut map));

  // Iterate through all entries
  for (name, value) in map {
    for v in value {
      headers.append(
        HeaderName::from_bytes(name.as_bytes()).unwrap(),
        HeaderValue::from_str(&v).unwrap(),
      );
    }
  }

  headers
}
