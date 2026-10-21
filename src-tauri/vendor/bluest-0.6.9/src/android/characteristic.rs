use futures_core::Stream;
use futures_lite::stream;
use uuid::Uuid;

use crate::{CharacteristicProperties, Descriptor, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacteristicImpl {}

impl CharacteristicImpl {
    pub fn uuid(&self) -> Uuid {
        todo!()
    }

    pub async fn uuid_async(&self) -> Result<Uuid> {
        todo!()
    }

    pub async fn properties(&self) -> Result<CharacteristicProperties> {
        todo!()
    }

    pub async fn value(&self) -> Result<Vec<u8>> {
        todo!()
    }

    pub async fn read(&self) -> Result<Vec<u8>> {
        todo!()
    }

    pub async fn write(&self, _value: &[u8]) -> Result<()> {
        todo!()
    }

    pub async fn write_without_response(&self, _value: &[u8]) -> Result<()> {
        todo!()
    }

    pub fn max_write_len(&self) -> Result<usize> {
        todo!()
    }

    pub async fn max_write_len_async(&self) -> Result<usize> {
        todo!()
    }

    pub async fn notify(&self) -> Result<impl Stream<Item = Result<Vec<u8>>> + Send + Unpin + '_> {
        Ok(stream::empty()) // TODO
    }

    pub async fn is_notifying(&self) -> Result<bool> {
        todo!()
    }

    pub async fn discover_descriptors(&self) -> Result<Vec<Descriptor>> {
        todo!()
    }

    pub async fn descriptors(&self) -> Result<Vec<Descriptor>> {
        todo!()
    }
}
