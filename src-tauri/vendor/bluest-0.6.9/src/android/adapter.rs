use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_channel::{Receiver, Sender};
use futures_core::Stream;
use futures_lite::{stream, StreamExt};
use java_spaghetti::{Arg, ByteArray, Env, Global, Local, Null, PrimitiveArray, VM};
use tracing::{debug, warn};
use uuid::Uuid;

use super::bindings::android::bluetooth::le::{BluetoothLeScanner, ScanResult, ScanSettings, ScanSettings_Builder};
use super::bindings::android::bluetooth::{BluetoothAdapter, BluetoothManager};
use super::bindings::android::os::ParcelUuid;
use super::bindings::com::github::alexmoon::bluest::android::BluestScanCallback;
use super::device::DeviceImpl;
use super::{JavaIterator, OptionExt};
use crate::android::bindings::java::util::Map_Entry;
use crate::util::defer;
use crate::{
    AdapterEvent, AdvertisementData, AdvertisingDevice, ConnectionEvent, Device, DeviceId, ManufacturerData, Result,
};

struct AdapterInner {
    manager: Global<BluetoothManager>,
    _adapter: Global<BluetoothAdapter>,
    le_scanner: Global<BluetoothLeScanner>,
}

#[derive(Clone)]
pub struct AdapterImpl {
    inner: Arc<AdapterInner>,
}

impl AdapterImpl {
    pub unsafe fn new(vm: *mut java_spaghetti::sys::JavaVM, manager: java_spaghetti::sys::jobject) -> Result<Self> {
        let vm = VM::from_raw(vm);
        let manager: Global<BluetoothManager> = Global::from_raw(vm, manager);

        vm.with_env(|env| {
            let local_manager = manager.as_ref(env);
            let adapter = local_manager.getAdapter()?.non_null()?;
            let le_scanner = adapter.getBluetoothLeScanner()?.non_null()?;

            Ok(Self {
                inner: Arc::new(AdapterInner {
                    _adapter: adapter.as_global(),
                    le_scanner: le_scanner.as_global(),
                    manager: manager.clone(),
                }),
            })
        })
    }

    pub(crate) async fn events(&self) -> Result<impl Stream<Item = Result<AdapterEvent>> + Send + Unpin + '_> {
        Ok(stream::empty()) // TODO
    }

    pub async fn wait_available(&self) -> Result<()> {
        Ok(())
    }

    pub async fn open_device(&self, _id: &DeviceId) -> Result<Device> {
        todo!()
    }

    pub async fn connected_devices(&self) -> Result<Vec<Device>> {
        todo!()
    }

    pub async fn connected_devices_with_services(&self, _services: &[Uuid]) -> Result<Vec<Device>> {
        todo!()
    }

    pub async fn scan<'a>(
        &'a self,
        _services: &'a [Uuid],
    ) -> Result<impl Stream<Item = AdvertisingDevice> + Send + Unpin + 'a> {
        self.inner.manager.vm().with_env(|env| {
            let receiver = SCAN_CALLBACKS.allocate();
            let callback = BluestScanCallback::new(env, receiver.id)?;
            let callback_global = callback.as_global();
            let scanner = self.inner.le_scanner.as_ref(env);
            let settings = ScanSettings_Builder::new(env)?;
            settings.setScanMode(ScanSettings::SCAN_MODE_LOW_LATENCY)?;
            let settings = settings.build()?.non_null()?;
            scanner.startScan_List_ScanSettings_ScanCallback(Null, settings, callback)?;

            let guard = defer(move || {
                self.inner.manager.vm().with_env(|env| {
                    let callback = callback_global.as_ref(env);
                    let scanner = self.inner.le_scanner.as_ref(env);
                    match scanner.stopScan_ScanCallback(callback) {
                        Ok(()) => debug!("stopped scan"),
                        Err(e) => warn!("failed to stop scan: {:?}", e),
                    };
                });
            });

            Ok(Box::pin(receiver).map(move |x| {
                let _guard = &guard;
                x
            }))
        })
    }

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

    pub async fn connect_device(&self, _device: &Device) -> Result<()> {
        // Windows manages the device connection automatically
        Ok(())
    }

    pub async fn disconnect_device(&self, _device: &Device) -> Result<()> {
        // Windows manages the device connection automatically
        Ok(())
    }

    pub async fn device_connection_events<'a>(
        &'a self,
        _device: &'a Device,
    ) -> Result<impl Stream<Item = ConnectionEvent> + Send + Unpin + 'a> {
        Ok(stream::empty()) // TODO
    }
}

impl PartialEq for AdapterImpl {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AdapterImpl {}

impl std::hash::Hash for AdapterImpl {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl std::fmt::Debug for AdapterImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Adapter").finish()
    }
}

static SCAN_CALLBACKS: CallbackRouter<AdvertisingDevice> = CallbackRouter::new();

struct CallbackRouter<T: Send + 'static> {
    map: Mutex<Option<HashMap<i32, Sender<T>>>>,
    next_id: AtomicI32,
}

