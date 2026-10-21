use crate::{Characteristic, Result, Service, Uuid};

#[derive(Debug, Clone)]
pub struct ServiceImpl {}

impl PartialEq for ServiceImpl {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Eq for ServiceImpl {}

impl std::hash::Hash for ServiceImpl {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        todo!()
    }
}

impl ServiceImpl {
    pub fn uuid(&self) -> Uuid {
        todo!()
    }

    pub async fn uuid_async(&self) -> Result<Uuid> {
        todo!()
    }

    pub async fn is_primary(&self) -> Result<bool> {
        todo!()
    }

    pub async fn discover_characteristics(&self) -> Result<Vec<Characteristic>> {
        todo!()
    }

    pub async fn discover_characteristics_with_uuid(&self, _uuid: Uuid) -> Result<Vec<Characteristic>> {
        todo!()
    }

    pub async fn characteristics(&self) -> Result<Vec<Characteristic>> {
        todo!()
    }

    pub async fn discover_included_services(&self) -> Result<Vec<Service>> {
        todo!()
    }

    pub async fn discover_included_services_with_uuid(&self, _uuid: Uuid) -> Result<Vec<Service>> {
        todo!()
    }

    pub async fn included_services(&self) -> Result<Vec<Service>> {
        todo!()
    }
}
