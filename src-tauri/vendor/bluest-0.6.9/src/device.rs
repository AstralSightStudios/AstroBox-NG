#![allow(clippy::let_unit_value)]

use futures_core::Stream;
use futures_lite::StreamExt;

use crate::error::ErrorKind;
#[cfg(feature = "l2cap")]
use crate::l2cap_channel::L2capChannel;
use crate::pairing::PairingAgent;
use crate::{sys, DeviceId, Error, Result, Service, Uuid};

/// A Bluetooth LE device
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Device(pub(crate) sys::device::DeviceImpl);

impl std::fmt::Display for Device {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Device {
    /// This device's unique identifier
    #[inline]
    pub fn id(&self) -> DeviceId {
        self.0.id()
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    ///
    /// # Panics
    ///
    /// On Linux, this method will panic if there is a current Tokio runtime and it is single-threaded or if there is
    /// no current Tokio runtime and creating one fails.
    #[inline]
    pub fn name(&self) -> Result<String> {
        self.0.name()
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    #[inline]
    pub async fn name_async(&self) -> Result<String> {
        self.0.name_async().await
    }

    /// The connection status for this device
    #[inline]
    pub async fn is_connected(&self) -> bool {
        self.0.is_connected().await
    }

    /// The pairing status for this device
    #[inline]
    pub async fn is_paired(&self) -> Result<bool> {
        self.0.is_paired().await
    }

    /// Attempt to pair this device using the system default pairing UI
    ///
    /// # Platform specific
    ///
    /// ## MacOS/iOS
    ///
    /// Device pairing is performed automatically by the OS when a characteristic requiring security is accessed. This
    /// method is a no-op.
    ///
    /// ## Windows
    ///
    /// This will fail unless it is called from a UWP application.
    #[inline]
    pub async fn pair(&self) -> Result<()> {
        self.0.pair().await
    }

    /// Attempt to pair this device using the system default pairing UI
    ///
    /// # Platform specific
    ///
    /// On MacOS/iOS, device pairing is performed automatically by the OS when a characteristic requiring security is
    /// accessed. This method is a no-op.
    #[inline]
    pub async fn pair_with_agent<T: PairingAgent + 'static>(&self, agent: &T) -> Result<()> {
        self.0.pair_with_agent(agent).await
    }

    /// Disconnect and unpair this device from the system
    ///
    /// # Platform specific
    ///
    /// Not supported on MacOS/iOS.
    #[inline]
    pub async fn unpair(&self) -> Result<()> {
        self.0.unpair().await
    }

    /// Discover the primary services of this device.
    #[inline]
    pub async fn discover_services(&self) -> Result<Vec<Service>> {
        self.0.discover_services().await
    }

    /// Discover the primary service(s) of this device with the given [`Uuid`].
    #[inline]
    pub async fn discover_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        self.0.discover_services_with_uuid(uuid).await
    }

    /// Get previously discovered services.
    ///
    /// If no services have been discovered yet, this method will perform service discovery.
    #[inline]
    pub async fn services(&self) -> Result<Vec<Service>> {
        self.0.services().await
    }

    /// Asynchronously blocks until a GATT services changed packet is received
    ///
    /// # Platform specific
    ///
    /// See [`Device::service_changed_indications`].
    pub async fn services_changed(&self) -> Result<()> {
        self.service_changed_indications()
            .await?
            .next()
            .await
            .ok_or(Error::from(ErrorKind::AdapterUnavailable))
            .map(|x| x.map(|_| ()))?
    }

    /// Monitors the device for service changed indications.
    ///
    /// # Platform specific
    ///
    /// On Windows an event is generated whenever the `services` value is updated. In addition to actual service change
    /// indications this occurs when, for example, `discover_services` is called or when an unpaired device disconnects.
    #[inline]
    pub async fn service_changed_indications(
        &self,
    ) -> Result<impl Stream<Item = Result<ServicesChanged>> + Send + Unpin + '_> {
        self.0.service_changed_indications().await
    }

    /// Get the current signal strength from the device in dBm.
    ///
    /// # Platform specific
    ///
    /// Returns [`NotSupported`][crate::error::ErrorKind::NotSupported] on Windows and Linux.
    #[inline]
    pub async fn rssi(&self) -> Result<i16> {
        self.0.rssi().await
    }

    /// Open an L2CAP connection-oriented channel (CoC) to this device.
    ///
    /// # Platform specific
    ///
    /// Returns [`NotSupported`][crate::error::ErrorKind::NotSupported] on iOS/MacOS and Linux.
    /// The `l2cap` feature is not available on Windows.
    #[inline]
    #[cfg(feature = "l2cap")]
    pub async fn open_l2cap_channel(&self, psm: u16, secure: bool) -> Result<L2capChannel> {
        let (reader, writer) = self.0.open_l2cap_channel(psm, secure).await?;
        Ok(L2capChannel { reader, writer })
    }
}

/// A services changed notification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServicesChanged(pub(crate) sys::device::ServicesChangedImpl);

impl ServicesChanged {
    /// Check if `service` was invalidated by this service changed indication.
    ///
    /// # Platform specific
    ///
    /// Windows does not indicate which services were affected by a services changed event, so this method will
    /// pessimistically return true for all services.
    pub fn was_invalidated(&self, service: &Service) -> bool {
        self.0.was_invalidated(service)
    }
}
