use objc2::rc::Retained;
use objc2_core_bluetooth::{CBPeripheralState, CBService, CBUUID};
use objc2_foundation::{NSArray, NSData};

use super::delegates::{PeripheralDelegate, PeripheralEvent};
use super::dispatch::Dispatched;
use crate::error::ErrorKind;
use crate::{BluetoothUuidExt, Characteristic, Error, Result, Service, Uuid};

/// A Bluetooth GATT service
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceImpl {
    pub(super) inner: Dispatched<CBService>,
    delegate: Retained<PeripheralDelegate>,
}

impl Service {
    pub(super) fn new(service: Retained<CBService>, delegate: Retained<PeripheralDelegate>) -> Self {
        Service(ServiceImpl {
            inner: unsafe { Dispatched::new(service) },
            delegate,
        })
    }
}

impl ServiceImpl {
    /// The [`Uuid`] identifying the type of this GATT service
    pub fn uuid(&self) -> Uuid {
        self.inner
            .dispatch(|service| unsafe { Uuid::from_bluetooth_bytes(service.UUID().data().as_bytes_unchecked()) })
    }

    /// The [`Uuid`] identifying the type of this GATT service
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(self.uuid())
    }

    /// Whether this is a primary service of the device.
    pub async fn is_primary(&self) -> Result<bool> {
        self.inner.dispatch(|service| unsafe { Ok(service.isPrimary()) })
    }

    /// Discover all characteristics associated with this service.
    pub async fn discover_characteristics(&self) -> Result<Vec<Characteristic>> {
        self.discover_characteristics_inner(None).await
    }

    /// Discover the characteristic(s) with the given [`Uuid`].
    pub async fn discover_characteristics_with_uuid(&self, uuid: Uuid) -> Result<Vec<Characteristic>> {
        let characteristics = self.discover_characteristics_inner(Some(uuid)).await?;
        Ok(characteristics.into_iter().filter(|x| x.uuid() == uuid).collect())
    }

    async fn discover_characteristics_inner(&self, uuid: Option<Uuid>) -> Result<Vec<Characteristic>> {
        let mut receiver = self.delegate.sender().new_receiver();
        self.inner.dispatch(|service| {
            let uuids = uuid.map(|uuid| unsafe {
                NSArray::from_retained_slice(&[CBUUID::UUIDWithData(&NSData::with_bytes(uuid.as_bluetooth_bytes()))])
            });

            let peripheral = unsafe {
                service
                    .peripheral()
                    .ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?
            };

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.discoverCharacteristics_forService(uuids.as_deref(), service) };
            Ok(())
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DiscoveredCharacteristics { service, error } if service == self.inner => match error {
                    Some(err) => return Err(Error::from_nserror(err)),
                    None => break,
                },
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&self.inner) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }

        self.characteristics_inner()
    }

    /// Get previously discovered characteristics.
    ///
    /// If no characteristics have been discovered yet, this method will perform characteristic discovery.
    pub async fn characteristics(&self) -> Result<Vec<Characteristic>> {
        match self.characteristics_inner() {
            Ok(characteristics) => Ok(characteristics),
            Err(_) => self.discover_characteristics().await,
        }
    }

    fn characteristics_inner(&self) -> Result<Vec<Characteristic>> {
        self.inner.dispatch(|service| {
            unsafe { service.characteristics() }
                .map(|s| {
                    s.iter()
                        .map(|x| Characteristic::new(x, self.delegate.clone()))
                        .collect()
                })
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "no characteristics have been discovered"))
        })
    }

    /// Discover the included services of this service.
    pub async fn discover_included_services(&self) -> Result<Vec<Service>> {
        self.discover_included_services_inner(None).await
    }

    /// Discover the included service(s) with the given [`Uuid`].
    pub async fn discover_included_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        let services = self.discover_included_services_inner(Some(uuid)).await?;
        Ok(services.into_iter().filter(|x| x.uuid() == uuid).collect())
    }

    async fn discover_included_services_inner(&self, uuid: Option<Uuid>) -> Result<Vec<Service>> {
        let mut receiver = self.delegate.sender().new_receiver();
        self.inner.dispatch(|service| {
            let uuids = uuid.map(|uuid| unsafe {
                NSArray::from_retained_slice(&[CBUUID::UUIDWithData(&NSData::with_bytes(uuid.as_bluetooth_bytes()))])
            });

            let peripheral = unsafe {
                service
                    .peripheral()
                    .ok_or(Error::new(ErrorKind::NotFound, None, "peripheral not found"))?
            };

            if unsafe { peripheral.state() } != CBPeripheralState::Connected {
                return Err(Error::from(ErrorKind::NotConnected));
            }

            unsafe { peripheral.discoverIncludedServices_forService(uuids.as_deref(), service) };
            Ok(())
        })?;

        loop {
            match receiver.recv().await.map_err(Error::from_recv_error)? {
                PeripheralEvent::DiscoveredIncludedServices { service, error } if service == self.inner => {
                    match error {
                        Some(err) => return Err(Error::from_nserror(err)),
                        None => break,
                    }
                }
                PeripheralEvent::Disconnected { error } => {
                    return Err(Error::from_kind_and_nserror(ErrorKind::NotConnected, error));
                }
                PeripheralEvent::ServicesChanged { invalidated_services }
                    if invalidated_services.contains(&self.inner) =>
                {
                    return Err(ErrorKind::ServiceChanged.into());
                }
                _ => (),
            }
        }

        self.included_services_inner()
    }

    /// Get previously discovered included services.
    ///
    /// If no included services have been discovered yet, this method will perform included service discovery.
    pub async fn included_services(&self) -> Result<Vec<Service>> {
        match self.included_services_inner() {
            Ok(services) => Ok(services),
            Err(_) => self.discover_included_services().await,
        }
    }

    fn included_services_inner(&self) -> Result<Vec<Service>> {
        self.inner.dispatch(|service| {
            unsafe { service.includedServices() }
                .map(|s| s.iter().map(|x| Service::new(x, self.delegate.clone())).collect())
                .ok_or_else(|| Error::new(ErrorKind::NotReady, None, "no included services have been discovered"))
        })
    }
}
