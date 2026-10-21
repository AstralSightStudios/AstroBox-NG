use windows::Devices::Bluetooth::BluetoothCacheMode;
use windows::Devices::Bluetooth::GenericAttributeProfile::GattDescriptor;
use windows::Storage::Streams::{DataReader, DataWriter};

use super::error::check_communication_status;
use crate::{Descriptor, Result, Uuid};

/// A Bluetooth GATT descriptor
#[derive(Clone, PartialEq, Eq)]
pub struct DescriptorImpl {
    inner: GattDescriptor,
}

impl std::fmt::Debug for DescriptorImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Descriptor")
            .field("uuid", &self.inner.Uuid().unwrap())
            .field("handle", &self.inner.AttributeHandle().unwrap())
            .finish()
    }
}

impl Descriptor {
    pub(super) fn new(descriptor: GattDescriptor) -> Self {
        Descriptor(DescriptorImpl { inner: descriptor })
    }
}

impl DescriptorImpl {
    /// The [`Uuid`] identifying the type of descriptor
    pub fn uuid(&self) -> Uuid {
        Uuid::from_u128(self.inner.Uuid().expect("UUID missing on GattDescriptor").to_u128())
    }

    /// The [`Uuid`] identifying the type of this GATT descriptor
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(Uuid::from_u128(self.inner.Uuid()?.to_u128()))
    }

    /// The cached value of this descriptor
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.read_value(BluetoothCacheMode::Cached).await
    }

    /// Read the value of this descriptor from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.read_value(BluetoothCacheMode::Uncached).await
    }

    async fn read_value(&self, cachemode: BluetoothCacheMode) -> Result<Vec<u8>> {
        let res = self.inner.ReadValueWithCacheModeAsync(cachemode)?.await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "reading descriptor value")?;

        let buf = res.Value()?;
        let mut data = vec![0; buf.Length()? as usize];
        let reader = DataReader::FromBuffer(&buf)?;
        reader.ReadBytes(data.as_mut_slice())?;
        Ok(data)
    }

    /// Write the value of this descriptor on the device to `value`
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        let op = {
            let writer = DataWriter::new()?;
            writer.WriteBytes(value)?;
            let buf = writer.DetachBuffer()?;
            self.inner.WriteValueWithResultAsync(&buf)?
        };
        let res = op.await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "writing descriptor value")
    }
}
