#![allow(clippy::let_unit_value)]

use futures_core::Stream;
use futures_lite::StreamExt;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_bluetooth::{CBPeripheral, CBPeripheralState, CBService, CBUUID};
use objc2_foundation::{NSArray, NSData};

use super::delegates::{PeripheralDelegate, PeripheralEvent};
use super::dispatch::Dispatched;
#[cfg(feature = "l2cap")]
use super::l2cap_channel::{L2capChannelReader, L2capChannelWriter};
use crate::device::ServicesChanged;
use crate::error::ErrorKind;
use crate::pairing::PairingAgent;
use crate::{BluetoothUuidExt, Device, DeviceId, Error, Result, Service, Uuid};

/// A Bluetooth LE device
#[derive(Clone)]
pub struct DeviceImpl {
    pub(super) peripheral: Dispatched<CBPeripheral>,
    delegate: Retained<PeripheralDelegate>,
}

impl PartialEq for DeviceImpl {
    fn eq(&self, other: &Self) -> bool {
        self.peripheral == other.peripheral
    }
}

impl Eq for DeviceImpl {}

impl std::hash::Hash for DeviceImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.peripheral.hash(state)
    }
}

impl std::fmt::Debug for DeviceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Device").field(&self.peripheral).finish()
    }
}

impl std::fmt::Display for DeviceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name().as_deref().unwrap_or("(Unknown)"))
    }
}

impl Device {
    pub(super) fn new(peripheral: Retained<CBPeripheral>) -> Self {
        let delegate = unsafe { peripheral.delegate() }.unwrap_or_else(|| {
            // Create a new delegate and attach it to the peripheral
            let delegate = ProtocolObject::from_retained(PeripheralDelegate::new());
            unsafe { peripheral.setDelegate(Some(&delegate)) }
            delegate
        });

        let delegate = delegate.downcast().unwrap();

        Device(DeviceImpl {
            peripheral: unsafe { Dispatched::new(peripheral) },
            delegate,
        })
    }
}

impl DeviceImpl {
    /// This device's unique identifier
    pub fn id(&self) -> DeviceId {
        let uuid = self
            .peripheral
            .dispatch(|peripheral| unsafe { Uuid::from_bluetooth_bytes(&peripheral.identifier().as_bytes()[..]) });
        super::DeviceId(uuid)
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    pub fn name(&self) -> Result<String> {
        self.peripheral
            .dispatch(|peripheral| match unsafe { peripheral.name() } {
                Some(name) => Ok(name.to_string()),
                None => Err(ErrorKind::NotFound.into()),
            })
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    pub async fn name_async(&self) -> Result<String> {
        self.name()
    }

    /// The connection status for this device
    pub async fn is_connected(&self) -> bool {
        self.peripheral.dispatch(|peripheral| unsafe { peripheral.state() }) == CBPeripheralState::Connected
    }

    /// The pairing status for this device
    pub async fn is_paired(&self) -> Result<bool> {
        Err(ErrorKind::NotSupported.into())
    }

    /// Attempt to pair this device using the system default pairing UI
    ///
    /// Device pairing is performed automatically by the OS when a characteristic requiring security is accessed. This
    /// method is a no-op.
    pub async fn pair(&self) -> Result<()> {
        Ok(())
    }

    /// Attempt to pair this device using the system default pairing UI
    ///
    /// Device pairing is performed automatically by the OS when a characteristic requiring security is accessed. This
    /// method is a no-op.
    pub async fn pair_with_agent<T: PairingAgent>(&self, _agent: &T) -> Result<()> {
        Ok(())
    }

    /// Disconnect and unpair this device from the system
    ///
    /// # Platform specific
    ///
    /// Not supported on MacOS/iOS.
    pub async fn unpair(&self) -> Result<()> {
        Err(ErrorKind::NotSupported.into())
    }

    /// Discover the primary services of this device.
    pub async fn discover_services(&self) -> Result<Vec<Service>> {
        self.discover_services_inner(None).await
    }

    /// Discover the primary service(s) of this device with the given [`Uuid`].
    pub async fn discover_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        let services = self.discover_services_inner(Some(uuid)).await?;
        Ok(services.into_iter().filter(|x| x.uuid() == uuid).collect())
    }

    async fn discover_services_inner(&self, uuid: Option<Uuid>) -> Result<Vec<Service>> {
        let mut receiver = self.delegate.sender().new_receiver();

        if !self.is_connected().await {
            return Err(ErrorKind::NotConnected.into());
        }

        self.peripheral.dispatch(|peripheral| {
            let uuids = uuid.map(|uuid| unsafe {
                NSArray::from_retained_slice(&[CBUUID::UUIDWithData(&NSData::with_bytes(uuid.as_bluetooth_bytes()))])
            });

            unsafe { peripheral.discoverServices(uuids.as_deref()) };
        });

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DiscoveredServices { error: None } => break,
                PeripheralEvent::DiscoveredServices { error: Some(err) } => {
                    return Err(Error::from_nserror(err));
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                _ => (),
            }
        }

        self.services_inner()
    }

