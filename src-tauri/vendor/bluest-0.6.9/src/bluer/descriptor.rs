use crate::{Descriptor, Result, Uuid};

/// A Bluetooth GATT descriptor
#[derive(Debug, Clone)]
pub struct DescriptorImpl {
    inner: bluer::gatt::remote::Descriptor,
}

impl PartialEq for DescriptorImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.adapter_name() == other.inner.adapter_name()
            && self.inner.device_address() == other.inner.device_address()
            && self.inner.service_id() == other.inner.service_id()
            && self.inner.characteristic_id() == other.inner.characteristic_id()
            && self.inner.id() == other.inner.id()
    }
}

impl Eq for DescriptorImpl {}

impl Descriptor {
    pub(super) fn new(inner: bluer::gatt::remote::Descriptor) -> Descriptor {
        Descriptor(DescriptorImpl { inner })
    }
}

impl DescriptorImpl {
    /// The [`Uuid`] identifying the type of this GATT descriptor
    ///
    /// # Panics
    ///
    /// This method will panic if there is a current Tokio runtime and it is single-threaded, if there is no current
    /// Tokio runtime and creating one fails, or if the underlying [`DescriptorImpl::uuid_async()`] method fails.
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

    /// The [`Uuid`] identifying the type of this GATT descriptor
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.inner.uuid().await.map_err(Into::into)
    }

    /// The cached value of this descriptor
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.inner.cached_value().await.map_err(Into::into)
    }

    /// Read the value of this descriptor from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.inner.read().await.map_err(Into::into)
    }

    /// Write the value of this descriptor on the device to `value`
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        self.inner.write(value).await.map_err(Into::into)
    }
}
