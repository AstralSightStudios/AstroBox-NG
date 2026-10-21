use futures_core::Stream;

use crate::{sys, CharacteristicProperties, Descriptor, Result, Uuid};

/// A Bluetooth GATT characteristic
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Characteristic(pub(crate) sys::characteristic::CharacteristicImpl);

impl Characteristic {
    /// The [`Uuid`] identifying the type of this GATT characteristic
    ///
    /// # Panics
    ///
    /// On Linux, this method will panic if there is a current Tokio runtime and it is single-threaded, if there is no
    /// current Tokio runtime and creating one fails, or if the underlying [`Characteristic::uuid_async()`] method
    /// fails.
    #[inline]
    pub fn uuid(&self) -> Uuid {
        self.0.uuid()
    }

    /// The [`Uuid`] identifying the type of this GATT characteristic
    #[inline]
    pub async fn uuid_async(&self) -> Result<Uuid> {
        self.0.uuid_async().await
    }

    /// The properties of this this GATT characteristic.
    ///
    /// Characteristic properties indicate which operations (e.g. read, write, notify, etc) may be performed on this
    /// characteristic.
    #[inline]
    pub async fn properties(&self) -> Result<CharacteristicProperties> {
        self.0.properties().await
    }

    /// The cached value of this characteristic
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    #[inline]
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.0.value().await
    }

    /// Read the value of this characteristic from the device
    #[inline]
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.0.read().await
    }

    /// Write the value of this descriptor on the device to `value` and request the device return a response indicating
    /// a successful write.
    #[inline]
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        self.0.write(value).await
    }

    /// Write the value of this descriptor on the device to `value` without requesting a response.
    #[inline]
    pub async fn write_without_response(&self, value: &[u8]) -> Result<()> {
        self.0.write_without_response(value).await
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    #[inline]
    pub fn max_write_len(&self) -> Result<usize> {
        self.0.max_write_len()
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    #[inline]
    pub async fn max_write_len_async(&self) -> Result<usize> {
        self.0.max_write_len_async().await
    }

    /// Enables notification of value changes for this GATT characteristic.
    ///
    /// Returns a stream of values for the characteristic sent from the device.
    #[inline]
    pub async fn notify(&self) -> Result<impl Stream<Item = Result<Vec<u8>>> + Send + Unpin + '_> {
        self.0.notify().await
    }

    /// Is the device currently sending notifications for this characteristic?
    #[inline]
    pub async fn is_notifying(&self) -> Result<bool> {
        self.0.is_notifying().await
    }

    /// Discover the descriptors associated with this characteristic.
    #[inline]
    pub async fn discover_descriptors(&self) -> Result<Vec<Descriptor>> {
        self.0.discover_descriptors().await
    }

    /// Get previously discovered descriptors.
    ///
    /// If no descriptors have been discovered yet, this method will perform descriptor discovery.
    #[inline]
    pub async fn descriptors(&self) -> Result<Vec<Descriptor>> {
        self.0.descriptors().await
    }
}
