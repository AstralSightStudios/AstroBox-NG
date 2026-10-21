use crate::{sys, Result, Uuid};

/// A Bluetooth GATT descriptor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor(pub(crate) sys::descriptor::DescriptorImpl);

impl Descriptor {
    /// The [`Uuid`] identifying the type of this GATT descriptor
    ///
    /// # Panics
    ///
    /// On Linux, this method will panic if there is a current Tokio runtime and it is single-threaded, if there is no
    /// current Tokio runtime and creating one fails, or if the underlying [`Descriptor::uuid_async()`] method
    /// fails.
    #[inline]
    pub fn uuid(&self) -> Uuid {
        self.0.uuid()
    }

    /// The [`Uuid`] identifying the type of this GATT descriptor
    #[inline]
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.0.uuid_async().await
    }

    /// The cached value of this descriptor
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    #[inline]
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.0.value().await
    }

    /// Read the value of this descriptor from the device
    #[inline]
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.0.read().await
    }

    /// Write the value of this descriptor on the device to `value`
    #[inline]
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        self.0.write(value).await
    }
}
