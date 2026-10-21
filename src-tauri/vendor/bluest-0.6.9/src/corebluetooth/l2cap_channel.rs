use core::ptr::NonNull;
use std::fmt;
use std::sync::Arc;

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
use objc2_core_bluetooth::CBL2CAPChannel;
use objc2_foundation::{
    NSDefaultRunLoopMode, NSInputStream, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol,
    NSOutputStream, NSRunLoop, NSStream, NSStreamDelegate, NSStreamEvent, NSString,
};
use tracing::debug;

use super::dispatch::Dispatched;
use crate::error::{Error, ErrorKind};
use crate::Result;

/// Utility struct to close the channel on drop.
pub(super) struct L2capCloser {
    channel: Dispatched<CBL2CAPChannel>,
}

impl L2capCloser {
    fn close(&self) {
        self.channel.dispatch(|channel| unsafe {
            if let Some(c) = channel.inputStream() {
                c.close()
            }
            if let Some(c) = channel.outputStream() {
                c.close()
            }
        })
    }
}

impl Drop for L2capCloser {
    fn drop(&mut self) {
        self.close()
    }
}

/// The reader side of an L2CAP channel.
pub struct L2capChannelReader {
    stream: Receiver<Vec<u8>>,
    closer: Arc<L2capCloser>,
    _delegate: Retained<InputStreamDelegate>,
}

impl L2capChannelReader {
    /// Creates a new L2capChannelReader.
    pub(crate) fn new(channel: Dispatched<CBL2CAPChannel>) -> Self {
        let (sender, receiver) = async_channel::bounded(16);
        let closer = Arc::new(L2capCloser {
            channel: channel.clone(),
        });

        let delegate = channel.dispatch(|channel| unsafe {
            let input_stream = channel.inputStream().unwrap();
            let delegate = InputStreamDelegate::new(sender);
            input_stream.setDelegate(Some(&ProtocolObject::from_retained(delegate.clone())));
            input_stream.scheduleInRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSDefaultRunLoopMode);
            input_stream.open();
            delegate
        });

        Self {
            stream: receiver,
            _delegate: delegate,
            closer,
        }
    }

    /// Reads data from the L2CAP channel into the provided buffer.
    #[inline]
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let packet = self
            .stream
            .recv()
            .await
            .map_err(|_| Error::new(ErrorKind::ConnectionFailed, None, "channel is closed".to_string()))?;

        if packet.len() > buf.len() {
            return Err(Error::new(
                ErrorKind::InvalidParameter,
                None,
                "Buffer is too small".to_string(),
            ));
        }

        buf[..packet.len()].copy_from_slice(&packet);
        Ok(packet.len())
    }

    /// Attempts to read data from the L2CAP channel into the provided buffer without blocking.
    #[inline]
    pub fn try_read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let packet = self.stream.try_recv().map_err(|e| match e {
            TryRecvError::Empty => Error::new(ErrorKind::NotReady, None, "no received packet in queue".to_string()),
            TryRecvError::Closed => Error::new(ErrorKind::ConnectionFailed, None, "channel is closed".to_string()),
        })?;

        if packet.len() > buf.len() {
            return Err(Error::new(
                ErrorKind::InvalidParameter,
                None,
                "Buffer is too small".to_string(),
            ));
        }

        buf[..packet.len()].copy_from_slice(&packet);
        Ok(packet.len())
    }

    /// Closes the L2CAP channel reader.
    pub async fn close(&mut self) -> Result<()> {
        self.closer.close();
        Ok(())
    }
}

impl fmt::Debug for L2capChannelReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("L2capChannelReader")
    }
}

/// The writer side of an L2CAP channel.
pub struct L2capChannelWriter {
    stream: Sender<Vec<u8>>,
    closer: Arc<L2capCloser>,
    _delegate: Retained<OutputStreamDelegate>,
}

impl L2capChannelWriter {
    /// Creates a new L2capChannelWriter.
    pub(crate) fn new(channel: Dispatched<CBL2CAPChannel>) -> Self {
        let (sender, receiver) = async_channel::bounded(16);
        let closer = Arc::new(L2capCloser {
            channel: channel.clone(),
        });

        let delegate = channel.dispatch(|channel| unsafe {
            let output_stream = channel.outputStream().unwrap();
            let delegate = OutputStreamDelegate::new(receiver, Dispatched::retain(&output_stream));
            output_stream.setDelegate(Some(&ProtocolObject::from_retained(delegate.clone())));
            output_stream.scheduleInRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSDefaultRunLoopMode);
            output_stream.open();

            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("ChannelWriteNotification");
            center.addObserver_selector_name_object(&delegate, sel!(onNotified:), Some(&name), None);
            delegate
        });

