use crate::{Result, Uuid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorImpl {}

impl DescriptorImpl {
    pub fn uuid(&self) -> Uuid {
        todo!()
    }

    pub async fn uuid_async(&self) -> Result<Uuid> {
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
}
