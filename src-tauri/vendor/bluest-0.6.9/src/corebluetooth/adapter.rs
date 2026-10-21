#![allow(clippy::let_unit_value)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_core::Stream;
use futures_lite::{stream, StreamExt};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, Message};
use objc2_core_bluetooth::{CBCentralManager, CBManager, CBManagerAuthorization, CBManagerState, CBUUID};
use objc2_foundation::{NSArray, NSData, NSUUID};
use tracing::{debug, error, info, warn};

use super::delegates::{self, CentralDelegate};
use super::dispatch::{self, Dispatched};
use crate::error::ErrorKind;
use crate::util::defer;
use crate::{
    AdapterEvent, AdvertisingDevice, BluetoothUuidExt, ConnectionEvent, Device, DeviceId, Error, Result, Uuid,
};

/// The system's Bluetooth adapter interface.
///
/// The default adapter for the system may be accessed with the [`Adapter::default()`] method.
#[derive(Clone)]
pub struct AdapterImpl {
    central: Dispatched<CBCentralManager>,
    delegate: Retained<CentralDelegate>,
    scanning: Arc<AtomicBool>,
    #[cfg(not(target_os = "macos"))]
    registered_connection_events: Arc<std::sync::Mutex<std::collections::HashMap<DeviceId, usize>>>,
}

impl PartialEq for AdapterImpl {
    fn eq(&self, other: &Self) -> bool {
        self.central == other.central
    }
}

impl Eq for AdapterImpl {}

impl std::hash::Hash for AdapterImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.central.hash(state);
    }
}

impl std::fmt::Debug for AdapterImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Adapter").field(&self.central).finish()
    }
}

impl AdapterImpl {
    /// Creates an interface to the default Bluetooth adapter for the system
    pub async fn default() -> Option<Self> {
        match unsafe { CBManager::authorization_class() } {
            CBManagerAuthorization::AllowedAlways => info!("Bluetooth authorization is allowed"),
            CBManagerAuthorization::Denied => error!("Bluetooth authorization is denied"),
            CBManagerAuthorization::NotDetermined => {
                warn!("Bluetooth authorization is undetermined")
            }
            CBManagerAuthorization::Restricted => warn!("Bluetooth authorization is restricted"),
            val => error!("Bluetooth authorization returned unknown value {:?}", val),
        }

        let delegate = CentralDelegate::new();
        let protocol = ProtocolObject::from_retained(delegate.clone());
        let central = unsafe {
            let this = CBCentralManager::alloc();
            // let options = NSDictionary::from_slices(&[CBCentralManagerOptionShowPowerAlertKey], &[NSNumber::numberWithBool(config.request_permissions)]);
            CBCentralManager::initWithDelegate_queue(this, Some(&protocol), Some(dispatch::queue()))
        };
        let central = unsafe { Dispatched::new(central) };

        Some(AdapterImpl {
            central,
            delegate,
            scanning: Default::default(),
            #[cfg(not(target_os = "macos"))]
            registered_connection_events: Default::default(),
        })
    }

