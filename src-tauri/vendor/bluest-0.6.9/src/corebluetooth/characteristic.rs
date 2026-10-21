use futures_core::Stream;
use futures_lite::StreamExt;
use objc2::rc::Retained;
use objc2_core_bluetooth::{
    CBCharacteristic, CBCharacteristicProperties, CBCharacteristicWriteType, CBPeripheralState,
};
use objc2_foundation::NSData;

use super::delegates::{PeripheralDelegate, PeripheralEvent};
use super::dispatch::Dispatched;
use crate::error::ErrorKind;
use crate::util::defer;
use crate::{BluetoothUuidExt, Characteristic, CharacteristicProperties, Descriptor, Error, Result, Uuid};

/// A Bluetooth GATT characteristic
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacteristicImpl {
    inner: Dispatched<CBCharacteristic>,
    delegate: Retained<PeripheralDelegate>,
}

impl Characteristic {
    pub(super) fn new(characteristic: Retained<CBCharacteristic>, delegate: Retained<PeripheralDelegate>) -> Self {
        Characteristic(CharacteristicImpl {
            inner: unsafe { Dispatched::new(characteristic) },
            delegate,
        })
    }
}

impl CharacteristicImpl {
    /// The [`Uuid`] identifying the type of this GATT characteristic
    pub fn uuid(&self) -> Uuid {
        self.inner.dispatch(|characteristic| unsafe {
            Uuid::from_bluetooth_bytes(characteristic.UUID().data().as_bytes_unchecked())
        })
    }

