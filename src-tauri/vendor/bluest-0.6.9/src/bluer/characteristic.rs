use bluer::gatt::remote::CharacteristicWriteRequest;
use bluer::gatt::WriteOp;
use futures_core::Stream;
use futures_lite::StreamExt;

use crate::{Characteristic, CharacteristicProperties, Descriptor, Result, Uuid};

/// A Bluetooth GATT characteristic
#[derive(Debug, Clone)]
pub struct CharacteristicImpl {
    inner: bluer::gatt::remote::Characteristic,
}

impl PartialEq for CharacteristicImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.adapter_name() == other.inner.adapter_name()
            && self.inner.device_address() == other.inner.device_address()
            && self.inner.service_id() == other.inner.service_id()
            && self.inner.id() == other.inner.id()
    }
}

impl Eq for CharacteristicImpl {}

impl std::hash::Hash for CharacteristicImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.adapter_name().hash(state);
        self.inner.device_address().hash(state);
        self.inner.service_id().hash(state);
        self.inner.id().hash(state);
    }
}

impl Characteristic {
    pub(super) fn new(inner: bluer::gatt::remote::Characteristic) -> Characteristic {
        Characteristic(CharacteristicImpl { inner })
    }
}

impl CharacteristicImpl {
    /// The [`Uuid`] identifying the type of this GATT characteristic
    ///
    /// # Panics
    ///
    /// This method will panic if there is a current Tokio runtime and it is single-threaded, if there is no current
    /// Tokio runtime and creating one fails, or if the underlying [`CharacteristicImpl::uuid_async()`] method fails.
    pub fn uuid(&self) -> Uuid {
        // Call an async function from a synchronous context
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(move || handle.block_on(self.uuid_async())),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(self.uuid_async()),
        }
        .unwrap()
    }

    /// The [`Uuid`] identifying the type of this GATT characteristic
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.inner.uuid().await.map_err(Into::into)
    }

    /// The properties of this this GATT characteristic.
    ///
    /// Characteristic properties indicate which operations (e.g. read, write, notify, etc) may be performed on this
    /// characteristic.
    pub async fn properties(&self) -> Result<CharacteristicProperties> {
        self.inner.flags().await.map(Into::into).map_err(Into::into)
    }

    /// The cached value of this characteristic
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.inner.cached_value().await.map_err(Into::into)
    }

    /// Read the value of this characteristic from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.inner.read().await.map_err(Into::into)
    }

    /// Write the value of this descriptor on the device to `value` and request the device return a response indicating
    /// a successful write.
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        self.inner.write(value).await.map_err(Into::into)
    }

    /// Write the value of this descriptor on the device to `value` without requesting a response.
    pub async fn write_without_response(&self, value: &[u8]) -> Result<()> {
        self.inner
            .write_ext(
                value,
                &CharacteristicWriteRequest {
                    op_type: WriteOp::Command,
                    ..Default::default()
                },
            )
            .await
            .map_err(Into::into)
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub fn max_write_len(&self) -> Result<usize> {
        // Call an async function from a synchronous context
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(move || handle.block_on(self.max_write_len_async())),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(self.max_write_len_async()),
        }
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub async fn max_write_len_async(&self) -> Result<usize> {
        let mtu = self.inner.mtu().await?;
        // GATT characteristic writes have 3 bytes of overhead (opcode + handle id)
        Ok(mtu - 3)
    }

    /// Enables notification of value changes for this GATT characteristic.
    ///
    /// Returns a stream of values for the characteristic sent from the device.
    pub async fn notify(&self) -> Result<impl Stream<Item = Result<Vec<u8>>> + Send + Unpin + '_> {
        Ok(Box::pin(self.inner.notify().await?.map(Ok)))
    }

    /// Is the device currently sending notifications for this characteristic?
    pub async fn is_notifying(&self) -> Result<bool> {
        Ok(self.inner.notifying().await?.unwrap_or(false))
    }

    /// Discover the descriptors associated with this characteristic.
    pub async fn discover_descriptors(&self) -> Result<Vec<Descriptor>> {
        self.descriptors().await
    }

    /// Get previously discovered descriptors.
    ///
    /// If no descriptors have been discovered yet, this method will perform descriptor discovery.
    pub async fn descriptors(&self) -> Result<Vec<Descriptor>> {
        self.inner
            .descriptors()
            .await
            .map_err(Into::into)
            .map(|x| x.into_iter().map(Descriptor::new).collect())
    }
}

impl From<bluer::gatt::CharacteristicFlags> for CharacteristicProperties {
    fn from(flags: bluer::gatt::CharacteristicFlags) -> Self {
        CharacteristicProperties {
            broadcast: flags.broadcast,
            read: flags.read,
            write_without_response: flags.write_without_response,
            write: flags.write,
            notify: flags.notify,
            indicate: flags.indicate,
            authenticated_signed_writes: flags.authenticated_signed_writes,
            extended_properties: flags.extended_properties,
            reliable_write: flags.reliable_write,
            writable_auxiliaries: flags.writable_auxiliaries,
        }
    }
}