        Self {
            stream: sender,
            _delegate: delegate,
            closer,
        }
    }

    /// Writes data to the L2CAP channel.
    pub async fn write(&mut self, packet: &[u8]) -> Result<()> {
        self.stream
            .send(packet.to_vec())
            .await
            .map_err(|_| Error::new(ErrorKind::ConnectionFailed, None, "channel is closed".to_string()))?;
        self.notify();
        Ok(())
    }

    /// Attempts to write data to the L2CAP channel without blocking.
    pub fn try_write(&mut self, packet: &[u8]) -> Result<()> {
        self.stream.try_send(packet.to_vec()).map_err(|e| match e {
            TrySendError::Closed(_) => Error::new(ErrorKind::ConnectionFailed, None, "channel is closed".to_string()),
            TrySendError::Full(_) => Error::new(ErrorKind::NotReady, None, "No buffer space for write".to_string()),
        })?;
        self.notify();
        Ok(())
    }

    fn notify(&self) {
        unsafe {
            let name = NSString::from_str("ChannelWriteNotification");
            let center = NSNotificationCenter::defaultCenter();
            center.postNotificationName_object(&name, None);
        }
    }

    /// Closes the L2CAP channel writer.
    pub async fn close(&mut self) -> Result<()> {
        self.closer.close();
        Ok(())
    }
}

impl fmt::Debug for L2capChannelWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("L2capChannelWriter")
    }
}

#[derive(Debug)]
struct InputStreamDelegateIvars {
    sender: Sender<Vec<u8>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = InputStreamDelegateIvars]
    #[derive(Debug, PartialEq, Eq, Hash)]
    struct InputStreamDelegate;

    unsafe impl NSObjectProtocol for InputStreamDelegate {}

    unsafe impl NSStreamDelegate for InputStreamDelegate {
        #[unsafe(method(stream:handleEvent:))]
        fn handle_event(&self, stream: &NSStream, event_code: NSStreamEvent) {
            let mut buf = [0u8; 1024];
            let input_stream = stream.downcast_ref::<NSInputStream>().unwrap();
            if let NSStreamEvent::HasBytesAvailable = event_code {
                let res = unsafe { input_stream.read_maxLength(NonNull::new_unchecked(buf.as_mut_ptr()), buf.len()) };
                if res < 0 {
                    debug!("Read Loop Error: Stream read failed");
                    return;
                }
                let size = res.try_into().unwrap();
                let mut packet = Vec::new();
                packet.extend_from_slice(&buf[..size]);
                if self.ivars().sender.try_send(packet).is_err() {
                    debug!("Read Loop Error: Sender is closed");
                    unsafe {
                        input_stream.setDelegate(None);
                        input_stream.close();
                    }
                }
            }
        }
    }
);

impl InputStreamDelegate {
    pub fn new(sender: Sender<Vec<u8>>) -> Retained<Self> {
        let ivars = InputStreamDelegateIvars { sender };
        let this = InputStreamDelegate::alloc().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

#[derive(Debug)]
struct OutputStreamDelegateIvars {
    receiver: Receiver<Vec<u8>>,
    stream: Dispatched<NSOutputStream>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = OutputStreamDelegateIvars]
    #[derive(Debug, PartialEq, Eq, Hash)]
    struct OutputStreamDelegate;

    unsafe impl NSObjectProtocol for OutputStreamDelegate {}

    unsafe impl NSStreamDelegate for OutputStreamDelegate {
        #[unsafe(method(stream:handleEvent:))]
        fn handle_event(&self, stream: &NSStream, event_code: NSStreamEvent) {
            let output_stream = stream.downcast_ref::<NSOutputStream>().unwrap();
            if let NSStreamEvent::HasSpaceAvailable = event_code {
                if let Ok(mut packet) = self.ivars().receiver.try_recv() {
                    let res = unsafe {
                        output_stream.write_maxLength(NonNull::new_unchecked(packet.as_mut_ptr()), packet.len())
                    };
                    if res < 0 {
                        debug!("Write Loop Error: Stream write failed");
                        unsafe {
                            output_stream.setDelegate(None);
                            output_stream.close();
                            let center = NSNotificationCenter::defaultCenter();
                            center.removeObserver(self);
                        }
                    }
                }
            }
        }

        #[unsafe(method(onNotified:))]
        fn on_notified(&self, _n: &NSNotification) {
            if let Ok(mut packet) = self.ivars().receiver.try_recv() {
                let stream = unsafe { self.ivars().stream.get() };
                let res = unsafe { stream.write_maxLength(NonNull::new_unchecked(packet.as_mut_ptr()), packet.len()) };
                if res < 0 {
                    debug!("Write Loop Error: Stream write failed");
                    unsafe {
                        stream.setDelegate(None);
                        stream.close();
                        let center = NSNotificationCenter::defaultCenter();
                        center.removeObserver(self);
                    }
                }
            }
        }
    }
);

impl OutputStreamDelegate {
    pub fn new(receiver: Receiver<Vec<u8>>, stream: Dispatched<NSOutputStream>) -> Retained<Self> {
        let ivars = OutputStreamDelegateIvars { receiver, stream };
        let this = OutputStreamDelegate::alloc().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}