    /// The [`Uuid`] identifying the type of this GATT characteristic
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(self.uuid())
    }

    /// The properties of this this GATT characteristic.
    ///
    /// Characteristic properties indicate which operations (e.g. read, write, notify, etc) may be performed on this
    /// characteristic.
    pub async fn properties(&self) -> Result<CharacteristicProperties> {
        let cb_props = self
            .inner
            .dispatch(|characteristic| unsafe { characteristic.properties() });

        let mut props = CharacteristicProperties::default();
        if cb_props.contains(CBCharacteristicProperties::Broadcast) {
            props.broadcast = true;
        }
        if cb_props.contains(CBCharacteristicProperties::Read) {
            props.read = true;
        }
        if cb_props.contains(CBCharacteristicProperties::WriteWithoutResponse) {
            props.write_without_response = true;
        }
        if cb_props.contains(CBCharacteristicProperties::Write) {
            props.write = true;
        }
        if cb_props.contains(CBCharacteristicProperties::Notify) {
            props.notify = true;
        }
        if cb_props.contains(CBCharacteristicProperties::Indicate) {
            props.indicate = true;
        }
        if cb_props.contains(CBCharacteristicProperties::AuthenticatedSignedWrites) {
            props.authenticated_signed_writes = true;
        }
        if cb_props.contains(CBCharacteristicProperties::ExtendedProperties) {
            props.extended_properties = true;
        }

        Ok(props)
    }

    /// The cached value of this characteristic
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.inner.dispatch(|characteristic| unsafe {
            characteristic
                .value()
                .map(|val| val.as_bytes_unchecked().to_vec())
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "the characteristic value has not been read"))
        })
    }

    /// Read the value of this characteristic from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|characteristic| {
            let service = unsafe { characteristic.service() }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "service not found",
            ))?;
            let peripheral =
                unsafe { service.peripheral() }.ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?;

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.readValueForCharacteristic(characteristic) };
            Ok(unsafe { Dispatched::new(service) })
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::CharacteristicValueUpdate {
                    characteristic,
                    data,
                    error,
                } if characteristic == self.inner => match error {
                    Some(err) => return Err(Error::from_nserror(err)),
                    None => return Ok(data),
                },
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&service) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }
    }

    /// Write the value of this descriptor on the device to `value` and request the device return a response indicating
    /// a successful write.
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|characteristic| {
            let service = unsafe { characteristic.service() }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "service not found",
            ))?;
            let peripheral =
                unsafe { service.peripheral() }.ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?;

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            let data = NSData::with_bytes(value);

            unsafe {
                peripheral.writeValue_forCharacteristic_type(
                    &data,
                    characteristic,
                    CBCharacteristicWriteType::WithResponse,
                )
            };

            Ok(unsafe { Dispatched::new(service) })
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::CharacteristicValueWrite { characteristic, error } if characteristic == self.inner => {
                    match error {
                        Some(err) => return Err(Error::from_nserror(err)),
                        None => return Ok(()),
                    }
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&service) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }
    }

    /// Write the value of this descriptor on the device to `value` without requesting a response.
    pub async fn write_without_response(&self, value: &[u8]) -> Result<()> {
        let mut receiver = self.delegate.sender().new_receiver();
        loop {
            let service = self.inner.dispatch(|characteristic| {
                let service = unsafe { characteristic.service() }.ok_or(Error::new(
                    ErrorKind::NotFound,
                    None,
                    "service not found",
                ))?;
                let peripheral = unsafe { service.peripheral() }.ok_or(Error::new(
                    ErrorKind::NotFound,
                    None,
                    "peripheral not found",
                ))?;

                if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                    Err(Error::from(ErrorKind::NotConnected))
                } else if unsafe { peripheral.canSendWriteWithoutResponse() } {
                    let data = NSData::with_bytes(value);
                    unsafe {
                        peripheral.writeValue_forCharacteristic_type(
                            &data,
                            characteristic,
                            CBCharacteristicWriteType::WithoutResponse,
                        )
                    };
                    Ok(None)
                } else {
                    Ok(Some(unsafe { Dispatched::new(service) }))
                }
            })?;

            if let Some(service) = service {
                while let Ok(evt) = receiver.recv().await {
                    match evt {
                        PeripheralEvent::ReadyToWrite => break,
                        PeripheralEvent::Disconnected { error } => {
                            return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                        }
                        PeripheralEvent::ServicesChanged { invalidated_services }
                            if invalidated_services.contains(&service) =>
                        {
                            return Err(ErrorKind::ServiceChanged.into());
                        }
                        _ => (),
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub fn max_write_len(&self) -> Result<usize> {
        self.inner.dispatch(|characteristic| {
            let peripheral = unsafe { characteristic.service().and_then(|x| x.peripheral()) }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "peripheral not found",
            ))?;
            unsafe { Ok(peripheral.maximumWriteValueLengthForType(CBCharacteristicWriteType::WithoutResponse)) }
        })
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub async fn max_write_len_async(&self) -> Result<usize> {
        self.max_write_len()
    }

    /// Enables notification of value changes for this GATT characteristic.
    ///
    /// Returns a stream of values for the characteristic sent from the device.
    pub async fn notify(&self) -> Result<impl Stream<Item = Result<Vec<u8>>> + Send + Unpin + '_> {
        let properties = self.properties().await?;
        if !(properties.notify || properties.indicate) {
            return Err(Error::new(
                ErrorKind::NotSupported,
                None,
                "characteristic does not support indications or notifications",
            ));
        };

        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|characteristic| {
            let service = unsafe { characteristic.service() }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "service not found",
            ))?;
            let peripheral =
                unsafe { service.peripheral() }.ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?;

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.setNotifyValue_forCharacteristic(true, characteristic) };

            Ok(unsafe { Dispatched::new(service) })
        })?;

        let guard = defer(move || {
            self.inner.dispatch(|characteristic| {
                if let Some(peripheral) = unsafe { characteristic.service().and_then(|x| x.peripheral()) } {
                    unsafe { peripheral.setNotifyValue_forCharacteristic(false, characteristic) };
                }
            });
        });

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::NotificationStateUpdate { characteristic, error } if characteristic == self.inner => {
                    match error {
                        Some(err) => return Err(Error::from_nserror(err)),
                        None => break,
                    }
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&service) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }

        let updates = receiver
            .filter_map(move |x| {
                let _guard = &guard;
                match x {
                    PeripheralEvent::CharacteristicValueUpdate {
                        characteristic,
                        data,
                        error,
                    } if characteristic == self.inner => match error {
                        Some(err) => Some(Err(Error::from_nserror(err))),
                        None => Some(Ok(data)),
                    },
                    PeripheralEvent::Disconnected { error } => {
                        Some(Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error)))
                    }
                    PeripheralEvent::ServicesChanged { invalidated_services }
                        if invalidated_services.contains(&service) =>
                    {
                        Some(Err(ErrorKind::ServiceChanged.into()))
                    }
                    _ => None,
                }
            })
            .then(move |x| {
                Box::pin(async move {
                    match x {
                        Ok(data) => Ok(data),
                        Err(err) => Err(err),
                    }
                })
            });

        Ok(updates)
    }

    /// Is the device currently sending notifications for this characteristic?
    pub async fn is_notifying(&self) -> Result<bool> {
        Ok(self
            .inner
            .dispatch(|characteristic| unsafe { characteristic.isNotifying() }))
    }

    /// Discover the descriptors associated with this characteristic.
    pub async fn discover_descriptors(&self) -> Result<Vec<Descriptor>> {
        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|characteristic| {
            let service = unsafe { characteristic.service() }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "service not found",
            ))?;
            let peripheral =
                unsafe { service.peripheral() }.ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?;

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.discoverDescriptorsForCharacteristic(characteristic) }
            Ok(unsafe { Dispatched::new(service) })
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DiscoveredDescriptors { characteristic, error } if characteristic == self.inner => {
                    match error {
                        Some(err) => return Err(Error::from_nserror(err)),
                        None => break,
                    }
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&service) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }

        self.descriptors_inner()
    }

    /// Get previously discovered descriptors.
    ///
    /// If no descriptors have been discovered yet, this method will perform descriptor discovery.
    pub async fn descriptors(&self) -> Result<Vec<Descriptor>> {
        match self.descriptors_inner() {
            Ok(descriptors) => Ok(descriptors),
            Err(_) => self.discover_descriptors().await,
        }
    }

    fn descriptors_inner(&self) -> Result<Vec<Descriptor>> {
        self.inner.dispatch(|characteristic| {
            unsafe { characteristic.descriptors() }
                .map(|s| s.iter().map(|x| Descriptor::new(x, self.delegate.clone())).collect())
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "no descriptors have been discovered"))
        })
    }
}
