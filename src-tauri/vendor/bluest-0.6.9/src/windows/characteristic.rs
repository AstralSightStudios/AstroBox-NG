use futures_core::Stream;
use futures_lite::StreamExt;
use tracing::{error, warn};
use windows::Devices::Bluetooth::BluetoothCacheMode;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue, GattValueChangedEventArgs,
    GattWriteOption, GattWriteResult,
};
use windows::Foundation::{AsyncOperationCompletedHandler, TypedEventHandler};
use windows::Storage::Streams::{DataReader, DataWriter};

use super::error::check_communication_status;
use crate::error::ErrorKind;
use crate::util::defer;
use crate::{Characteristic, CharacteristicProperties, Descriptor, Error, Result, Uuid};

/// A Bluetooth GATT characteristic
#[derive(Clone)]
pub struct CharacteristicImpl {
    inner: GattCharacteristic,
}

impl PartialEq for CharacteristicImpl {
    fn eq(&self, other: &Self) -> bool {
        self.inner.Service().unwrap().Session().unwrap() == other.inner.Service().unwrap().Session().unwrap()
            && self.inner.AttributeHandle().unwrap() == other.inner.AttributeHandle().unwrap()
    }
}

impl Eq for CharacteristicImpl {}

impl std::hash::Hash for CharacteristicImpl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner
            .Service()
            .unwrap()
            .Session()
            .unwrap()
            .DeviceId()
            .unwrap()
            .Id()
            .unwrap()
            .to_os_string()
            .hash(state);
        self.inner.AttributeHandle().unwrap().hash(state);
    }
}

impl std::fmt::Debug for CharacteristicImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Characteristic")
            .field("uuid", &self.inner.Uuid().expect("UUID missing on GattCharacteristic"))
            .field(
                "handle",
                &self
                    .inner
                    .AttributeHandle()
                    .expect("AttributeHandle missing on GattCharacteristic"),
            )
            .finish()
    }
}

impl Characteristic {
    pub(super) fn new(characteristic: GattCharacteristic) -> Self {
        Characteristic(CharacteristicImpl { inner: characteristic })
    }
}

impl CharacteristicImpl {
    /// The [`Uuid`] identifying the type of this GATT characteristic
    pub fn uuid(&self) -> Uuid {
        Uuid::from_u128(self.inner.Uuid().expect("UUID missing on GattCharacteristic").to_u128())
    }

    /// The [`Uuid`] identifying the type of this GATT characteristic
    pub async fn uuid_async(&self) -> Result<Uuid> {
        Ok(Uuid::from_u128(self.inner.Uuid()?.to_u128()))
    }

    /// The properties of this this GATT characteristic.
    ///
    /// Characteristic properties indicate which operations (e.g. read, write, notify, etc) may be performed on this
    /// characteristic.
    pub async fn properties(&self) -> Result<CharacteristicProperties> {
        let props = self.inner.CharacteristicProperties()?;
        Ok(CharacteristicProperties::from_bits(props.0))
    }

    /// The cached value of this characteristic
    ///
    /// If the value has not yet been read, this method may either return an error or perform a read of the value.
    pub async fn value(&self) -> Result<Vec<u8>> {
        self.read_value(BluetoothCacheMode::Cached).await
    }

