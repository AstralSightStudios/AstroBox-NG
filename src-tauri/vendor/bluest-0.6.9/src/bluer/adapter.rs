use std::sync::Arc;

use bluer::AdapterProperty;
use futures_core::Stream;
use futures_lite::StreamExt;

use crate::error::ErrorKind;
use crate::{AdapterEvent, AdvertisingDevice, ConnectionEvent, Device, DeviceId, Error, Result, Uuid};

/// The system's Bluetooth adapter interface.
///
/// The default adapter for the system may be accessed with the [`Adapter::default()`] method.
#[derive(Debug, Clone)]
pub struct AdapterImpl {
    inner: bluer::Adapter,
    session: Arc<bluer::Session>,
}

impl PartialEq for AdapterImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.name() == other.inner.name()
    }
}

impl Eq for AdapterImpl {}

impl std::hash::Hash for AdapterImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.name().hash(state);
    }
}

impl AdapterImpl {
    /// Creates an interface to the default Bluetooth adapter for the system
    pub async fn default() -> Option<Self> {
        let session = Arc::new(bluer::Session::new().await.ok()?);
        session
            .default_adapter()
            .await
            .ok()
            .map(|inner| AdapterImpl { inner, session })
    }

    /// A stream of [`AdapterEvent`] which allows the application to identify when the adapter is enabled or disabled.
    pub async fn events(&self) -> Result<impl Stream<Item = Result<AdapterEvent>> + Send + Unpin + '_> {
        let stream = self.inner.events().await?;
        Ok(stream.filter_map(|event| match event {
            bluer::AdapterEvent::PropertyChanged(AdapterProperty::Powered(true)) => Some(Ok(AdapterEvent::Available)),
            bluer::AdapterEvent::PropertyChanged(AdapterProperty::Powered(false)) => {
                Some(Ok(AdapterEvent::Unavailable))
            }
            _ => None,
        }))
    }

    /// Asynchronously blocks until the adapter is available
    pub async fn wait_available(&self) -> Result<()> {
        let events = self.events();
        if !self.inner.is_powered().await? {
            events
                .await?
                .skip_while(|x| x.is_ok() && !matches!(x, Ok(AdapterEvent::Available)))
                .next()
                .await
                .ok_or_else(|| Error::new(ErrorKind::Internal, None, "adapter event stream closed unexpectedly"))??;
        }
        Ok(())
    }

    /// Attempts to create the device identified by `id`
    pub async fn open_device(&self, id: &DeviceId) -> Result<Device> {
        Device::new(self.session.clone(), &self.inner, id.0)
    }

    /// Finds all connected Bluetooth LE devices
    pub async fn connected_devices(&self) -> Result<Vec<Device>> {
        let mut devices = Vec::new();
        for device in self
            .inner
            .device_addresses()
            .await?
            .into_iter()
            .filter_map(|addr| Device::new(self.session.clone(), &self.inner, addr).ok())
        {
            if device.is_connected().await {
                devices.push(device);
            }
        }

        Ok(devices)
    }

    /// Finds all connected devices providing any service in `services`
    ///
    /// # Panics
    ///
    /// Panics if `services` is empty.
    pub async fn connected_devices_with_services(&self, services: &[Uuid]) -> Result<Vec<Device>> {
        assert!(!services.is_empty());

        let devices = self.connected_devices().await?;
        let mut res = Vec::new();
        for device in devices {
            for service in device.0.inner.services().await? {
                if services.contains(&service.uuid().await?) {
                    res.push(device);
                    break;
                }
            }
        }

        Ok(res)
    }

    /// Starts scanning for Bluetooth advertising packets.
    ///
    /// Returns a stream of [`AdvertisingDevice`] structs which contain the data from the advertising packet and the
    /// [`Device`] which sent it. Scanning is automatically stopped when the stream is dropped. Inclusion of duplicate
    /// packets is a platform-specific implementation detail.
    ///
    /// If `services` is not empty, returns advertisements including at least one GATT service with a UUID in
    /// `services`. Otherwise returns all advertisements.
    pub async fn scan<'a>(
        &'a self,
        services: &'a [Uuid],
    ) -> Result<impl Stream<Item = AdvertisingDevice> + Send + Unpin + 'a> {
        Ok(self
            .inner
            .discover_devices()
            .await?
            .then(move |event| {
                Box::pin(async move {
                    match event {
                        bluer::AdapterEvent::DeviceAdded(addr) => {
                            let device = Device::new(self.session.clone(), &self.inner, addr).ok()?;
                            if !device.is_connected().await {
                                let adv_data = device.0.adv_data().await;
                                let rssi = device.rssi().await.ok();
                                Some(AdvertisingDevice { device, adv_data, rssi })
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                })
            })
            .filter_map(|x| x)
            .filter(|x: &AdvertisingDevice| {
                services.is_empty() || x.adv_data.services.iter().any(|y| services.contains(y))
            }))
    }

    /// Finds Bluetooth devices providing any service in `services`.
    ///
    /// Returns a stream of [`Device`] structs with matching connected devices returned first. If the stream is not
    /// dropped before all matching connected devices are consumed then scanning will begin for devices advertising any
    /// of the `services`. Scanning will continue until the stream is dropped. Inclusion of duplicate devices is a
    /// platform-specific implementation detail.
    pub async fn discover_devices<'a>(
        &'a self,
        services: &'a [Uuid],
    ) -> Result<impl Stream<Item = Result<Device>> + Send + Unpin + 'a> {
        Ok(self
            .inner
            .discover_devices()
            .await?
            .then(move |event| {
                Box::pin(async move {
                    match event {
                        bluer::AdapterEvent::DeviceAdded(addr) => {
                            let device = match Device::new(self.session.clone(), &self.inner, addr) {
                                Ok(device) => device,
                                Err(err) => return Some(Err(err)),
                            };

                            if services.is_empty() {
                                Some(Ok(device))
                            } else {
                                match device.0.inner.uuids().await {
                                    Ok(uuids) => {
                                        let uuids = uuids.unwrap_or_default();
                                        if services.iter().any(|x| uuids.contains(x)) {
                                            Some(Ok(device))
                                        } else {
                                            None
                                        }
                                    }
                                    Err(err) => Some(Err(err.into())),
                                }
                            }
                        }
                        _ => None,
                    }
                })
            })
            .filter_map(|x| x))
    }

    /// Connects to the [`Device`]
    pub async fn connect_device(&self, device: &Device) -> Result<()> {
        device.0.inner.connect().await.map_err(Into::into)
    }

    /// Disconnects from the [`Device`]
    pub async fn disconnect_device(&self, device: &Device) -> Result<()> {
        device.0.inner.disconnect().await.map_err(Into::into)
    }

    /// Monitors a device for connection/disconnection events.
    #[inline]
    pub async fn device_connection_events<'a>(
        &'a self,
        device: &'a Device,
    ) -> Result<impl Stream<Item = ConnectionEvent> + Send + Unpin + 'a> {
        let events = device.0.inner.events().await?;
        Ok(events.filter_map(|ev| match ev {
            bluer::DeviceEvent::PropertyChanged(bluer::DeviceProperty::Connected(false)) => {
                Some(ConnectionEvent::Disconnected)
            }
            bluer::DeviceEvent::PropertyChanged(bluer::DeviceProperty::Connected(true)) => {
                Some(ConnectionEvent::Connected)
            }
            _ => None,
        }))
    }
}
