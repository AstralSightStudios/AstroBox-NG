use std::collections::HashMap;

use objc2_core_bluetooth::{
    CBAdvertisementDataIsConnectable, CBAdvertisementDataLocalNameKey, CBAdvertisementDataManufacturerDataKey,
    CBAdvertisementDataOverflowServiceUUIDsKey, CBAdvertisementDataServiceDataKey, CBAdvertisementDataServiceUUIDsKey,
    CBAdvertisementDataTxPowerLevelKey, CBUUID,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSString};
use uuid::Uuid;

use crate::{AdvertisementData, BluetoothUuidExt, ManufacturerData};

impl AdvertisementData {
    pub(crate) fn from_nsdictionary(adv_data: &NSDictionary<NSString>) -> Self {
        let is_connectable = adv_data
            .objectForKey(unsafe { CBAdvertisementDataIsConnectable })
            .is_some_and(|val| val.downcast_ref::<NSNumber>().map(|b| b.as_bool()).unwrap_or(false));

        let local_name = adv_data
            .objectForKey(unsafe { CBAdvertisementDataLocalNameKey })
            .and_then(|val| val.downcast_ref::<NSString>().map(|s| s.to_string()));

        let manufacturer_data = adv_data
            .objectForKey(unsafe { CBAdvertisementDataManufacturerDataKey })
            .and_then(|val| val.downcast_ref::<NSData>().map(|v| v.to_vec()))
            .and_then(|val| {
                (val.len() >= 2).then(|| ManufacturerData {
                    company_id: u16::from_le_bytes(val[0..2].try_into().unwrap()),
                    data: val[2..].to_vec(),
                })
            });

        let tx_power_level: Option<i16> = adv_data
            .objectForKey(unsafe { CBAdvertisementDataTxPowerLevelKey })
            .and_then(|val| val.downcast_ref::<NSNumber>().map(|val| val.shortValue()));

        let service_data = if let Some(val) = adv_data.objectForKey(unsafe { CBAdvertisementDataServiceDataKey }) {
            unsafe {
                if let Some(val) = val.downcast_ref::<NSDictionary>() {
                    let mut res = HashMap::with_capacity(val.count());
                    for k in val.allKeys() {
                        if let Some(key) = k.downcast_ref::<CBUUID>() {
                            if let Some(val) = val
                                .objectForKey_unchecked(&k)
                                .and_then(|val| val.downcast_ref::<NSData>())
                            {
                                res.insert(
                                    Uuid::from_bluetooth_bytes(key.data().as_bytes_unchecked()),
                                    val.to_vec(),
                                );
                            }
                        }
                    }
                    res
                } else {
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        let services = adv_data
            .objectForKey(unsafe { CBAdvertisementDataServiceUUIDsKey })
            .into_iter()
            .chain(adv_data.objectForKey(unsafe { CBAdvertisementDataOverflowServiceUUIDsKey }))
            .flat_map(|x| x.downcast::<NSArray>())
            .flatten()
            .flat_map(|obj| obj.downcast::<CBUUID>())
            .map(|uuid| unsafe { uuid.data() })
            .map(|data| unsafe { Uuid::from_bluetooth_bytes(data.as_bytes_unchecked()) })
            .collect();

        AdvertisementData {
            local_name,
            manufacturer_data,
            services,
            service_data,
            tx_power_level,
            is_connectable,
        }
    }
}
