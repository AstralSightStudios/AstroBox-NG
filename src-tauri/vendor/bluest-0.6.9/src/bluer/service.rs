use std::sync::Arc;

use crate::{Characteristic, Result, Service, Uuid};

/// A Bluetooth GATT service
#[derive(Debug, Clone)]
pub struct ServiceImpl {
    pub(super) inner: bluer::gatt::remote::Service,
    device: Arc<bluer::Device>,
}

impl PartialEq for ServiceImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.adapter_name() == other.inner.adapter_name()
            && self.inner.device_address() == other.inner.device_address()
            && self.inner.id() == other.inner.id()
    }
}

impl Eq for ServiceImpl {}

impl std::hash::Hash for ServiceImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.adapter_name().hash(state);
        self.inner.device_address().hash(state);
        self.inner.id().hash(state);
    }
}

impl Service {
    pub(super) fn new(device: Arc<bluer::Device>, inner: bluer::gatt::remote::Service) -> Service {
        Service(ServiceImpl { inner, device })
    }
}

impl ServiceImpl {
    /// The [`Uuid`] identifying the type of this GATT service
    ///
    /// # Panics
    ///
    /// This method will panic if there is a current Tokio runtime and it is single-threaded, if there is no current
    /// Tokio runtime and creating one fails, or if the underlying [`ServiceImpl::uuid_async()`] method fails.
    pub fn uuid(&self) -> Uuid {
        // Call an async function from a synchronous context
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(move || handle.block_on(self.uuid_async())),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(self.uuid_async()),
        }
        .unwrap()
    }

    /// The [`Uuid`] identifying the type of this GATT service
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.inner.uuid().await.map_err(Into::into)
    }

    /// Whether this is a primary service of the device.
    pub async fn is_primary(&self) -> Result<bool> {
        self.inner.primary().await.map_err(Into::into)
    }

    /// Discover all characteristics associated with this service.
    pub async fn discover_characteristics(&self) -> Result<Vec<Characteristic>> {
        self.characteristics().await
    }

    /// Discover the characteristic(s) with the given [`Uuid`].
    pub async fn discover_characteristics_with_uuid(&self, uuid: Uuid) -> Result<Vec<Characteristic>> {
        Ok(self
            .characteristics()
            .await?
            .into_iter()
            .filter(|x| x.uuid() == uuid)
            .collect())
    }

    /// Get previously discovered characteristics.
    ///
    /// If no characteristics have been discovered yet, this method will perform characteristic discovery.
    pub async fn characteristics(&self) -> Result<Vec<Characteristic>> {
        self.inner
            .characteristics()
            .await
            .map_err(Into::into)
            .map(|x| x.into_iter().map(Characteristic::new).collect())
    }

    /// Discover the included services of this service.
    pub async fn discover_included_services(&self) -> Result<Vec<Service>> {
        self.included_services().await
    }

    /// Discover the included service(s) with the given [`Uuid`].
    pub async fn discover_included_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        Ok(self
            .included_services()
            .await?
            .into_iter()
            .filter(|x| x.uuid() == uuid)
            .collect())
    }

    /// Get previously discovered included services.
    ///
    /// If no included services have been discovered yet, this method will perform included service discovery.
    pub async fn included_services(&self) -> Result<Vec<Service>> {
        let includes = self.inner.includes().await?;
        let mut res = Vec::with_capacity(includes.len());
        for id in includes {
            res.push(Service::new(self.device.clone(), self.device.service(id).await?));
        }
        Ok(res)
    }
}