impl<T: Send + 'static> CallbackRouter<T> {
    const fn new() -> Self {
        Self {
            map: Mutex::new(None),
            next_id: AtomicI32::new(0),
        }
    }

    fn allocate(&'static self) -> CallbackReceiver<T> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = async_channel::bounded(16);
        self.map
            .lock()
            .unwrap()
            .get_or_insert_with(Default::default)
            .insert(id, sender);

        CallbackReceiver {
            router: self,
            id,
            receiver,
        }
    }

    fn callback(&'static self, id: i32, val: T) {
        if let Some(sender) = self.map.lock().unwrap().as_mut().unwrap().get_mut(&id) {
            if let Err(e) = sender.send_blocking(val) {
                warn!("failed to send scan callback: {:?}", e)
            }
        }
    }
}

struct CallbackReceiver<T: Send + 'static> {
    router: &'static CallbackRouter<T>,
    id: i32,
    receiver: Receiver<T>,
}

impl<T: Send + 'static> Stream for CallbackReceiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // safety: this is just a manually-written pin projection.
        let receiver = unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().receiver) };
        receiver.poll_next(cx)
    }
}

impl<T: Send> Drop for CallbackReceiver<T> {
    fn drop(&mut self) {
        self.router.map.lock().unwrap().as_mut().unwrap().remove(&self.id);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_github_alexmoon_bluest_android_BluestScanCallback_nativeOnScanResult(
    env: Env<'_>,
    _class: *mut (), // self class, ignore
    id: i32,
    callback_type: i32,
    scan_result: Arg<ScanResult>,
) {
    if let Err(e) = on_scan_result(env, id, callback_type, scan_result) {
        warn!("on_scan_result failed: {:?}", e);
    }
}

fn convert_uuid(uuid: Local<'_, ParcelUuid>) -> Result<Uuid> {
    let uuid = uuid.getUuid()?.non_null()?;
    let lsb = uuid.getLeastSignificantBits()? as u64;
    let msb = uuid.getMostSignificantBits()? as u64;
    Ok(Uuid::from_u64_pair(msb, lsb))
}

#[no_mangle]
fn on_scan_result(env: Env<'_>, id: i32, callback_type: i32, scan_result: Arg<ScanResult>) -> Result<()> {
    let scan_result = unsafe { scan_result.into_ref(env) }.non_null()?;

    tracing::info!("got callback! {} {}", id, callback_type);

    let scan_record = scan_result.getScanRecord()?.non_null()?;
    let device = scan_result.getDevice()?.non_null()?;

    let address = device.getAddress()?.non_null()?.to_string_lossy();
    let rssi = scan_result.getRssi()?;
    let is_connectable = scan_result.isConnectable()?;
    let local_name = scan_record.getDeviceName()?.map(|s| s.to_string_lossy());
    let tx_power_level = scan_record.getTxPowerLevel()?;

    // Services
    let mut services = Vec::new();
    if let Some(uuids) = scan_record.getServiceUuids()? {
        for uuid in JavaIterator(uuids.iterator()?.non_null()?) {
            services.push(convert_uuid(uuid.cast()?)?)
        }
    }

    // Service data
    let mut service_data = HashMap::new();
    let sd = scan_record.getServiceData()?.non_null()?;
    let sd = sd.entrySet()?.non_null()?;
    for entry in JavaIterator(sd.iterator()?.non_null()?) {
        let entry: Local<Map_Entry> = entry.cast()?;
        let key: Local<ParcelUuid> = entry.getKey()?.non_null()?.cast()?;
        let val: Local<ByteArray> = entry.getValue()?.non_null()?.cast()?;
        service_data.insert(convert_uuid(key)?, val.as_vec().into_iter().map(|i| i as u8).collect());
    }

    // Manufacturer data
    let mut manufacturer_data = None;
    let msd = scan_record.getManufacturerSpecificData()?.non_null()?;
    // TODO there can be multiple manufacturer data entries, but the bluest API only supports one. So grab just the first.
    if msd.size()? != 0 {
        let val: Local<'_, ByteArray> = msd.valueAt(0)?.non_null()?.cast()?;
        manufacturer_data = Some(ManufacturerData {
            company_id: msd.keyAt(0)? as _,
            data: val.as_vec().into_iter().map(|i| i as u8).collect(),
        });
    }

    let device_id = DeviceId(address);

    let d = AdvertisingDevice {
        device: Device(DeviceImpl {
            id: device_id,
            device: device.as_global(),
        }),
        adv_data: AdvertisementData {
            is_connectable,
            local_name,
            manufacturer_data, // TODO, SparseArray is cursed.
            service_data,
            services,
            tx_power_level: Some(tx_power_level as _),
        },
        rssi: Some(rssi as _),
    };
    SCAN_CALLBACKS.callback(id, d);

    Ok(())
}

#[no_mangle]
pub extern "system" fn Java_com_github_alexmoon_bluest_android_BluestScanCallback_nativeOnScanFailed(
    _env: Env<'_>,
    _class: *mut (), // self class, ignore
    id: i32,
    error_code: i32,
) {
    tracing::error!("got scan fail! {} {}", id, error_code);
    todo!()
}
