#![cfg(feature = "l2cap")]

use crate::{sys, Result};

/// A Bluetooth LE L2CAP Connection-oriented Channel (CoC)
#[derive(Debug)]
pub struct L2capChannel {
    pub(crate) reader: sys::l2cap_channel::L2capChannelReader,
    pub(crate) writer: sys::l2cap_channel::L2capChannelWriter,
}

/// Reader half of a L2CAP Connection-oriented Channel (CoC)
#[derive(Debug)]
pub struct L2capChannelReader {
    reader: sys::l2cap_channel::L2capChannelReader,
}

/// Writerhalf of a L2CAP Connection-oriented Channel (CoC)
#[derive(Debug)]
pub struct L2capChannelWriter {
    writer: sys::l2cap_channel::L2capChannelWriter,
}

impl L2capChannel {
    /// Read a packet from the L2CAP channel.
    ///
    /// The packet is written to the start of `buf`, and the packet length is returned.
    #[inline]
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.reader.read(buf).await
    }

    /// Write a packet to the L2CAP channel.
    #[inline]
    pub async fn write(&mut self, packet: &[u8]) -> Result<()> {
        self.writer.write(packet).await
    }

    /// Close the L2CAP channel.
    ///
    /// This closes the entire channel, in both directions (reading and writing).
    ///
    /// The channel is automatically closed when `L2capChannel` is dropped, so
    /// you don't need to call this explicitly.
    #[inline]
    pub async fn close(&mut self) -> Result<()> {
        self.writer.close().await
    }

    /// Split the channel into read and write halves.
    #[inline]
    pub fn split(self) -> (L2capChannelReader, L2capChannelWriter) {
        let Self { reader, writer } = self;
        (L2capChannelReader { reader }, L2capChannelWriter { writer })
    }
}

impl L2capChannelReader {
    /// Read a packet from the L2CAP channel.
    ///
    /// The packet is written to the start of `buf`, and the packet length is returned.
    #[inline]
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.reader.read(buf).await
    }

    /// Try reading a packet from the L2CAP channel.
    ///
    /// The packet is written to the start of `buf`, and the packet length is returned.
    ///
    /// If no packet is immediately available for reading, this returns an error with kind `NotReady`.
    #[inline]
    pub fn try_read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.reader.try_read(buf)
    }

    /// Close the L2CAP channel.
    ///
    /// This closes the entire channel, not just the read half.
    ///
    /// The channel is automatically closed when both the `L2capChannelWriter`
    /// and `L2capChannelReader` are dropped, so you don't need to call this explicitly.
    #[inline]
    pub async fn close(&mut self) -> Result<()> {
        self.reader.close().await
    }
}

impl L2capChannelWriter {
    /// Write a packet to the L2CAP channel.
    ///
    /// If the buffer is full, this will wait until there's buffer space for the packet.
    #[inline]
    pub async fn write(&mut self, packet: &[u8]) -> Result<()> {
        self.writer.write(packet).await
    }

    /// Try writing a packet to the L2CAP channel.
    ///
    /// If there's no buffer space, this returns an error with kind `NotReady`.
    #[inline]
    pub fn try_write(&mut self, packet: &[u8]) -> Result<()> {
        self.writer.try_write(packet)
    }

    /// Close the L2CAP channel.
    ///
    /// This closes the entire channel, not just the write half.
    ///
    /// The channel is automatically closed when both the `L2capChannelWriter`
    /// and `L2capChannelReader` are dropped, so you don't need to call this explicitly.
    #[inline]
    pub async fn close(&mut self) -> Result<()> {
        self.writer.close().await
    }
}
