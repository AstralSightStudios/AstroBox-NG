use std::pin::pin;

use futures_channel::mpsc;
use futures_core::Stream;
use futures_lite::{future, StreamExt};
use tracing::error;
use windows::core::{GUID, HSTRING};
use windows::Devices::Bluetooth::{
    BluetoothAddressType, BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
};
use windows::Devices::Enumeration::{DevicePairingKinds, DevicePairingRequestedEventArgs};
use windows::Foundation::TypedEventHandler;

use super::error::{check_communication_status, check_pairing_status, check_unpairing_status};
use crate::device::ServicesChanged;
use crate::error::ErrorKind;
use crate::pairing::{IoCapability, PairingAgent, Passkey};
use crate::util::defer;
use crate::{Device, DeviceId, Error, Result, Service, Uuid};

/// A Bluetooth LE device
#[derive(Clone)]
pub struct DeviceImpl {
    pub(super) inner: BluetoothLEDevice,
}

impl PartialEq for DeviceImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.DeviceId() == other.inner.DeviceId()
    }
}

impl Eq for DeviceImpl {}

impl std::hash::Hash for DeviceImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.DeviceId().unwrap().to_os_string().hash(state);
    }
}

impl std::fmt::Debug for DeviceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("Device");
        f.field("id", &self.id());
        if let Ok(name) = self.name() {
            f.field("name", &name);
        }
        f.finish()
    }
}

impl std::fmt::Display for DeviceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name().as_deref().unwrap_or("(Unknown)"))
    }
}

impl Device {
    pub(super) async fn from_addr(addr: u64, kind: Option<BluetoothAddressType>) -> windows::core::Result<Self> {
        let inner = if let Some(kind) = kind {
            BluetoothLEDevice::FromBluetoothAddressWithBluetoothAddressTypeAsync(addr, kind)?.await?
        } else {
            BluetoothLEDevice::FromBluetoothAddressAsync(addr)?.await?
        };
        Ok(Device(DeviceImpl { inner }))
    }

    pub(super) async fn from_id(id: &HSTRING) -> windows::core::Result<Self> {
        let inner = BluetoothLEDevice::FromIdAsync(id)?.await?;
        Ok(Device(DeviceImpl { inner }))
    }
}

