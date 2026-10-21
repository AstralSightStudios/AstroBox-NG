#![allow(clippy::let_unit_value)]

use futures_core::Stream;

use crate::{sys, AdapterEvent, AdvertisingDevice, ConnectionEvent, Device, DeviceId, Result, Uuid};

/// The system's Bluetooth adapter interface.
///
/// The default adapter for the system may be accessed with the [`Adapter::default()`] method.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Adapter(sys::adapter::AdapterImpl);

impl Adapter {
    /// Creates an interface to the default Bluetooth adapter for the system.
    ///
    /// # Safety
    ///
    /// - `java_vm` must be a valid JNI `JavaVM` pointer to a VM that will stay alive for the entire duration the `Adapter` or any structs obtained from it are live.
    /// - `bluetooth_manager` must be a valid global reference to an `android.bluetooth.BluetoothManager` instance, from the `java_vm` VM.
    /// - The `Adapter` takes ownership of the global reference and will delete it with the `DeleteGlobalRef` JNI call when dropped. You must not do that yourself.
    #[cfg(target_os = "android")]
    pub unsafe fn new(
        java_vm: *mut java_spaghetti::sys::JavaVM,
        bluetooth_manager: java_spaghetti::sys::jobject,
    ) -> Result<Self> {
        sys::adapter::AdapterImpl::new(java_vm, bluetooth_manager).map(Self)
    }

    /// Creates an interface to the default Bluetooth adapter for the system
    #[inline]
    #[cfg(not(target_os = "android"))]
    pub async fn default() -> Option<Self> {
        sys::adapter::AdapterImpl::default().await.map(Adapter)
    }

    /// A stream of [`AdapterEvent`] which allows the application to identify when the adapter is enabled or disabled.
    #[inline]
    pub async fn events(&self) -> Result<impl Stream<Item = Result<AdapterEvent>> + Send + Unpin + '_> {
        self.0.events().await
    }

    /// Asynchronously blocks until the adapter is available
    #[inline]
    pub async fn wait_available(&self) -> Result<()> {
        self.0.wait_available().await
    }

    /// Attempts to create the device identified by `id`
    #[inline]
    pub async fn open_device(&self, id: &DeviceId) -> Result<Device> {
        self.0.open_device(id).await
    }

    /// Finds all connected Bluetooth LE devices
    #[inline]
    pub async fn connected_devices(&self) -> Result<Vec<Device>> {
        self.0.connected_devices().await
    }

    /// Finds all connected devices providing any service in `services`
    ///
    /// # Panics
    ///
    /// Panics if `services` is empty.
    #[inline]
    pub async fn connected_devices_with_services(&self, services: &[Uuid]) -> Result<Vec<Device>> {
        self.0.connected_devices_with_services(services).await
    }

    /// Starts scanning for Bluetooth advertising packets.
    ///
    /// Returns a stream of [`AdvertisingDevice`] structs which contain the data from the advertising packet and the
    /// [`Device`] which sent it. Scanning is automatically stopped when the stream is dropped. Inclusion of duplicate
    /// packets is a platform-specific implementation detail.
    ///
    /// If `services` is not empty, returns advertisements including at least one GATT service with a UUID in
    /// `services`. Otherwise returns all advertisements.
    #[inline]
    pub async fn scan<'a>(
        &'a self,
        services: &'a [Uuid],
    ) -> Result<impl Stream<Item = AdvertisingDevice> + Send + Unpin + 'a> {
        self.0.scan(services).await
    }

    /// Finds Bluetooth devices providing any service in `services`.
    ///
    /// Returns a stream of [`Device`] structs with matching connected devices returned first. If the stream is not
    /// dropped before all matching connected devices are consumed then scanning will begin for devices advertising any
    /// of the `services`. Scanning will continue until the stream is dropped. Inclusion of duplicate devices is a
    /// platform-specific implementation detail.
    #[inline]
    pub async fn discover_devices<'a>(
        &'a self,
        services: &'a [Uuid],
    ) -> Result<impl Stream<Item = Result<Device>> + Send + Unpin + 'a> {
        self.0.discover_devices(services).await
    }

    /// Connects to the [`Device`]
    ///
    /// # Platform specifics
    ///
    /// ## MacOS/iOS
    ///
    /// This method must be called before any methods on the [`Device`] which require a connection are called. After a
    /// successful return from this method, a connection has been established with the device (if one did not already
    /// exist) and the application can then interact with the device. This connection will be maintained until either
    /// [`disconnect_device`][Self::disconnect_device] is called or the `Adapter` is dropped.
    ///
    /// ## Windows
    ///
    /// On Windows, device connections are automatically managed by the OS. This method has no effect. Instead, a
    /// connection will automatically be established, if necessary, when methods on the device requiring a connection
    /// are called.
    ///
    /// ## Linux
    ///
    /// If the device is not yet connected to the system, this method must be called before any methods on the
    /// [`Device`] which require a connection are called.  After a successful return from this method, a connection has
    /// been established with the device (if one did not already exist) and the application can then interact with the
    /// device. This connection will be maintained until [`disconnect_device`][Self::disconnect_device] is called.
    #[inline]
    pub async fn connect_device(&self, device: &Device) -> Result<()> {
        self.0.connect_device(device).await
    }

    /// Disconnects from the [`Device`]
    ///
    /// # Platform specifics
    ///
    /// ## MacOS/iOS
    ///
    /// Once this method is called, the application will no longer have access to the [`Device`] and any methods
    /// which would require a connection will fail. If no other application has a connection to the same device,
    /// the underlying Bluetooth connection will be closed.
    ///
    /// ## Windows
    ///
    /// On Windows, device connections are automatically managed by the OS. This method has no effect. Instead, the
    /// connection will be closed only when the [`Device`] and all its child objects are dropped.
    ///
    /// ## Linux
    ///
    /// This method disconnects the device from the system, even if other applications are using the device.
    #[inline]
    pub async fn disconnect_device(&self, device: &Device) -> Result<()> {
        self.0.disconnect_device(device).await
    }

    /// Monitors a device for connection/disconnection events.
    ///
    /// # Platform specifics
    ///
    /// ## MacOS/iOS
    ///
    /// On MacOS connection events will only be generated for calls to `connect_device` and disconnection events
    /// will only be generated for devices that have been connected with `connect_device`.
    ///
    /// On iOS/iPadOS connection and disconnection events can be generated for any device.
    #[inline]
    pub async fn device_connection_events<'a>(
        &'a self,
        device: &'a Device,
    ) -> Result<impl Stream<Item = ConnectionEvent> + Send + Unpin + 'a> {
        self.0.device_connection_events(device).await
    }
}
