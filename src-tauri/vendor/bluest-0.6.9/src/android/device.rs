use futures_core::Stream;
use futures_lite::stream;
use java_spaghetti::Global;
use uuid::Uuid;

use super::bindings::android::bluetooth::BluetoothDevice;
#[cfg(feature = "l2cap")]
use super::l2cap_channel::{L2capChannelReader, L2capChannelWriter};
use crate::pairing::PairingAgent;
use crate::{DeviceId, Result, Service, ServicesChanged};

#[derive(Clone)]
pub struct DeviceImpl {
    pub(super) id: DeviceId,
    #[cfg_attr(not(feature = "l2cap"), allow(unused))]
    pub(super) device: Global<BluetoothDevice>,
}

impl PartialEq for DeviceImpl {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Eq for DeviceImpl {}

impl std::hash::Hash for DeviceImpl {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        todo!()
    }
}

impl std::fmt::Debug for DeviceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("Device");
        f.finish()
    }
}

impl std::fmt::Display for DeviceImpl {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //f.write_str(self.name().as_deref().unwrap_or("(Unknown)"))
        todo!()
    }
}

impl DeviceImpl {
    pub fn id(&self) -> DeviceId {
        self.id.clone()
    }

    pub fn name(&self) -> Result<String> {
        todo!()
    }

    pub async fn name_async(&self) -> Result<String> {
        todo!()
    }

    pub async fn is_connected(&self) -> bool {
        todo!()
    }

    pub async fn is_paired(&self) -> Result<bool> {
        todo!()
    }

    pub async fn pair(&self) -> Result<()> {
        todo!()
    }

    pub async fn pair_with_agent<T: PairingAgent + 'static>(&self, _agent: &T) -> Result<()> {
        todo!()
    }

    pub async fn unpair(&self) -> Result<()> {
        todo!()
    }

    pub async fn discover_services(&self) -> Result<Vec<Service>> {
        todo!()
    }

    pub async fn discover_services_with_uuid(&self, _uuid: Uuid) -> Result<Vec<Service>> {
        todo!()
    }

    pub async fn services(&self) -> Result<Vec<Service>> {
        todo!()
    }

    pub async fn service_changed_indications(
        &self,
    ) -> Result<impl Stream<Item = Result<ServicesChanged>> + Send + Unpin + '_> {
        Ok(stream::empty()) // TODO
    }

    pub async fn rssi(&self) -> Result<i16> {
        todo!()
    }

    #[cfg(feature = "l2cap")]
    pub async fn open_l2cap_channel(
        &self,
        psm: u16,
        secure: bool,
    ) -> std::prelude::v1::Result<(L2capChannelReader, L2capChannelWriter), crate::Error> {
        super::l2cap_channel::open_l2cap_channel(self.device.clone(), psm, secure)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServicesChangedImpl;

impl ServicesChangedImpl {
    pub fn was_invalidated(&self, _service: &Service) -> bool {
        true
    }
}