impl DeviceImpl {
    /// This device's unique identifier
    pub fn id(&self) -> DeviceId {
        super::DeviceId(
            self.inner
                .DeviceId()
                .expect("error getting DeviceId for BluetoothLEDevice")
                .to_os_string(),
        )
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    pub fn name(&self) -> Result<String> {
        let name = self.inner.Name()?;
        Ok(name.to_string_lossy())
    }

    /// The local name for this device, if available
    ///
    /// This can either be a name advertised or read from the device, or a name assigned to the device by the OS.
    pub async fn name_async(&self) -> Result<String> {
        self.name()
    }

    /// The connection status for this device
    pub async fn is_connected(&self) -> bool {
        self.inner.ConnectionStatus() == Ok(BluetoothConnectionStatus::Connected)
    }

    /// The pairing status for this device
    pub async fn is_paired(&self) -> Result<bool> {
        self.inner
            .DeviceInformation()?
            .Pairing()?
            .IsPaired()
            .map_err(Into::into)
    }

    /// Attempt to pair this device using the system default pairing UI
    ///
    /// This will fail unless it is called from a UWP application.
    pub async fn pair(&self) -> Result<()> {
        let op = self.inner.DeviceInformation()?.Pairing()?.PairAsync()?;
        let res = op.await?;
        check_pairing_status(res.Status()?)
    }

    /// Attempt to pair this device using the system default pairing UI
    pub async fn pair_with_agent<T: PairingAgent>(&self, agent: &T) -> Result<()> {
        let pairing_kinds_supported = match agent.io_capability() {
            IoCapability::DisplayOnly => DevicePairingKinds::DisplayPin,
            IoCapability::DisplayYesNo => {
                DevicePairingKinds::ConfirmOnly | DevicePairingKinds::DisplayPin | DevicePairingKinds::ConfirmPinMatch
            }
            IoCapability::KeyboardOnly => DevicePairingKinds::ConfirmOnly | DevicePairingKinds::ProvidePin,
            IoCapability::NoInputNoOutput => DevicePairingKinds::ConfirmOnly,
            IoCapability::KeyboardDisplay => {
                DevicePairingKinds::ConfirmOnly
                    | DevicePairingKinds::DisplayPin
                    | DevicePairingKinds::ProvidePin
                    | DevicePairingKinds::ConfirmPinMatch
            }
        };

        let (mut tx, mut rx) = mpsc::channel(1);
        let custom = self.inner.DeviceInformation()?.Pairing()?.Custom()?;
        custom.PairingRequested(&TypedEventHandler::new(
            move |_custom, event_args: &Option<DevicePairingRequestedEventArgs>| {
                if let Some(event_args) = event_args.clone() {
                    let deferral = event_args.GetDeferral()?;
                    let _ = tx.try_send((event_args, deferral));
                }
                Ok(())
            },
        ))?;

        let op = custom.PairAsync(pairing_kinds_supported)?;

        let device = Device(self.clone());
        let pairing_fut = pin!(async move {
            while let Some((event_args, deferral)) = rx.next().await {
                match event_args.PairingKind()? {
                    DevicePairingKinds::ConfirmOnly => {
                        if agent.confirm(&device).await.is_ok() {
                            event_args.Accept()?;
                        }
                    }
                    DevicePairingKinds::DisplayPin => {
                        if let Ok(passkey) = event_args.Pin()?.to_string_lossy().parse::<Passkey>() {
                            agent.display_passkey(&device, passkey);
                        }
                    }
                    DevicePairingKinds::ProvidePin => {
                        if let Ok(passkey) = agent.request_passkey(&device).await {
                            event_args.AcceptWithPin(&passkey.to_string().into())?;
                        }
                    }
                    DevicePairingKinds::ConfirmPinMatch => {
                        if let Ok(passkey) = event_args.Pin()?.to_string_lossy().parse::<Passkey>() {
                            if let Ok(()) = agent.confirm_passkey(&device, passkey).await {
                                event_args.Accept()?;
                            }
                        }
                    }
                    _ => (),
                }

                deferral.Complete()?;
            }

            Result::<_, Error>::Ok(())
        });

        let op = async move {
            let res = op.await;
            check_pairing_status(res?.Status()?)
        };
        let pairing_fut = async move {
            pairing_fut.await.and_then(|_| {
                Err(Error::new(
                    ErrorKind::Other,
                    None,
                    "Pairing agent terminated unexpectedly".to_owned(),
                ))
            })
        };
        future::or(op, pairing_fut).await
    }

    /// Disconnect and unpair this device from the system
    pub async fn unpair(&self) -> Result<()> {
        let op = self.inner.DeviceInformation()?.Pairing()?.UnpairAsync()?;
        let res = op.await?;
        check_unpairing_status(res.Status()?)
    }

    /// Discover the primary services of this device.
    pub async fn discover_services(&self) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }

    /// Discover the primary service(s) of this device with the given [`Uuid`].
    pub async fn discover_services_with_uuid(&self, uuid: Uuid) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetGattServicesForUuidWithCacheModeAsync(GUID::from_u128(uuid.as_u128()), BluetoothCacheMode::Uncached)?
            .await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "discovering services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }

    /// Get previously discovered services.
    ///
    /// If no services have been discovered yet, this method will perform service discovery.
    pub async fn services(&self) -> Result<Vec<Service>> {
        let res = self
            .inner
            .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Cached)?
            .await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering services")?;
        let services = res.Services()?;
        Ok(services.into_iter().map(Service::new).collect())
    }

    /// Monitors the device for services changed events.
    pub async fn service_changed_indications(
        &self,
    ) -> Result<impl Stream<Item = Result<ServicesChanged>> + Send + Unpin + '_> {
        let (mut sender, receiver) = futures_channel::mpsc::channel(16);
        let token = self.inner.GattServicesChanged(&TypedEventHandler::new(move |_, _| {
            if let Err(err) = sender.try_send(Ok(ServicesChanged(ServicesChangedImpl))) {
                error!("Error sending service changed indication: {:?}", err);
            }
            Ok(())
        }))?;

        let guard = defer(move || {
            if let Err(err) = self.inner.RemoveGattServicesChanged(token) {
                error!("Error removing state changed handler: {:?}", err);
            }
        });

        Ok(receiver.map(move |x| {
            let _guard = &guard;
            x
        }))
    }

    /// Get the current signal strength from the device in dBm.
    ///
    /// Returns [ErrorKind::NotSupported].
    pub async fn rssi(&self) -> Result<i16> {
        Err(ErrorKind::NotSupported.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServicesChangedImpl;

impl ServicesChangedImpl {
    pub fn was_invalidated(&self, _service: &Service) -> bool {
        true
    }
}
