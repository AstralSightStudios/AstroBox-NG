#![cfg(feature = "l2cap")]

use std::sync::Arc;
use std::{fmt, slice, thread};

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use java_spaghetti::{ByteArray, Global, Local, PrimitiveArray};
use tracing::{debug, warn};

use super::bindings::android::bluetooth::{BluetoothDevice, BluetoothSocket};
use super::OptionExt;
use crate::error::ErrorKind;
use crate::{Error, Result};

pub fn open_l2cap_channel(
    device: Global<BluetoothDevice>,
    psm: u16,
    secure: bool,
) -> std::prelude::v1::Result<(L2capChannelReader, L2capChannelWriter), crate::Error> {
    device.vm().with_env(|env| {
        let device = device.as_local(env);

        let channel = if secure {
            device.createL2capChannel(psm as _)?.non_null()?
        } else {
            device.createInsecureL2capChannel(psm as _)?.non_null()?
        };

        channel.connect()?;

        // The L2capCloser closes the l2cap channel when dropped.
        // We put it in an Arc held by both the reader and writer, so it gets dropped
        // when
        let closer = Arc::new(L2capCloser {
            channel: channel.as_global(),
        });

        let (read_sender, read_receiver) = async_channel::bounded::<Vec<u8>>(16);
        let (write_sender, write_receiver) = async_channel::bounded::<Vec<u8>>(16);
        let input_stream = channel.getInputStream()?.non_null()?.as_global();
        let output_stream = channel.getOutputStream()?.non_null()?.as_global();

        // Unfortunately, Android's API for L2CAP channels is only blocking. Only way to deal with it
        // is to launch two background threads with blocking loops for reading and writing, which communicate
        // with the async Rust world via async channels.
        //
        // The loops stop when either Android returns an error (for example if the channel is closed), or the
        // async channel gets closed because the user dropped the reader or writer structs.
        thread::spawn(move || {
            debug!("l2cap read thread running!");

            input_stream.vm().with_env(|env| {
                let stream = input_stream.as_local(env);
                let arr: Local<ByteArray> = ByteArray::new(env, 1024);

                loop {
                    match stream.read_byte_array(&arr) {
                        Ok(n) if n < 0 => {
                            warn!("failed to read from l2cap channel: {}", n);
                            break;
                        }
                        Err(e) => {
                            warn!("failed to read from l2cap channel: {:?}", e);
                            break;
                        }
                        Ok(n) => {
                            let n = n as usize;
                            let mut buf = vec![0u8; n];
                            arr.get_region(0, u8toi8_mut(&mut buf));
                            if let Err(e) = read_sender.send_blocking(buf) {
                                warn!("failed to enqueue received l2cap packet: {:?}", e);
                                break;
                            }
                        }
                    }
                }
            });

            debug!("l2cap read thread exiting!");
        });

        thread::spawn(move || {
            debug!("l2cap write thread running!");

            output_stream.vm().with_env(|env| {
                let stream = output_stream.as_local(env);

                loop {
                    match write_receiver.recv_blocking() {
                        Err(e) => {
                            warn!("failed to dequeue l2cap packet to send: {:?}", e);
                            break;
                        }
                        Ok(packet) => {
                            let b = ByteArray::new_from(env, u8toi8(&packet));
                            if let Err(e) = stream.write_byte_array(b) {
                                warn!("failed to write to l2cap channel: {:?}", e);
                                break;
                            };
                        }
                    }
                }
            });

            debug!("l2cap write thread exiting!");
        });

        Ok((
            L2capChannelReader {
                closer: closer.clone(),
                stream: read_receiver,
            },
            L2capChannelWriter {
                closer,
                stream: write_sender,
            },
        ))
    })
}

/// Utility struct to close the channel on drop.
pub(super) struct L2capCloser {
    channel: Global<BluetoothSocket>,
}

impl L2capCloser {
    fn close(&self) {
        self.channel.vm().with_env(|env| {
            let channel = self.channel.as_local(env);
            match channel.close() {
                Ok(()) => debug!("l2cap channel closed"),
                Err(e) => warn!("failed to close channel: {:?}", e),
            };
        });
    }
}

impl Drop for L2capCloser {
    fn drop(&mut self) {
        self.close()
    }
}

pub struct L2capChannelReader {
    stream: Receiver<Vec<u8>>,
    closer: Arc<L2capCloser>,
}

impl L2capChannelReader {
    #[inline]
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let packet = self
            .stream
            .recv()
            .await
            .map_err(|_| Error::new(ErrorKind::ConnectionFailed, None, "channel is closed"))?;

        if packet.len() > buf.len() {
            return Err(Error::new(ErrorKind::InvalidParameter, None, "Buffer is too small"));
        }

        buf[..packet.len()].copy_from_slice(&packet);

        Ok(packet.len())
    }

    #[inline]
    pub fn try_read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let packet = self.stream.try_recv().map_err(|e| match e {
            TryRecvError::Empty => Error::new(ErrorKind::NotReady, None, "no received packet in queue"),
            TryRecvError::Closed => Error::new(ErrorKind::ConnectionFailed, None, "channel is closed"),
        })?;

        if packet.len() > buf.len() {
            return Err(Error::new(ErrorKind::InvalidParameter, None, "Buffer is too small"));
        }

        buf[..packet.len()].copy_from_slice(&packet);

        Ok(packet.len())
    }

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

pub struct L2capChannelWriter {
    stream: Sender<Vec<u8>>,
    closer: Arc<L2capCloser>,
}

impl L2capChannelWriter {
    pub async fn write(&mut self, packet: &[u8]) -> Result<()> {
        self.stream
            .send(packet.to_vec())
            .await
            .map_err(|_| Error::new(ErrorKind::ConnectionFailed, None, "channel is closed"))
    }

    pub fn try_write(&mut self, packet: &[u8]) -> Result<()> {
        self.stream.try_send(packet.to_vec()).map_err(|e| match e {
            TrySendError::Closed(_) => Error::new(ErrorKind::ConnectionFailed, None, "channel is closed"),
            TrySendError::Full(_) => Error::new(ErrorKind::NotReady, None, "No buffer space for write"),
        })
    }

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

fn u8toi8(slice: &[u8]) -> &[i8] {
    let len = slice.len();
    let data = slice.as_ptr() as *const i8;
    // safety: any bit pattern is valid for u8 and i8, so transmuting them is fine.
    unsafe { slice::from_raw_parts(data, len) }
}

fn u8toi8_mut(slice: &mut [u8]) -> &mut [i8] {
    let len = slice.len();
    let data = slice.as_mut_ptr() as *mut i8;
    // safety: any bit pattern is valid for u8 and i8, so transmuting them is fine.
    unsafe { slice::from_raw_parts_mut(data, len) }
}