    /// Read the value of this characteristic from the device
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.read_value(BluetoothCacheMode::Uncached).await
    }

    async fn read_value(&self, cachemode: BluetoothCacheMode) -> Result<Vec<u8>> {
        let res = self.inner.ReadValueWithCacheModeAsync(cachemode)?.await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "reading characteristic")?;

        let buf = res.Value()?;
        let mut data = vec![0; buf.Length()? as usize];
        let reader = DataReader::FromBuffer(&buf)?;
        reader.ReadBytes(data.as_mut_slice())?;
        Ok(data)
    }

    /// Write the value of this descriptor on the device to `value`
    pub async fn write(&self, value: &[u8]) -> Result<()> {
        self.write_kind(value, GattWriteOption::WriteWithResponse).await
    }

    /// Write the value of this descriptor on the device to `value` without requesting a response.
    pub async fn write_without_response(&self, value: &[u8]) -> Result<()> {
        self.write_kind(value, GattWriteOption::WriteWithoutResponse).await
    }

    async fn write_kind(&self, value: &[u8], writeoption: GattWriteOption) -> Result<()> {
        let op = {
            let writer = DataWriter::new()?;
            writer.WriteBytes(value)?;
            let buf = writer.DetachBuffer()?;
            self.inner.WriteValueWithResultAndOptionAsync(&buf, writeoption)?
        };
        let res = op.await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "writing characteristic")
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub fn max_write_len(&self) -> Result<usize> {
        let mtu = self.inner.Service()?.Session()?.MaxPduSize()?;
        // GATT characteristic writes have 3 bytes of overhead (opcode + handle id)
        Ok(usize::from(mtu) - 3)
    }

    /// Get the maximum amount of data that can be written in a single packet for this characteristic.
    pub async fn max_write_len_async(&self) -> Result<usize> {
        self.max_write_len()
    }

    /// Enables notification of value changes for this GATT characteristic.
    ///
    /// Returns a stream of values for the characteristic sent from the device.
    pub async fn notify(&self) -> Result<impl Stream<Item = Result<Vec<u8>>> + Send + Unpin + '_> {
        let props = self.properties().await?;
        let value = if props.notify {
            GattClientCharacteristicConfigurationDescriptorValue::Notify
        } else if props.indicate {
            GattClientCharacteristicConfigurationDescriptorValue::Indicate
        } else {
            return Err(Error::new(
                ErrorKind::NotSupported,
                None,
                "characteristic does not support indications or notifications",
            ));
        };

        let (mut sender, receiver) = futures_channel::mpsc::channel(16);
        let token = self.inner.ValueChanged(&TypedEventHandler::new(
            move |_characteristic, event_args: &Option<GattValueChangedEventArgs>| {
                let event_args = event_args
                    .as_ref()
                    .expect("GattValueChangedEventArgs was null in ValueChanged handler");

                fn get_value(event_args: &GattValueChangedEventArgs) -> Result<Vec<u8>> {
                    let buf = event_args.CharacteristicValue()?;
                    let len = buf.Length()?;
                    let mut data: Vec<u8> = vec![0; len as usize];
                    let reader = DataReader::FromBuffer(&buf)?;
                    reader.ReadBytes(data.as_mut_slice())?;
                    Ok(data)
                }

                if let Err(err) = sender.try_send(get_value(event_args)) {
                    error!("Error sending characteristic value changed notification: {:?}", err);
                }

                Ok(())
            },
        ))?;

        let guard = defer(move || {
            if let Err(err) = self.inner.RemoveValueChanged(token) {
                warn!("Error removing value change event handler: {:?}", err);
            }
        });

        let res = self
            .inner
            .WriteClientCharacteristicConfigurationDescriptorWithResultAsync(value)?
            .await?;

        check_communication_status(res.Status()?, res.ProtocolError(), "enabling notifications")?;

        let guard = defer(move || {
            let _guard = guard;
            let res = self
                .inner
                .WriteClientCharacteristicConfigurationDescriptorWithResultAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::None,
                )
                .and_then(|op| {
                    op.SetCompleted(&AsyncOperationCompletedHandler::new(move |op, _status| {
                        fn check_status(res: windows::core::Result<GattWriteResult>) -> Result<()> {
                            let res = res?;
                            check_communication_status(
                                res.Status()?,
                                res.ProtocolError(),
                                "disabling characteristic notifications",
                            )
                        }

                        if let Err(err) = check_status(op.as_ref().unwrap().GetResults()) {
                            warn!("Error disabling characteristic notifications {:?}", err);
                        }

                        Ok(())
                    }))
                });

            if let Err(err) = res {
                warn!("Error disabling characteristic notifications: {:?}", err);
            }
        });

        Ok(receiver.map(move |x| {
            let _guard = &guard;
            x
        }))
    }

    /// Is the device currently sending notifications for this characteristic?
    pub async fn is_notifying(&self) -> Result<bool> {
        let res = self
            .inner
            .ReadClientCharacteristicConfigurationDescriptorAsync()?
            .await?;

        check_communication_status(
            res.Status()?,
            res.ProtocolError(),
            "reading client characteristic configuration descriptor",
        )?;

        const INDICATE: i32 = GattClientCharacteristicConfigurationDescriptorValue::Indicate.0;
        const NOTIFY: i32 = GattClientCharacteristicConfigurationDescriptorValue::Notify.0;
        let cccd = res.ClientCharacteristicConfigurationDescriptor()?;
        Ok((cccd.0 & (INDICATE | NOTIFY)) != 0)
    }

    /// Discover the descriptors associated with this characteristic.
    pub async fn discover_descriptors(&self) -> Result<Vec<Descriptor>> {
        self.get_descriptors(BluetoothCacheMode::Uncached).await
    }

    /// Get previously discovered descriptors.
    ///
    /// If no descriptors have been discovered yet, this method will perform descriptor discovery.
    pub async fn descriptors(&self) -> Result<Vec<Descriptor>> {
        self.get_descriptors(BluetoothCacheMode::Cached).await
    }

    async fn get_descriptors(&self, cachemode: BluetoothCacheMode) -> Result<Vec<Descriptor>> {
        let res = self.inner.GetDescriptorsWithCacheModeAsync(cachemode)?.await?;
        check_communication_status(res.Status()?, res.ProtocolError(), "discovering descriptors")?;
        let descriptors = res.Descriptors()?;
        Ok(descriptors.into_iter().map(Descriptor::new).collect())
    }
}
