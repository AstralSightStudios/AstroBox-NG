use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_core_bluetooth::{CBDescriptor, CBPeripheralState};
use objc2_foundation::{NSData, NSNumber, NSString, NSUInteger};

use super::delegates::{PeripheralDelegate, PeripheralEvent};
use super::dispatch::Dispatched;
use crate::error::ErrorKind;
use crate::{BluetoothUuidExt, Descriptor, Error, Result, Uuid};

/// A Bluetooth GATT descriptor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorImpl {
    inner: Dispatched<CBDescriptor>,
    delegate: Retained<PeripheralDelegate>,
}

fn value_to_slice(val: &AnyObject) -> Vec<u8> {
    if let Some(val) = val.downcast_ref::<NSNumber>() {
        // Characteristic EXtended Properties, Client Characteristic COnfiguration, Service Characteristic Configuration, or L2CAP PSM Value Characteristic
        let n = val.as_u16();
        n.to_le_bytes().to_vec()
    } else if let Some(val) = val.downcast_ref::<NSString>() {
        // Characteristic User Description
        let ptr: *const u8 = unsafe { msg_send![val, UTF8String] };
        let val = if ptr.is_null() {
            &[]
        } else {
            let len: NSUInteger = unsafe { msg_send![val, lengthOfBytesUsingEncoding: 4usize] }; // NSUTF8StringEncoding
            unsafe { std::slice::from_raw_parts(ptr, len) }
        };
        val.to_vec()
    } else if let Some(val) = val.downcast_ref::<NSData>() {
        // All other descriptors
        val.to_vec()
    } else {
        Vec::new()
    }
}

impl Descriptor {
    pub(super) fn new(descriptor: Retained<CBDescriptor>, delegate: Retained<PeripheralDelegate>) -> Self {
        Descriptor(DescriptorImpl {
            inner: unsafe { Dispatched::new(descriptor) },
            delegate,
        })
    }
}

impl DescriptorImpl {
    /// The [`Uuid`] identifying the type of this GATT descriptor
    pub fn uuid(&self) -> Uuid {
        self.inner
            .dispatch(|descriptor| unsafe { Uuid::from_bluetooth_bytes(descriptor.UUID().data().as_bytes_unchecked()) })
    }

    /// The [`Uuid`] identifying the type of this GATT descriptor
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(self.uuid())
    }

    /// The cached value of this descriptor
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.inner.dispatch(|descriptor| unsafe {
            descriptor
                .value()
                .map(|val| value_to_slice(&val))
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "the descriptor value has not been read"))
        })
    }

    /// Read the value of this descriptor from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|descriptor| {
            let service = unsafe { descriptor.characteristic().and_then(|x| x.service()) }.ok_or(Error::new(
                ErrorKind::NotFound,
                None,
                "service not found",
            ))?;
            let peripheral =
                unsafe { service.peripheral() }.ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?;

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.readValueForDescriptor(descriptor) };

            Ok(unsafe { Dispatched::new(service) })
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DescriptorValueUpdate { descriptor, error } if descriptor == self.inner => match error
                {
                    Some(err) => return Err(Error::from_nserror(err)),
                    None => return self.value().await,
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

    /// Write the value of this descriptor on the device to `value`
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        let mut receiver = self.delegate.sender().new_receiver();
        let service = self.inner.dispatch(|descriptor| {
            let service = unsafe { descriptor.characteristic().and_then(|x| x.service()) }.ok_or(Error::new(
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
            unsafe { peripheral.writeValue_forDescriptor(&data, descriptor) };
            Ok(unsafe { Dispatched::new(service) })
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DescriptorValueWrite { descriptor, error } if descriptor == self.inner => {
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
}
