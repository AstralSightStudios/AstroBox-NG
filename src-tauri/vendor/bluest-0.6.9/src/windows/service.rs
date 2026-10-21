use windows::core::GUID;
use windows::Devices::Bluetooth::BluetoothCacheMode;
use windows::Devices::Bluetooth::GenericAttributeProfile::GattDeviceService;

use super::error::check_communication_status;
use crate::error::ErrorKind;
use crate::{Characteristic, Result, Service, Uuid};

/// A Bluetooth GATT service
#[derive(Clone)]
pub struct ServiceImpl {
    inner: GattDeviceService,
}

impl PartialEq for ServiceImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.Session().unwrap() == other.inner.Session().unwrap()
            && self.inner.AttributeHandle().unwrap() == other.inner.AttributeHandle().unwrap()
    }
}

impl Eq for ServiceImpl {}

impl std::hash::Hash for ServiceImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner
            .Session()
            .unwrap()
            .DeviceId()
            .unwrap()
            .Id()
            .unwrap()
            .to_os_string()
            .hash(state);
        self.inner.AttributeHandle().unwrap().hash(state);
    }
}

impl std::fmt::Debug for ServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field(
                "device_id",
                &self
                    .inner
                    .Session()
                    .expect("Session missing on GattDeviceService")
                    .DeviceId()
                    .expect("DeviceId missing on GattSession")
                    .Id()
                    .expect("Id missing on BluetoothDeviceId")
                    .to_os_string(),
            )
            .field("uuid", &self.inner.Uuid().expect("UUID missing on GattDeviceService"))
            .field(
                "handle",
                &self
                    .inner
                    .AttributeHandle()
                    .expect("AttributeHandle missing on GattDeviceService"),
            )
            .finish()
    }
}

impl Service {
    pub(super) fn new(service: GattDeviceService) -> Self {
        Service(ServiceImpl { inner: service })
    }
}

impl ServiceImpl {
    /// The [`Uuid`] identifying the type of service
    pub fn uuid(&self) -> Uuid {
        Uuid::from_u128(self.inner.Uuid().expect("UUID missing on GattDeviceService").to_u128())
    }

    /// The [`Uuid`] identifying the type of this GATT service
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(Uuid::from_u128(self.inner.Uuid()?.to_u128()))
    }

    /// Whether this is a primary service of the device.
    ///
    /// Returns [ErrorKind::NotSupported].
    pub async fn is_primary(&self) -> Result<bool> {
        Err(ErrorKind::NotSupported.into())
    }

    /// Discover all characteristics associated with this service.
    pub async fn discover_characteristics(&self) -> Result<Vec<Characteristic>> {
        let res = self
            .inner
            .GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering characteristics")?;
        let characteristics = res.Characteristics()?;
        Ok(characteristics.into_iter().map(Characteristic::new).collect())
    }

    /// Discover the characteristic(s) with the given [`Uuid`].
    pub async fn discover_characteristics_with_uuid(&self, uuid: Uuid) -> Result<Vec<Characteristic>> {
        let res = self
            .inner
            .GetCharacteristicsForUuidWithCacheModeAsync(GUID::from_u128(uuid.as_u128()), BluetoothCacheMode::Uncached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering characteristics")?;
        let characteristics = res.Characteristics()?;
        Ok(characteristics.into_iter().map(Characteristic::new).collect())
    }

    /// Get previously discovered characteristics.
    ///
    /// If no characteristics have been discovered yet, this method will perform characteristic discovery.
    pub async fn characteristics(&self) -> Result<Vec<Characteristic>> {
        let res = self
            .inner
            .GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Cached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering characteristics")?;
        let characteristics = res.Characteristics()?;
        Ok(characteristics.into_iter().map(Characteristic::new).collect())
    }

    /// Discover the included services of this service.
    pub async fn discover_included_services(&self) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetIncludedServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering included services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }

    /// Discover the included service(s) with the given [`Uuid`].
    pub async fn discover_included_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetIncludedServicesForUuidWithCacheModeAsync(
                GUID::from_u128(uuid.as_u128()),
                BluetoothCacheMode::Uncached,
            )?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering included services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }

    /// Get previously discovered included services.
    ///
    /// If no included services have been discovered yet, this method will perform included service discovery.
    pub async fn included_services(&self) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetIncludedServicesWithCacheModeAsync(BluetoothCacheMode::Cached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering included services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }
}