    /// A stream of [`AdapterEvent`] which allows the application to identify when the adapter is enabled or disabled.
    pub async fn events(&self) -> Result<impl Stream<Item = Result<AdapterEvent>> + Send + Unpin + '_> {
        let receiver = self.delegate.sender().new_receiver();
        Ok(receiver.filter_map(|x| {
            match x {
                delegates::CentralEvent::StateChanged => {
                    // TODO: Check CBCentralManager::authorization()?
                    let state = self.state();
                    debug!("Central state is now {:?}", state);
                    match state {
                        CBManagerState::PoweredOn => Some(Ok(AdapterEvent::Available)),
                        _ => Some(Ok(AdapterEvent::Unavailable)),
                    }
                }
                _ => None,
            }
        }))
    }

    fn state(&self) -> CBManagerState {
        self.central.dispatch(|central| unsafe { central.state() })
    }

    /// Asynchronously blocks until the adapter is available
    pub async fn wait_available(&self) -> Result<()> {
        let events = self.events();
        if self.state() != CBManagerState::PoweredOn {
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
        self.central.dispatch(|central| unsafe {
            let identifiers = NSArray::from_retained_slice(&[NSUUID::from_bytes(*id.0.as_bytes())]);
            let peripherals = central.retrievePeripheralsWithIdentifiers(&identifiers);

            peripherals
                .iter()
                .next()
                .map(Device::new)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, None, "opening device"))
        })
    }

    /// Finds all connected Bluetooth LE devices
    pub async fn connected_devices(&self) -> Result<Vec<Device>> {
        self.connected_devices_with_services(&[crate::btuuid::services::GENERIC_ATTRIBUTE])
            .await
    }

    /// Finds all connected devices providing any service in `services`
    ///
    /// # Panics
    ///
    /// Panics if `services` is empty.
    pub async fn connected_devices_with_services(&self, services: &[Uuid]) -> Result<Vec<Device>> {
        assert!(!services.is_empty());

        self.central.dispatch(|central| {
            let services = {
                let vec = services
                    .iter()
                    .copied()
                    .map(|s| unsafe { CBUUID::UUIDWithData(&NSData::with_bytes(s.as_bluetooth_bytes())) })
                    .collect::<Vec<_>>();
                NSArray::from_retained_slice(&vec[..])
            };
            unsafe {
                let peripherals = central.retrieveConnectedPeripheralsWithServices(&services);
                Ok(peripherals.iter().map(Device::new).collect())
            }
        })
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
        if self.state() != CBManagerState::PoweredOn {
            return Err(Error::from(ErrorKind::AdapterUnavailable));
        }

        if self.scanning.swap(true, Ordering::Acquire) {
            return Err(ErrorKind::AlreadyScanning.into());
        }

        let receiver = self.delegate.sender().new_receiver();
        self.central.dispatch(|central| {
            let services = (!services.is_empty()).then(|| {
                let vec = services
                    .iter()
                    .copied()
                    .map(|s| unsafe { CBUUID::UUIDWithData(&NSData::with_bytes(s.as_bluetooth_bytes())) })
                    .collect::<Vec<_>>();
                NSArray::from_retained_slice(&vec[..])
            });

            unsafe { central.scanForPeripheralsWithServices_options(services.as_deref(), None) };
        });

        let guard = defer(|| {
            self.central.dispatch(|central| unsafe { central.stopScan() });
            self.scanning.store(false, Ordering::Release);
        });

        let events = receiver
            .take_while(|_| self.state() == CBManagerState::PoweredOn)
            .filter_map(move |x| {
                let _guard = &guard;
                match x {
                    delegates::CentralEvent::Discovered {
                        peripheral,
                        adv_data,
                        rssi,
                    } => peripheral.dispatch(|peripheral| {
                        Some(AdvertisingDevice {
                            device: Device::new(peripheral.retain()),
                            adv_data,
                            rssi: Some(rssi),
                        })
                    }),
                    _ => None,
                }
            });

        Ok(events)
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
        let connected = stream::iter(self.connected_devices_with_services(services).await?).map(Ok);

        // try_unfold is used to ensure we do not start scanning until the connected devices have been consumed
        let advertising = Box::pin(stream::try_unfold(None, |state| async {
            let mut stream = match state {
                Some(stream) => stream,
                None => self.scan(services).await?,
            };
            Ok(stream.next().await.map(|x| (x.device, Some(stream))))
        }));

        Ok(connected.chain(advertising))
    }

    /// Connects to the [`Device`]
    ///
    /// This method must be called before any methods on the [`Device`] which require a connection are called. After a
    /// successful return from this method, a connection has been established with the device (if one did not already
    /// exist) and the application can then interact with the device. This connection will be maintained until either
    /// [`disconnect_device`][Self::disconnect_device] is called or the `Adapter` is dropped.
    pub async fn connect_device(&self, device: &Device) -> Result<()> {
        if self.state() != CBManagerState::PoweredOn {
            return Err(ErrorKind::AdapterUnavailable.into());
        }

        let mut events = self.delegate.sender().new_receiver();
        debug!("Connecting to {:?}", device);
        self.central
            .dispatch(|central| unsafe { central.connectPeripheral_options(device.0.peripheral.get(), None) });

        let drop = defer(|| {
            self.central.dispatch(|central| unsafe {
                central.cancelPeripheralConnection(device.0.peripheral.get());
            })
        });

        while let Some(event) = events.next().await {
            if self.state() != CBManagerState::PoweredOn {
                drop.defuse();
                return Err(ErrorKind::AdapterUnavailable.into());
            }
            match event {
                delegates::CentralEvent::Connect { peripheral } if peripheral == device.0.peripheral => {
                    drop.defuse();
                    return Ok(());
                }
                delegates::CentralEvent::ConnectFailed { peripheral, error } if peripheral == device.0.peripheral => {
                    drop.defuse();
                    return Err(error.map_or(ErrorKind::ConnectionFailed.into(), Error::from_nserror));
                }
                _ => (),
            }
        }

        unreachable!()
    }

    /// Disconnects from the [`Device`]
    ///
    /// Once this method is called, the application will no longer have access to the [`Device`] and any methods
    /// which would require a connection will fail. If no other application has a connection to the same device,
    /// the underlying Bluetooth connection will be closed.
    pub async fn disconnect_device(&self, device: &Device) -> Result<()> {
        if self.state() != CBManagerState::PoweredOn {
            return Err(ErrorKind::AdapterUnavailable.into());
        }

        let mut events = self.delegate.sender().new_receiver();
        debug!("Disconnecting from {:?}", device);
        self.central
            .dispatch(|central| unsafe { central.cancelPeripheralConnection(device.0.peripheral.get()) });
        while let Some(event) = events.next().await {
            if self.state() != CBManagerState::PoweredOn {
                return Err(ErrorKind::AdapterUnavailable.into());
            }
            match event {
                delegates::CentralEvent::Disconnect {
                    peripheral,
                    error: None,
                } if peripheral == device.0.peripheral => return Ok(()),
                delegates::CentralEvent::Disconnect {
                    peripheral,
                    error: Some(err),
                } if peripheral == device.0.peripheral => return Err(Error::from_nserror(err)),
                _ => (),
            }
        }

        unreachable!()
    }

    #[cfg(not(target_os = "macos"))]
    fn register_connection_events(&self, device: DeviceId) -> impl Drop + '_ {
        use std::collections::HashMap;

        use objc2::rc::Retained;
        use objc2_core_bluetooth::CBConnectionEventMatchingOptionPeripheralUUIDs;
        use objc2_foundation::{NSDictionary, NSString};

        fn options(devices: &HashMap<DeviceId, usize>) -> Retained<NSDictionary<NSString>> {
            let ids: Vec<Retained<NSUUID>> = devices.keys().map(|x| NSUUID::from_bytes(*x.0.as_bytes())).collect();
            let ids = NSArray::from_retained_slice(&ids[..]);
            NSDictionary::from_retained_objects(
                &[unsafe { CBConnectionEventMatchingOptionPeripheralUUIDs }],
                &[ids.into()],
            )
        }

        self.central.dispatch(|central| {
            let mut guard = self.registered_connection_events.lock().unwrap();

            match guard.entry(device.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    *e.get_mut() += 1;
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(1);
                    let opts = options(&guard);
                    unsafe { central.registerForConnectionEventsWithOptions(Some(&opts)) }
                }
            }
        });

        defer(move || {
            self.central.dispatch(|central| {
                let mut guard = self.registered_connection_events.lock().unwrap();
                match guard.entry(device) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        *e.get_mut() -= 1;
                        if *e.get() == 0 {
                            e.remove();
                            let opts = options(&guard);
                            unsafe { central.registerForConnectionEventsWithOptions(Some(&opts)) }
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(_) => unreachable!(),
                }
            })
        })
    }

    /// Monitors a device for connection/disconnection events.
    ///
    /// # Platform specifics
    ///
    /// ## MacOS/iOS
    ///
    /// Available on iOS/iPadOS only. On MacOS no events will be generated.
    #[cfg(not(target_os = "macos"))]
    pub async fn device_connection_events<'a>(
        &'a self,
        device: &'a Device,
    ) -> Result<impl Stream<Item = ConnectionEvent> + Send + Unpin + 'a> {
        let events = self.delegate.sender().new_receiver();
        let guard = self.register_connection_events(device.id());

        let id = device
            .0
            .peripheral
            .dispatch(|peripheral| unsafe { peripheral.identifier() });

        Ok(events
            .take_while(|_| self.state() == CBManagerState::PoweredOn)
            .filter_map(move |x| {
                let _guard = &guard;
                match x {
                    delegates::CentralEvent::Connect { peripheral }
                        if peripheral.dispatch(|peripheral| unsafe { peripheral.identifier() } == id) =>
                    {
                        Some(ConnectionEvent::Connected)
                    }
                    delegates::CentralEvent::Disconnect { peripheral, .. }
                        if peripheral.dispatch(|peripheral| unsafe { peripheral.identifier() } == id) =>
                    {
                        Some(ConnectionEvent::Disconnected)
                    }
                    delegates::CentralEvent::ConnectionEvent { peripheral, event }
                        if peripheral.dispatch(|peripheral| unsafe { peripheral.identifier() } == id) =>
                    {
                        Some(event)
                    }
                    _ => None,
                }
            }))
    }

    /// Monitors a device for connection/disconnection events.
    ///
    /// # Platform specifics
    ///
    /// ## MacOS/iOS
    ///
    /// Available on iOS/iPadOS only. On MacOS no events will be generated.
    #[cfg(target_os = "macos")]
    pub async fn device_connection_events<'a>(
        &'a self,
        device: &'a Device,
    ) -> Result<impl Stream<Item = ConnectionEvent> + Send + Unpin + 'a> {
        let events = self.delegate.sender().new_receiver();
        Ok(events
            .take_while(|_| self.state() == CBManagerState::PoweredOn)
            .filter_map(move |x| match x {
                delegates::CentralEvent::Connect { peripheral } if peripheral == device.0.peripheral => {
                    Some(ConnectionEvent::Connected)
                }
                delegates::CentralEvent::Disconnect { peripheral, .. } if peripheral == device.0.peripheral => {
                    Some(ConnectionEvent::Disconnected)
                }
                _ => None,
            }))
    }
}
