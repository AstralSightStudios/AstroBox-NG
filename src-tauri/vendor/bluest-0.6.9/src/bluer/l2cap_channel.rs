#![cfg(feature = "l2cap")]

use std::fmt;

use crate::Result;

pub struct L2capChannelReader {
    _private: (),
}

impl L2capChannelReader {
    #[inline]
    pub async fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    pub fn try_read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        todo!()
    }

    pub async fn close(&mut self) -> Result<()> {
        todo!()
    }
}

impl fmt::Debug for L2capChannelReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("L2capChannelReader")
    }
}

pub struct L2capChannelWriter {
    _private: (),
}

impl L2capChannelWriter {
    pub async fn write(&mut self, _packet: &[u8]) -> Result<()> {
        todo!()
    }

    pub fn try_write(&mut self, _packet: &[u8]) -> Result<()> {
        todo!()
    }

    pub async fn close(&mut self) -> Result<()> {
        todo!()
    }
}

impl fmt::Debug for L2capChannelWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("L2capChannelWriter")
    }
}