    /// Get previously discovered services.
    ///
    /// If no services have been discovered yet, this method will perform service discovery.
    pub async fn services(&self) -> Result<Vec<Service>> {
        match self.services_inner() {
            Ok(services) => Ok(services),
            Err(_) => self.discover_services().await,
        }
    }

    fn services_inner(&self) -> Result<Vec<Service>> {
        self.peripheral.dispatch(|peripheral| {
            unsafe { peripheral.services() }
                .map(|s| s.iter().map(|x| Service::new(x, self.delegate.clone())).collect())
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "no services have been discovered"))
        })
    }

    /// Monitors the device for services changed events.
    pub async fn service_changed_indications(
        &self,
    ) -> Result<impl Stream<Item = Result<ServicesChanged>> + Send + Unpin + '_> {
        let receiver = self.delegate.sender().new_receiver();

        if !self.is_connected().await {
            return Err(ErrorKind::NotConnected.into());
        }

        Ok(receiver.filter_map(|ev| match ev {
            PeripheralEvent::ServicesChanged { invalidated_services } => {
                Some(Ok(ServicesChanged(ServicesChangedImpl(invalidated_services))))
            }
            PeripheralEvent::Disconnected { error } => {
                Some(Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error)))
            }
            _ => None,
        }))
    }

    /// Get the current signal strength from the device in dBm.
    pub async fn rssi(&self) -> Result<i16> {
        let mut receiver = self.delegate.sender().new_receiver();
        self.peripheral.dispatch(|peripheral| unsafe { peripheral.readRSSI() });

        loop {
            match receiver.recv().await {
                Ok(PeripheralEvent::ReadRssi { rssi, error: None }) => return Ok(rssi),
                Ok(PeripheralEvent::ReadRssi { error: Some(err), .. }) => return Err(Error::from_nserror(err)),
                Err(err) => return Err(Error::from_recv_error(err)),
                _ => (),
            }
        }
    }

    /// Open L2CAP channel given PSM
    #[cfg(feature = "l2cap")]
    pub async fn open_l2cap_channel(
        &self,
        psm: u16,
        _secure: bool,
    ) -> Result<(L2capChannelReader, L2capChannelWriter)> {
        use tracing::{debug, info};

        let mut receiver = self.delegate.sender().new_receiver();

        if !self.is_connected().await {
            return Err(ErrorKind::NotConnected.into());
        }

        info!("starting open_l2cap_channel on {}", psm);
        self.peripheral
            .dispatch(|peripheral| unsafe { peripheral.openL2CAPChannel(psm) });

        let l2capchannel;
        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::L2CAPChannelOpened { channel, error: None } => {
                    l2capchannel = channel;
                    break;
                }
                PeripheralEvent::L2CAPChannelOpened { channel: _, error } => {
                    return Err(Error::from_nserror(error.unwrap()));
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                o => {
                    info!("Other event: {:?}", o);
                }
            }
        }
        debug!("open_l2cap_channel success {:?}", self.peripheral);

        let reader = L2capChannelReader::new(l2capchannel.clone());
        let writer = L2capChannelWriter::new(l2capchannel);

        Ok((reader, writer))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServicesChangedImpl(Vec<Dispatched<CBService>>);

impl ServicesChangedImpl {
    pub fn was_invalidated(&self, service: &Service) -> bool {
        self.0.contains(&service.0.inner)
    }
}
