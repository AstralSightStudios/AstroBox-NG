use crate::{sys, Characteristic, Result, Uuid};

/// A Bluetooth GATT service
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Service(pub(crate) sys::service::ServiceImpl);

impl Service {
    /// The [`Uuid`] identifying the type of this GATT service
    ///
    /// # Panics
    ///
    /// On Linux, this method will panic if there is a current Tokio runtime and it is single-threaded, if there is no
    /// current Tokio runtime and creating one fails, or if the underlying [`Service::uuid_async()`] method
    /// fails.
    #[inline]
    pub fn uuid(&self) -> Uuid {
        self.0.uuid()
    }

    /// The [`Uuid`] identifying the type of this GATT service
    #[inline]
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.0.uuid_async().await
    }

    /// Whether this is a primary service of the device.
    ///
    /// # Platform specific
    ///
    /// Returns [`NotSupported`][crate::error::ErrorKind::NotSupported] on Windows.
    #[inline]
    pub async fn is_primary(&self) -> Result<bool> {
        self.0.is_primary().await
    }

    /// Discover all characteristics associated with this service.
    #[inline]
    pub async fn discover_characteristics(&self) -> Result<Vec<Characteristic>> {
        self.0.discover_characteristics().await
    }

    /// Discover the characteristic(s) with the given [`Uuid`].
    #[inline]
    pub async fn discover_characteristics_with_uuid(&self, uuid: Uuid) -> Result<Vec<Characteristic>> {
        self.0.discover_characteristics_with_uuid(uuid).await
    }

    /// Get previously discovered characteristics.
    ///
    /// If no characteristics have been discovered yet, this method will perform characteristic discovery.
    #[inline]
    pub async fn characteristics(&self) -> Result<Vec<Characteristic>> {
        self.0.characteristics().await
    }

    /// Discover the included services of this service.
    #[inline]
    pub async fn discover_included_services(&self) -> Result<Vec<Service>> {
        self.0.discover_included_services().await
    }

    /// Discover the included service(s) with the given [`Uuid`].
    #[inline]
    pub async fn discover_included_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        self.0.discover_included_services_with_uuid(uuid).await
    }

    /// Get previously discovered included services.
    ///
    /// If no included services have been discovered yet, this method will perform included service discovery.
    #[inline]
    pub async fn included_services(&self) -> Result<Vec<Service>> {
        self.0.included_services().await
    }
}
