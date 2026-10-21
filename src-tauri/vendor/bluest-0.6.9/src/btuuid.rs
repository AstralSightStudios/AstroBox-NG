//! `Uuid` extensions for Bluetooth UUIDs

use crate::Uuid;

/// This is the Bluetooth Base UUID. It is used with 16-bit and 32-bit UUIDs
/// [defined](https://www.bluetooth.com/specifications/assigned-numbers/) by the Bluetooth SIG.
pub const BLUETOOTH_BASE_UUID: u128 = 0x00000000_0000_1000_8000_00805f9b34fb;

/// Const function to create a 16-bit Bluetooth UUID
#[must_use]
pub const fn bluetooth_uuid_from_u16(uuid: u16) -> Uuid {
    Uuid::from_u128(((uuid as u128) << 96) | BLUETOOTH_BASE_UUID)
}

/// Const function to create a 32-bit Bluetooth UUID
#[must_use]
pub const fn bluetooth_uuid_from_u32(uuid: u32) -> Uuid {
    Uuid::from_u128(((uuid as u128) << 96) | BLUETOOTH_BASE_UUID)
}

/// Extension trait for [`Uuid`] with helper methods for dealing with Bluetooth 16-bit and 32-bit UUIDs
pub trait BluetoothUuidExt: private::Sealed {
    /// Creates a 16-bit Bluetooth UUID
    fn from_u16(uuid: u16) -> Self;

    /// Creates a 32-bit Bluetooth UUID
    fn from_u32(uuid: u32) -> Self;

    /// Creates a UUID from `bytes`
    ///
    /// # Panics
    ///
    /// Panics if `bytes.len()` is not one of 2, 4, or 16
    fn from_bluetooth_bytes(bytes: &[u8]) -> Self;

    /// Returns `true` if self is a valid 16-bit Bluetooth UUID
    fn is_u16_uuid(&self) -> bool;

    /// Returns `true` if self is a valid 32-bit Bluetooth UUID
    fn is_u32_uuid(&self) -> bool;

    /// Tries to convert self into a 16-bit Bluetooth UUID
    fn try_to_u16(&self) -> Option<u16>;

    /// Tries to convert self into a 32-bit Bluetooth UUID
    fn try_to_u32(&self) -> Option<u32>;

    /// Returns a slice of octets representing the UUID. If the UUID is a valid 16- or 32-bit Bluetooth UUID, the
    /// returned slice will be 2 or 4 octets long, respectively. Otherwise the slice will be 16-octets in length.
    fn as_bluetooth_bytes(&self) -> &[u8];
}

impl BluetoothUuidExt for Uuid {
    fn from_u16(uuid: u16) -> Self {
        bluetooth_uuid_from_u16(uuid)
    }

    fn from_u32(uuid: u32) -> Self {
        bluetooth_uuid_from_u32(uuid)
    }

    fn from_bluetooth_bytes(bytes: &[u8]) -> Self {
        bytes
            .try_into()
            .map(|x| Self::from_u16(u16::from_be_bytes(x)))
            .or_else(|_| bytes.try_into().map(|x| Self::from_u32(u32::from_be_bytes(x))))
            .or_else(|_| bytes.try_into().map(Self::from_bytes))
            .expect("invalid slice length for bluetooth UUID")
    }

    fn is_u16_uuid(&self) -> bool {
        self.try_to_u16().is_some()
    }

    fn is_u32_uuid(&self) -> bool {
        let u = self.as_u128();
        (u & ((1 << 96) - 1)) == BLUETOOTH_BASE_UUID
    }

    fn try_to_u16(&self) -> Option<u16> {
        self.try_to_u32().and_then(|x| x.try_into().ok())
    }

    fn try_to_u32(&self) -> Option<u32> {
        let u = self.as_u128();
        self.is_u32_uuid().then_some((u >> 96) as u32)
    }

    fn as_bluetooth_bytes(&self) -> &[u8] {
        let bytes = self.as_bytes();
        if self.is_u16_uuid() {
            &bytes[2..4]
        } else if self.is_u32_uuid() {
            &bytes[0..4]
        } else {
            &bytes[..]
        }
    }
}

mod private {
    use crate::Uuid;

    pub trait Sealed {}

    impl Sealed for Uuid {}
}

/// Bluetooth GATT Service 16-bit UUIDs
#[allow(missing_docs)]
pub mod services {
    use super::bluetooth_uuid_from_u16;
    use crate::Uuid;

    pub const GENERIC_ACCESS: Uuid = bluetooth_uuid_from_u16(0x1800);
    pub const GENERIC_ATTRIBUTE: Uuid = bluetooth_uuid_from_u16(0x1801);
    pub const IMMEDIATE_ALERT: Uuid = bluetooth_uuid_from_u16(0x1802);
    pub const LINK_LOSS: Uuid = bluetooth_uuid_from_u16(0x1803);
    pub const TX_POWER: Uuid = bluetooth_uuid_from_u16(0x1804);
    pub const CURRENT_TIME: Uuid = bluetooth_uuid_from_u16(0x1805);
    pub const REFERENCE_TIME_UPDATE: Uuid = bluetooth_uuid_from_u16(0x1806);
    pub const NEXT_DST_CHANGE: Uuid = bluetooth_uuid_from_u16(0x1807);
    pub const GLUCOSE: Uuid = bluetooth_uuid_from_u16(0x1808);
    pub const HEALTH_THERMOMETER: Uuid = bluetooth_uuid_from_u16(0x1809);
    pub const DEVICE_INFORMATION: Uuid = bluetooth_uuid_from_u16(0x180A);
    pub const HEART_RATE: Uuid = bluetooth_uuid_from_u16(0x180D);
    pub const PHONE_ALERT_STATUS: Uuid = bluetooth_uuid_from_u16(0x180E);
    pub const BATTERY: Uuid = bluetooth_uuid_from_u16(0x180F);
    pub const BLOOD_PRESSURE: Uuid = bluetooth_uuid_from_u16(0x1810);
    pub const ALERT_NOTIFICATION: Uuid = bluetooth_uuid_from_u16(0x1811);
    pub const HUMAN_INTERFACE_DEVICE: Uuid = bluetooth_uuid_from_u16(0x1812);
    pub const SCAN_PARAMETERS: Uuid = bluetooth_uuid_from_u16(0x1813);
    pub const RUNNING_SPEED_AND_CADENCE: Uuid = bluetooth_uuid_from_u16(0x1814);
    pub const AUTOMATION_IO: Uuid = bluetooth_uuid_from_u16(0x1815);
    pub const CYCLING_SPEED_AND_CADENCE: Uuid = bluetooth_uuid_from_u16(0x1816);
    pub const CYCLING_POWER: Uuid = bluetooth_uuid_from_u16(0x1818);
    pub const LOCATION_AND_NAVIGATION: Uuid = bluetooth_uuid_from_u16(0x1819);
    pub const ENVIRONMENTAL_SENSING: Uuid = bluetooth_uuid_from_u16(0x181A);
    pub const BODY_COMPOSITION: Uuid = bluetooth_uuid_from_u16(0x181B);
    pub const USER_DATA: Uuid = bluetooth_uuid_from_u16(0x181C);
    pub const WEIGHT_SCALE: Uuid = bluetooth_uuid_from_u16(0x181D);
    pub const BOND_MANAGEMENT: Uuid = bluetooth_uuid_from_u16(0x181E);
    pub const CONTINUOUS_GLUCOSE_MONITORING: Uuid = bluetooth_uuid_from_u16(0x181F);
    pub const INTERNET_PROTOCOL_SUPPORT: Uuid = bluetooth_uuid_from_u16(0x1820);
    pub const INDOOR_POSITIONING: Uuid = bluetooth_uuid_from_u16(0x1821);
    pub const PULSE_OXIMETER: Uuid = bluetooth_uuid_from_u16(0x1822);
    pub const HTTP_PROXY: Uuid = bluetooth_uuid_from_u16(0x1823);
    pub const TRANSPORT_DISCOVERY: Uuid = bluetooth_uuid_from_u16(0x1824);
    pub const OBJECT_TRANSFER: Uuid = bluetooth_uuid_from_u16(0x1825);
    pub const FITNESS_MACHINE: Uuid = bluetooth_uuid_from_u16(0x1826);
    pub const MESH_PROVISIONING: Uuid = bluetooth_uuid_from_u16(0x1827);
    pub const MESH_PROXY: Uuid = bluetooth_uuid_from_u16(0x1828);
    pub const RECONNECTION_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x1829);
    pub const INSULIN_DELIVERY: Uuid = bluetooth_uuid_from_u16(0x183A);
    pub const BINARY_SENSOR: Uuid = bluetooth_uuid_from_u16(0x183B);
    pub const EMERGENCY_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x183C);
    pub const PHYSICAL_ACTIVITY_MONITOR: Uuid = bluetooth_uuid_from_u16(0x183E);
    pub const AUDIO_INPUT_CONTROL: Uuid = bluetooth_uuid_from_u16(0x1843);
    pub const VOLUME_CONTROL: Uuid = bluetooth_uuid_from_u16(0x1844);
    pub const VOLUME_OFFSET_CONTROL: Uuid = bluetooth_uuid_from_u16(0x1845);
    pub const COORDINATED_SET_IDENTIFICATION: Uuid = bluetooth_uuid_from_u16(0x1846);
    pub const DEVICE_TIME: Uuid = bluetooth_uuid_from_u16(0x1847);
    pub const MEDIA_CONTROL: Uuid = bluetooth_uuid_from_u16(0x1848);
    pub const GENERIC_MEDIA_CONTROL: Uuid = bluetooth_uuid_from_u16(0x1849);
    pub const CONSTANT_TONE_EXTENSION: Uuid = bluetooth_uuid_from_u16(0x184A);
    pub const TELEPHONE_BEARER: Uuid = bluetooth_uuid_from_u16(0x184B);
    pub const GENERIC_TELEPHONE_BEARER: Uuid = bluetooth_uuid_from_u16(0x184C);
    pub const MICROPHONE_CONTROL: Uuid = bluetooth_uuid_from_u16(0x184D);
    pub const AUDIO_STREAM_CONTROL: Uuid = bluetooth_uuid_from_u16(0x184E);
    pub const BROADCAST_AUDIO_SCAN: Uuid = bluetooth_uuid_from_u16(0x184F);
    pub const PUBLISHED_AUDIO_CAPABILITIES: Uuid = bluetooth_uuid_from_u16(0x1850);
    pub const BASIC_AUDIO_ANNOUNCEMENT: Uuid = bluetooth_uuid_from_u16(0x1851);
    pub const BROADCAST_AUDIO_ANNOUNCEMENT: Uuid = bluetooth_uuid_from_u16(0x1852);
    pub const COMMON_AUDIO: Uuid = bluetooth_uuid_from_u16(0x1853);
    pub const HEARING_ACCESS: Uuid = bluetooth_uuid_from_u16(0x1854);
    pub const TMAS: Uuid = bluetooth_uuid_from_u16(0x1855);
    pub const PUBLIC_BROADCAST_ANNOUNCEMENT: Uuid = bluetooth_uuid_from_u16(0x1856);
}

/// Bluetooth GATT Characteristic 16-bit UUIDs
#[allow(missing_docs)]
pub mod characteristics {
    use super::bluetooth_uuid_from_u16;
    use crate::Uuid;

    pub const DEVICE_NAME: Uuid = bluetooth_uuid_from_u16(0x2A00);
    pub const APPEARANCE: Uuid = bluetooth_uuid_from_u16(0x2A01);
    pub const PERIPHERAL_PRIVACY_FLAG: Uuid = bluetooth_uuid_from_u16(0x2A02);
    pub const RECONNECTION_ADDRESS: Uuid = bluetooth_uuid_from_u16(0x2A03);
    pub const PERIPHERAL_PREFERRED_CONNECTION_PARAMETERS: Uuid = bluetooth_uuid_from_u16(0x2A04);
    pub const SERVICE_CHANGED: Uuid = bluetooth_uuid_from_u16(0x2A05);
    pub const ALERT_LEVEL: Uuid = bluetooth_uuid_from_u16(0x2A06);
    pub const TX_POWER_LEVEL: Uuid = bluetooth_uuid_from_u16(0x2A07);
    pub const DATE_TIME: Uuid = bluetooth_uuid_from_u16(0x2A08);
    pub const DAY_OF_WEEK: Uuid = bluetooth_uuid_from_u16(0x2A09);
    pub const DAY_DATE_TIME: Uuid = bluetooth_uuid_from_u16(0x2A0A);
    pub const EXACT_TIME_256: Uuid = bluetooth_uuid_from_u16(0x2A0C);
    pub const DST_OFFSET: Uuid = bluetooth_uuid_from_u16(0x2A0D);
    pub const TIME_ZONE: Uuid = bluetooth_uuid_from_u16(0x2A0E);
    pub const LOCAL_TIME_INFORMATION: Uuid = bluetooth_uuid_from_u16(0x2A0F);
    pub const TIME_WITH_DST: Uuid = bluetooth_uuid_from_u16(0x2A11);
    pub const TIME_ACCURACY: Uuid = bluetooth_uuid_from_u16(0x2A12);
    pub const TIME_SOURCE: Uuid = bluetooth_uuid_from_u16(0x2A13);
    pub const REFERENCE_TIME_INFORMATION: Uuid = bluetooth_uuid_from_u16(0x2A14);
    pub const TIME_UPDATE_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A16);
    pub const TIME_UPDATE_STATE: Uuid = bluetooth_uuid_from_u16(0x2A17);
    pub const GLUCOSE_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A18);
    pub const BATTERY_LEVEL: Uuid = bluetooth_uuid_from_u16(0x2A19);
    pub const TEMPERATURE_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A1C);
    pub const TEMPERATURE_TYPE: Uuid = bluetooth_uuid_from_u16(0x2A1D);
    pub const INTERMEDIATE_TEMPERATURE: Uuid = bluetooth_uuid_from_u16(0x2A1E);
    pub const MEASUREMENT_INTERVAL: Uuid = bluetooth_uuid_from_u16(0x2A21);
    pub const BOOT_KEYBOARD_INPUT_REPORT: Uuid = bluetooth_uuid_from_u16(0x2A22);
    pub const SYSTEM_ID: Uuid = bluetooth_uuid_from_u16(0x2A23);
    pub const MODEL_NUMBER_STRING: Uuid = bluetooth_uuid_from_u16(0x2A24);
    pub const SERIAL_NUMBER_STRING: Uuid = bluetooth_uuid_from_u16(0x2A25);
    pub const FIRMWARE_REVISION_STRING: Uuid = bluetooth_uuid_from_u16(0x2A26);
    pub const HARDWARE_REVISION_STRING: Uuid = bluetooth_uuid_from_u16(0x2A27);
    pub const SOFTWARE_REVISION_STRING: Uuid = bluetooth_uuid_from_u16(0x2A28);
    pub const MANUFACTURER_NAME_STRING: Uuid = bluetooth_uuid_from_u16(0x2A29);
    pub const IEEE_11073_20601_REGULATORY_CERTIFICATION_DATA_LIST: Uuid = bluetooth_uuid_from_u16(0x2A2A);
    pub const CURRENT_TIME: Uuid = bluetooth_uuid_from_u16(0x2A2B);
    pub const SCAN_REFRESH: Uuid = bluetooth_uuid_from_u16(0x2A31);
    pub const BOOT_KEYBOARD_OUTPUT_REPORT: Uuid = bluetooth_uuid_from_u16(0x2A32);
    pub const BOOT_MOUSE_INPUT_REPORT: Uuid = bluetooth_uuid_from_u16(0x2A33);
    pub const GLUCOSE_MEASUREMENT_CONTEXT: Uuid = bluetooth_uuid_from_u16(0x2A34);
    pub const BLOOD_PRESSURE_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A35);
    pub const INTERMEDIATE_CUFF_PRESSURE: Uuid = bluetooth_uuid_from_u16(0x2A36);
    pub const HEART_RATE_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A37);
    pub const BODY_SENSOR_LOCATION: Uuid = bluetooth_uuid_from_u16(0x2A38);
    pub const HEART_RATE_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A39);
    pub const ALERT_STATUS: Uuid = bluetooth_uuid_from_u16(0x2A3F);
    pub const RINGER_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A40);
    pub const RINGER_SETTING: Uuid = bluetooth_uuid_from_u16(0x2A41);
    pub const ALERT_CATEGORY_ID_BIT_MASK: Uuid = bluetooth_uuid_from_u16(0x2A42);
    pub const ALERT_CATEGORY_ID: Uuid = bluetooth_uuid_from_u16(0x2A43);
    pub const ALERT_NOTIFICATION_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A44);
    pub const UNREAD_ALERT_STATUS: Uuid = bluetooth_uuid_from_u16(0x2A45);
    pub const NEW_ALERT: Uuid = bluetooth_uuid_from_u16(0x2A46);
    pub const SUPPORTED_NEW_ALERT_CATEGORY: Uuid = bluetooth_uuid_from_u16(0x2A47);
    pub const SUPPORTED_UNREAD_ALERT_CATEGORY: Uuid = bluetooth_uuid_from_u16(0x2A48);
    pub const BLOOD_PRESSURE_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A49);
    pub const HID_INFORMATION: Uuid = bluetooth_uuid_from_u16(0x2A4A);
    pub const REPORT_MAP: Uuid = bluetooth_uuid_from_u16(0x2A4B);
    pub const HID_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A4C);
    pub const REPORT: Uuid = bluetooth_uuid_from_u16(0x2A4D);
    pub const PROTOCOL_MODE: Uuid = bluetooth_uuid_from_u16(0x2A4E);
    pub const SCAN_INTERVAL_WINDOW: Uuid = bluetooth_uuid_from_u16(0x2A4F);
    pub const PNP_ID: Uuid = bluetooth_uuid_from_u16(0x2A50);
    pub const GLUCOSE_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A51);
    pub const RECORD_ACCESS_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A52);
    pub const RSC_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A53);
    pub const RSC_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A54);
    pub const SC_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A55);
    pub const AGGREGATE: Uuid = bluetooth_uuid_from_u16(0x2A5A);
    pub const CSC_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A5B);
    pub const CSC_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A5C);
    pub const SENSOR_LOCATION: Uuid = bluetooth_uuid_from_u16(0x2A5D);
    pub const PLX_SPOT_CHECK_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A5E);
    pub const PLX_CONTINUOUS_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A5F);
    pub const PLX_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2A60);
    pub const CYCLING_POWER_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A63);
    pub const CYCLING_POWER_VECTOR: Uuid = bluetooth_uuid_from_u16(0x2A64);
    pub const CYCLING_POWER_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A65);
    pub const CYCLING_POWER_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A66);
    pub const LOCATION_AND_SPEED: Uuid = bluetooth_uuid_from_u16(0x2A67);
    pub const NAVIGATION: Uuid = bluetooth_uuid_from_u16(0x2A68);
    pub const POSITION_QUALITY: Uuid = bluetooth_uuid_from_u16(0x2A69);
    pub const LN_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A6A);
    pub const LN_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A6B);
    pub const ELEVATION: Uuid = bluetooth_uuid_from_u16(0x2A6C);
    pub const PRESSURE: Uuid = bluetooth_uuid_from_u16(0x2A6D);
    pub const TEMPERATURE: Uuid = bluetooth_uuid_from_u16(0x2A6E);
    pub const HUMIDITY: Uuid = bluetooth_uuid_from_u16(0x2A6F);
    pub const TRUE_WIND_SPEED: Uuid = bluetooth_uuid_from_u16(0x2A70);
    pub const TRUE_WIND_DIRECTION: Uuid = bluetooth_uuid_from_u16(0x2A71);
    pub const APPARENT_WIND_SPEED: Uuid = bluetooth_uuid_from_u16(0x2A72);
    pub const APPARENT_WIND_DIRECTION: Uuid = bluetooth_uuid_from_u16(0x2A73);
    pub const GUST_FACTOR: Uuid = bluetooth_uuid_from_u16(0x2A74);
    pub const POLLEN_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2A75);
    pub const UV_INDEX: Uuid = bluetooth_uuid_from_u16(0x2A76);
    pub const IRRADIANCE: Uuid = bluetooth_uuid_from_u16(0x2A77);
    pub const RAINFALL: Uuid = bluetooth_uuid_from_u16(0x2A78);
    pub const WIND_CHILL: Uuid = bluetooth_uuid_from_u16(0x2A79);
    pub const HEAT_INDEX: Uuid = bluetooth_uuid_from_u16(0x2A7A);
    pub const DEW_POINT: Uuid = bluetooth_uuid_from_u16(0x2A7B);
    pub const DESCRIPTOR_VALUE_CHANGED: Uuid = bluetooth_uuid_from_u16(0x2A7D);
    pub const AEROBIC_HEART_RATE_LOWER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A7E);
    pub const AEROBIC_THRESHOLD: Uuid = bluetooth_uuid_from_u16(0x2A7F);
    pub const AGE: Uuid = bluetooth_uuid_from_u16(0x2A80);
    pub const ANAEROBIC_HEART_RATE_LOWER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A81);
    pub const ANAEROBIC_HEART_RATE_UPPER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A82);
    pub const ANAEROBIC_THRESHOLD: Uuid = bluetooth_uuid_from_u16(0x2A83);
    pub const AEROBIC_HEART_RATE_UPPER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A84);
    pub const DATE_OF_BIRTH: Uuid = bluetooth_uuid_from_u16(0x2A85);
    pub const DATE_OF_THRESHOLD_ASSESSMENT: Uuid = bluetooth_uuid_from_u16(0x2A86);
    pub const EMAIL_ADDRESS: Uuid = bluetooth_uuid_from_u16(0x2A87);
    pub const FAT_BURN_HEART_RATE_LOWER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A88);
    pub const FAT_BURN_HEART_RATE_UPPER_LIMIT: Uuid = bluetooth_uuid_from_u16(0x2A89);
    pub const FIRST_NAME: Uuid = bluetooth_uuid_from_u16(0x2A8A);
    pub const FIVE_ZONE_HEART_RATE_LIMITS: Uuid = bluetooth_uuid_from_u16(0x2A8B);
    pub const GENDER: Uuid = bluetooth_uuid_from_u16(0x2A8C);
    pub const HEART_RATE_MAX: Uuid = bluetooth_uuid_from_u16(0x2A8D);
    pub const HEIGHT: Uuid = bluetooth_uuid_from_u16(0x2A8E);
    pub const HIP_CIRCUMFERENCE: Uuid = bluetooth_uuid_from_u16(0x2A8F);
    pub const LAST_NAME: Uuid = bluetooth_uuid_from_u16(0x2A90);
    pub const MAXIMUM_RECOMMENDED_HEART_RATE: Uuid = bluetooth_uuid_from_u16(0x2A91);
    pub const RESTING_HEART_RATE: Uuid = bluetooth_uuid_from_u16(0x2A92);
    pub const SPORT_TYPE_FOR_AEROBIC_AND_ANAEROBIC_THRESHOLDS: Uuid = bluetooth_uuid_from_u16(0x2A93);
    pub const THREE_ZONE_HEART_RATE_LIMITS: Uuid = bluetooth_uuid_from_u16(0x2A94);
    pub const TWO_ZONE_HEART_RATE_LIMITS: Uuid = bluetooth_uuid_from_u16(0x2A95);
    pub const VO2_MAX: Uuid = bluetooth_uuid_from_u16(0x2A96);
    pub const WAIST_CIRCUMFERENCE: Uuid = bluetooth_uuid_from_u16(0x2A97);
    pub const WEIGHT: Uuid = bluetooth_uuid_from_u16(0x2A98);
    pub const DATABASE_CHANGE_INCREMENT: Uuid = bluetooth_uuid_from_u16(0x2A99);
    pub const USER_INDEX: Uuid = bluetooth_uuid_from_u16(0x2A9A);
    pub const BODY_COMPOSITION_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A9B);
    pub const BODY_COMPOSITION_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A9C);
    pub const WEIGHT_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2A9D);
    pub const WEIGHT_SCALE_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2A9E);
    pub const USER_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2A9F);
    pub const MAGNETIC_FLUX_DENSITY_2D: Uuid = bluetooth_uuid_from_u16(0x2AA0);
    pub const MAGNETIC_FLUX_DENSITY_3D: Uuid = bluetooth_uuid_from_u16(0x2AA1);
    pub const LANGUAGE: Uuid = bluetooth_uuid_from_u16(0x2AA2);
    pub const BAROMETRIC_PRESSURE_TREND: Uuid = bluetooth_uuid_from_u16(0x2AA3);
    pub const BOND_MANAGEMENT_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2AA4);
    pub const BOND_MANAGEMENT_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2AA5);
    pub const CENTRAL_ADDRESS_RESOLUTION: Uuid = bluetooth_uuid_from_u16(0x2AA6);
    pub const CGM_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2AA7);
    pub const CGM_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2AA8);
    pub const CGM_STATUS: Uuid = bluetooth_uuid_from_u16(0x2AA9);
    pub const CGM_SESSION_START_TIME: Uuid = bluetooth_uuid_from_u16(0x2AAA);
    pub const CGM_SESSION_RUN_TIME: Uuid = bluetooth_uuid_from_u16(0x2AAB);
    pub const CGM_SPECIFIC_OPS_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2AAC);
    pub const INDOOR_POSITIONING_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x2AAD);
    pub const LATITUDE: Uuid = bluetooth_uuid_from_u16(0x2AAE);
    pub const LONGITUDE: Uuid = bluetooth_uuid_from_u16(0x2AAF);
    pub const LOCAL_NORTH_COORDINATE: Uuid = bluetooth_uuid_from_u16(0x2AB0);
    pub const LOCAL_EAST_COORDINATE: Uuid = bluetooth_uuid_from_u16(0x2AB1);
    pub const FLOOR_NUMBER: Uuid = bluetooth_uuid_from_u16(0x2AB2);
    pub const ALTITUDE: Uuid = bluetooth_uuid_from_u16(0x2AB3);
    pub const UNCERTAINTY: Uuid = bluetooth_uuid_from_u16(0x2AB4);
    pub const LOCATION_NAME: Uuid = bluetooth_uuid_from_u16(0x2AB5);
    pub const URI: Uuid = bluetooth_uuid_from_u16(0x2AB6);
    pub const HTTP_HEADERS: Uuid = bluetooth_uuid_from_u16(0x2AB7);
    pub const HTTP_STATUS_CODE: Uuid = bluetooth_uuid_from_u16(0x2AB8);
    pub const HTTP_ENTITY_BODY: Uuid = bluetooth_uuid_from_u16(0x2AB9);
    pub const HTTP_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2ABA);
    pub const HTTPS_SECURITY: Uuid = bluetooth_uuid_from_u16(0x2ABB);
    pub const TDS_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2ABC);
    pub const OTS_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2ABD);
    pub const OBJECT_NAME: Uuid = bluetooth_uuid_from_u16(0x2ABE);
    pub const OBJECT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2ABF);
    pub const OBJECT_SIZE: Uuid = bluetooth_uuid_from_u16(0x2AC0);
    pub const OBJECT_FIRST_CREATED: Uuid = bluetooth_uuid_from_u16(0x2AC1);
    pub const OBJECT_LAST_MODIFIED: Uuid = bluetooth_uuid_from_u16(0x2AC2);
    pub const OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2AC3);
    pub const OBJECT_PROPERTIES: Uuid = bluetooth_uuid_from_u16(0x2AC4);
    pub const OBJECT_ACTION_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2AC5);
    pub const OBJECT_LIST_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2AC6);
    pub const OBJECT_LIST_FILTER: Uuid = bluetooth_uuid_from_u16(0x2AC7);
    pub const OBJECT_CHANGED: Uuid = bluetooth_uuid_from_u16(0x2AC8);
    pub const RESOLVABLE_PRIVATE_ADDRESS_ONLY: Uuid = bluetooth_uuid_from_u16(0x2AC9);
    pub const UNSPECIFIED: Uuid = bluetooth_uuid_from_u16(0x2ACA);
    pub const DIRECTORY_LISTING: Uuid = bluetooth_uuid_from_u16(0x2ACB);
    pub const FITNESS_MACHINE_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2ACC);
    pub const TREADMILL_DATA: Uuid = bluetooth_uuid_from_u16(0x2ACD);
    pub const CROSS_TRAINER_DATA: Uuid = bluetooth_uuid_from_u16(0x2ACE);
    pub const STEP_CLIMBER_DATA: Uuid = bluetooth_uuid_from_u16(0x2ACF);
    pub const STAIR_CLIMBER_DATA: Uuid = bluetooth_uuid_from_u16(0x2AD0);
    pub const ROWER_DATA: Uuid = bluetooth_uuid_from_u16(0x2AD1);
    pub const INDOOR_BIKE_DATA: Uuid = bluetooth_uuid_from_u16(0x2AD2);
    pub const TRAINING_STATUS: Uuid = bluetooth_uuid_from_u16(0x2AD3);
    pub const SUPPORTED_SPEED_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AD4);
    pub const SUPPORTED_INCLINATION_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AD5);
    pub const SUPPORTED_RESISTANCE_LEVEL_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AD6);
    pub const SUPPORTED_HEART_RATE_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AD7);
    pub const SUPPORTED_POWER_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AD8);
    pub const FITNESS_MACHINE_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2AD9);
    pub const FITNESS_MACHINE_STATUS: Uuid = bluetooth_uuid_from_u16(0x2ADA);
    pub const MESH_PROVISIONING_DATA_IN: Uuid = bluetooth_uuid_from_u16(0x2ADB);
    pub const MESH_PROVISIONING_DATA_OUT: Uuid = bluetooth_uuid_from_u16(0x2ADC);
    pub const MESH_PROXY_DATA_IN: Uuid = bluetooth_uuid_from_u16(0x2ADD);
    pub const MESH_PROXY_DATA_OUT: Uuid = bluetooth_uuid_from_u16(0x2ADE);
    pub const AVERAGE_CURRENT: Uuid = bluetooth_uuid_from_u16(0x2AE0);
    pub const AVERAGE_VOLTAGE: Uuid = bluetooth_uuid_from_u16(0x2AE1);
    pub const BOOLEAN: Uuid = bluetooth_uuid_from_u16(0x2AE2);
    pub const CHROMATIC_DISTANCE_FROM_PLANCKIAN: Uuid = bluetooth_uuid_from_u16(0x2AE3);
    pub const CHROMATICITY_COORDINATES: Uuid = bluetooth_uuid_from_u16(0x2AE4);
    pub const CHROMATICITY_IN_CCT_AND_DUV_VALUES: Uuid = bluetooth_uuid_from_u16(0x2AE5);
    pub const CHROMATICITY_TOLERANCE: Uuid = bluetooth_uuid_from_u16(0x2AE6);
    pub const CIE_13_3_1995_COLOR_RENDERING_INDEX: Uuid = bluetooth_uuid_from_u16(0x2AE7);
    pub const COEFFICIENT: Uuid = bluetooth_uuid_from_u16(0x2AE8);
    pub const CORRELATED_COLOR_TEMPERATURE: Uuid = bluetooth_uuid_from_u16(0x2AE9);
    pub const COUNT_16: Uuid = bluetooth_uuid_from_u16(0x2AEA);
    pub const COUNT_24: Uuid = bluetooth_uuid_from_u16(0x2AEB);
    pub const COUNTRY_CODE: Uuid = bluetooth_uuid_from_u16(0x2AEC);
    pub const DATE_UTC: Uuid = bluetooth_uuid_from_u16(0x2AED);
    pub const ELECTRIC_CURRENT: Uuid = bluetooth_uuid_from_u16(0x2AEE);
    pub const ELECTRIC_CURRENT_RANGE: Uuid = bluetooth_uuid_from_u16(0x2AEF);
    pub const ELECTRIC_CURRENT_SPECIFICATION: Uuid = bluetooth_uuid_from_u16(0x2AF0);
    pub const ELECTRIC_CURRENT_STATISTICS: Uuid = bluetooth_uuid_from_u16(0x2AF1);
    pub const ENERGY: Uuid = bluetooth_uuid_from_u16(0x2AF2);
    pub const ENERGY_IN_A_PERIOD_OF_DAY: Uuid = bluetooth_uuid_from_u16(0x2AF3);
    pub const EVENT_STATISTICS: Uuid = bluetooth_uuid_from_u16(0x2AF4);
    pub const FIXED_STRING_16: Uuid = bluetooth_uuid_from_u16(0x2AF5);
    pub const FIXED_STRING_24: Uuid = bluetooth_uuid_from_u16(0x2AF6);
    pub const FIXED_STRING_36: Uuid = bluetooth_uuid_from_u16(0x2AF7);
    pub const FIXED_STRING_8: Uuid = bluetooth_uuid_from_u16(0x2AF8);
    pub const GENERIC_LEVEL: Uuid = bluetooth_uuid_from_u16(0x2AF9);
    pub const GLOBAL_TRADE_ITEM_NUMBER: Uuid = bluetooth_uuid_from_u16(0x2AFA);
    pub const ILLUMINANCE: Uuid = bluetooth_uuid_from_u16(0x2AFB);
    pub const LUMINOUS_EFFICACY: Uuid = bluetooth_uuid_from_u16(0x2AFC);
    pub const LUMINOUS_ENERGY: Uuid = bluetooth_uuid_from_u16(0x2AFD);
    pub const LUMINOUS_EXPOSURE: Uuid = bluetooth_uuid_from_u16(0x2AFE);
    pub const LUMINOUS_FLUX: Uuid = bluetooth_uuid_from_u16(0x2AFF);
    pub const LUMINOUS_FLUX_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B00);
    pub const LUMINOUS_INTENSITY: Uuid = bluetooth_uuid_from_u16(0x2B01);
    pub const MASS_FLOW: Uuid = bluetooth_uuid_from_u16(0x2B02);
    pub const PERCEIVED_LIGHTNESS: Uuid = bluetooth_uuid_from_u16(0x2B03);
    pub const PERCENTAGE_8: Uuid = bluetooth_uuid_from_u16(0x2B04);
    pub const POWER: Uuid = bluetooth_uuid_from_u16(0x2B05);
    pub const POWER_SPECIFICATION: Uuid = bluetooth_uuid_from_u16(0x2B06);
    pub const RELATIVE_RUNTIME_IN_A_CURRENT_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B07);
    pub const RELATIVE_RUNTIME_IN_A_GENERIC_LEVEL_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B08);
    pub const RELATIVE_VALUE_IN_A_VOLTAGE_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B09);
    pub const RELATIVE_VALUE_IN_AN_ILLUMINANCE_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B0A);
    pub const RELATIVE_VALUE_IN_A_PERIOD_OF_DAY: Uuid = bluetooth_uuid_from_u16(0x2B0B);
    pub const RELATIVE_VALUE_IN_A_TEMPERATURE_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B0C);
    pub const TEMPERATURE_8: Uuid = bluetooth_uuid_from_u16(0x2B0D);
    pub const TEMPERATURE_8_IN_A_PERIOD_OF_DAY: Uuid = bluetooth_uuid_from_u16(0x2B0E);
    pub const TEMPERATURE_8_STATISTICS: Uuid = bluetooth_uuid_from_u16(0x2B0F);
    pub const TEMPERATURE_RANGE: Uuid = bluetooth_uuid_from_u16(0x2B10);
    pub const TEMPERATURE_STATISTICS: Uuid = bluetooth_uuid_from_u16(0x2B11);
    pub const TIME_DECIHOUR_8: Uuid = bluetooth_uuid_from_u16(0x2B12);
    pub const TIME_EXPONENTIAL_8: Uuid = bluetooth_uuid_from_u16(0x2B13);
    pub const TIME_HOUR_24: Uuid = bluetooth_uuid_from_u16(0x2B14);
    pub const TIME_MILLISECOND_24: Uuid = bluetooth_uuid_from_u16(0x2B15);
    pub const TIME_SECOND_16: Uuid = bluetooth_uuid_from_u16(0x2B16);
    pub const TIME_SECOND_8: Uuid = bluetooth_uuid_from_u16(0x2B17);
    pub const VOLTAGE: Uuid = bluetooth_uuid_from_u16(0x2B18);
    pub const VOLTAGE_SPECIFICATION: Uuid = bluetooth_uuid_from_u16(0x2B19);
    pub const VOLTAGE_STATISTICS: Uuid = bluetooth_uuid_from_u16(0x2B1A);
    pub const VOLUME_FLOW: Uuid = bluetooth_uuid_from_u16(0x2B1B);
    pub const CHROMATICITY_COORDINATE: Uuid = bluetooth_uuid_from_u16(0x2B1C);
    pub const RC_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2B1D);
    pub const RC_SETTINGS: Uuid = bluetooth_uuid_from_u16(0x2B1E);
    pub const RECONNECTION_CONFIGURATION_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B1F);
    pub const IDD_STATUS_CHANGED: Uuid = bluetooth_uuid_from_u16(0x2B20);
    pub const IDD_STATUS: Uuid = bluetooth_uuid_from_u16(0x2B21);
    pub const IDD_ANNUNCIATION_STATUS: Uuid = bluetooth_uuid_from_u16(0x2B22);
    pub const IDD_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2B23);
    pub const IDD_STATUS_READER_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B24);
    pub const IDD_COMMAND_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B25);
    pub const IDD_COMMAND_DATA: Uuid = bluetooth_uuid_from_u16(0x2B26);
    pub const IDD_RECORD_ACCESS_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B27);
    pub const IDD_HISTORY_DATA: Uuid = bluetooth_uuid_from_u16(0x2B28);
    pub const CLIENT_SUPPORTED_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2B29);
    pub const DATABASE_HASH: Uuid = bluetooth_uuid_from_u16(0x2B2A);
    pub const BSS_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B2B);
    pub const BSS_RESPONSE: Uuid = bluetooth_uuid_from_u16(0x2B2C);
    pub const EMERGENCY_ID: Uuid = bluetooth_uuid_from_u16(0x2B2D);
    pub const EMERGENCY_TEXT: Uuid = bluetooth_uuid_from_u16(0x2B2E);
    pub const ENHANCED_BLOOD_PRESSURE_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x2B34);
    pub const ENHANCED_INTERMEDIATE_CUFF_PRESSURE: Uuid = bluetooth_uuid_from_u16(0x2B35);
    pub const BLOOD_PRESSURE_RECORD: Uuid = bluetooth_uuid_from_u16(0x2B36);
    pub const BR_EDR_HANDOVER_DATA: Uuid = bluetooth_uuid_from_u16(0x2B38);
    pub const BLUETOOTH_SIG_DATA: Uuid = bluetooth_uuid_from_u16(0x2B39);
    pub const SERVER_SUPPORTED_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2B3A);
    pub const PHYSICAL_ACTIVITY_MONITOR_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2B3B);
    pub const GENERAL_ACTIVITY_INSTANTANEOUS_DATA: Uuid = bluetooth_uuid_from_u16(0x2B3C);
    pub const GENERAL_ACTIVITY_SUMMARY_DATA: Uuid = bluetooth_uuid_from_u16(0x2B3D);
    pub const CARDIORESPIRATORY_ACTIVITY_INSTANTANEOUS_DATA: Uuid = bluetooth_uuid_from_u16(0x2B3E);
    pub const CARDIORESPIRATORY_ACTIVITY_SUMMARY_DATA: Uuid = bluetooth_uuid_from_u16(0x2B3F);
    pub const STEP_COUNTER_ACTIVITY_SUMMARY_DATA: Uuid = bluetooth_uuid_from_u16(0x2B40);
    pub const SLEEP_ACTIVITY_INSTANTANEOUS_DATA: Uuid = bluetooth_uuid_from_u16(0x2B41);
    pub const SLEEP_ACTIVITY_SUMMARY_DATA: Uuid = bluetooth_uuid_from_u16(0x2B42);
    pub const PHYSICAL_ACTIVITY_MONITOR_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B43);
    pub const ACTIVITY_CURRENT_SESSION: Uuid = bluetooth_uuid_from_u16(0x2B44);
    pub const PHYSICAL_ACTIVITY_SESSION_DESCRIPTOR: Uuid = bluetooth_uuid_from_u16(0x2B45);
    pub const PREFERRED_UNITS: Uuid = bluetooth_uuid_from_u16(0x2B46);
    pub const HIGH_RESOLUTION_HEIGHT: Uuid = bluetooth_uuid_from_u16(0x2B47);
    pub const MIDDLE_NAME: Uuid = bluetooth_uuid_from_u16(0x2B48);
    pub const STRIDE_LENGTH: Uuid = bluetooth_uuid_from_u16(0x2B49);
    pub const HANDEDNESS: Uuid = bluetooth_uuid_from_u16(0x2B4A);
    pub const DEVICE_WEARING_POSITION: Uuid = bluetooth_uuid_from_u16(0x2B4B);
    pub const FOUR_ZONE_HEART_RATE_LIMITS: Uuid = bluetooth_uuid_from_u16(0x2B4C);
    pub const HIGH_INTENSITY_EXERCISE_THRESHOLD: Uuid = bluetooth_uuid_from_u16(0x2B4D);
    pub const ACTIVITY_GOAL: Uuid = bluetooth_uuid_from_u16(0x2B4E);
    pub const SEDENTARY_INTERVAL_NOTIFICATION: Uuid = bluetooth_uuid_from_u16(0x2B4F);
    pub const CALORIC_INTAKE: Uuid = bluetooth_uuid_from_u16(0x2B50);
    pub const TMAP_ROLE: Uuid = bluetooth_uuid_from_u16(0x2B51);
    pub const AUDIO_INPUT_STATE: Uuid = bluetooth_uuid_from_u16(0x2B77);
    pub const GAIN_SETTINGS_ATTRIBUTE: Uuid = bluetooth_uuid_from_u16(0x2B78);
    pub const AUDIO_INPUT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2B79);
    pub const AUDIO_INPUT_STATUS: Uuid = bluetooth_uuid_from_u16(0x2B7A);
    pub const AUDIO_INPUT_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B7B);
    pub const AUDIO_INPUT_DESCRIPTION: Uuid = bluetooth_uuid_from_u16(0x2B7C);
    pub const VOLUME_STATE: Uuid = bluetooth_uuid_from_u16(0x2B7D);
    pub const VOLUME_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B7E);
    pub const VOLUME_FLAGS: Uuid = bluetooth_uuid_from_u16(0x2B7F);
    pub const VOLUME_OFFSET_STATE: Uuid = bluetooth_uuid_from_u16(0x2B80);
    pub const AUDIO_LOCATION: Uuid = bluetooth_uuid_from_u16(0x2B81);
    pub const VOLUME_OFFSET_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B82);
    pub const AUDIO_OUTPUT_DESCRIPTION: Uuid = bluetooth_uuid_from_u16(0x2B83);
    pub const SET_IDENTITY_RESOLVING_KEY: Uuid = bluetooth_uuid_from_u16(0x2B84);
    pub const COORDINATED_SET_SIZE: Uuid = bluetooth_uuid_from_u16(0x2B85);
    pub const SET_MEMBER_LOCK: Uuid = bluetooth_uuid_from_u16(0x2B86);
    pub const SET_MEMBER_RANK: Uuid = bluetooth_uuid_from_u16(0x2B87);
    pub const DEVICE_TIME_FEATURE: Uuid = bluetooth_uuid_from_u16(0x2B8E);
    pub const DEVICE_TIME_PARAMETERS: Uuid = bluetooth_uuid_from_u16(0x2B8F);
    pub const DEVICE_TIME: Uuid = bluetooth_uuid_from_u16(0x2B90);
    pub const DEVICE_TIME_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2B91);
    pub const TIME_CHANGE_LOG_DATA: Uuid = bluetooth_uuid_from_u16(0x2B92);
    pub const MEDIA_PLAYER_NAME: Uuid = bluetooth_uuid_from_u16(0x2B93);
    pub const MEDIA_PLAYER_ICON_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2B94);
    pub const MEDIA_PLAYER_ICON_URL: Uuid = bluetooth_uuid_from_u16(0x2B95);
    pub const TRACK_CHANGED: Uuid = bluetooth_uuid_from_u16(0x2B96);
    pub const TRACK_TITLE: Uuid = bluetooth_uuid_from_u16(0x2B97);
    pub const TRACK_DURATION: Uuid = bluetooth_uuid_from_u16(0x2B98);
    pub const TRACK_POSITION: Uuid = bluetooth_uuid_from_u16(0x2B99);
    pub const PLAYBACK_SPEED: Uuid = bluetooth_uuid_from_u16(0x2B9A);
    pub const SEEKING_SPEED: Uuid = bluetooth_uuid_from_u16(0x2B9B);
    pub const CURRENT_TRACK_SEGMENTS_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2B9C);
    pub const CURRENT_TRACK_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2B9D);
    pub const NEXT_TRACK_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2B9E);
    pub const PARENT_GROUP_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2B9F);
    pub const CURRENT_GROUP_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2BA0);
    pub const PLAYING_ORDER: Uuid = bluetooth_uuid_from_u16(0x2BA1);
    pub const PLAYING_ORDERS_SUPPORTED: Uuid = bluetooth_uuid_from_u16(0x2BA2);
    pub const MEDIA_STATE: Uuid = bluetooth_uuid_from_u16(0x2BA3);
    pub const MEDIA_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BA4);
    pub const MEDIA_CONTROL_POINT_OPCODES_SUPPORTED: Uuid = bluetooth_uuid_from_u16(0x2BA5);
    pub const SEARCH_RESULTS_OBJECT_ID: Uuid = bluetooth_uuid_from_u16(0x2BA6);
    pub const SEARCH_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BA7);
    pub const MEDIA_PLAYER_ICON_OBJECT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2BA9);
    pub const TRACK_SEGMENTS_OBJECT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2BAA);
    pub const TRACK_OBJECT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2BAB);
    pub const GROUP_OBJECT_TYPE: Uuid = bluetooth_uuid_from_u16(0x2BAC);
    pub const CONSTANT_TONE_EXTENSION_ENABLE: Uuid = bluetooth_uuid_from_u16(0x2BAD);
    pub const ADVERTISING_CONSTANT_TONE_EXTENSION_MINIMUM_LENGTH: Uuid = bluetooth_uuid_from_u16(0x2BAE);
    pub const ADVERTISING_CONSTANT_TONE_EXTENSION_MINIMUM_TRANSMIT_COUNT: Uuid = bluetooth_uuid_from_u16(0x2BAF);
    pub const ADVERTISING_CONSTANT_TONE_EXTENSION_TRANSMIT_DURATION: Uuid = bluetooth_uuid_from_u16(0x2BB0);
    pub const ADVERTISING_CONSTANT_TONE_EXTENSION_INTERVAL: Uuid = bluetooth_uuid_from_u16(0x2BB1);
    pub const ADVERTISING_CONSTANT_TONE_EXTENSION_PHY: Uuid = bluetooth_uuid_from_u16(0x2BB2);
    pub const BEARER_PROVIDER_NAME: Uuid = bluetooth_uuid_from_u16(0x2BB3);
    pub const BEARER_UCI: Uuid = bluetooth_uuid_from_u16(0x2BB4);
    pub const BEARER_TECHNOLOGY: Uuid = bluetooth_uuid_from_u16(0x2BB5);
    pub const BEARER_URI_SCHEMES_SUPPORTED_LIST: Uuid = bluetooth_uuid_from_u16(0x2BB6);
    pub const BEARER_SIGNAL_STRENGTH: Uuid = bluetooth_uuid_from_u16(0x2BB7);
    pub const BEARER_SIGNAL_STRENGTH_REPORTING_INTERVAL: Uuid = bluetooth_uuid_from_u16(0x2BB8);
    pub const BEARER_LIST_CURRENT_CALLS: Uuid = bluetooth_uuid_from_u16(0x2BB9);
    pub const CONTENT_CONTROL_ID: Uuid = bluetooth_uuid_from_u16(0x2BBA);
    pub const STATUS_FLAGS: Uuid = bluetooth_uuid_from_u16(0x2BBB);
    pub const INCOMING_CALL_TARGET_BEARER_URI: Uuid = bluetooth_uuid_from_u16(0x2BBC);
    pub const CALL_STATE: Uuid = bluetooth_uuid_from_u16(0x2BBD);
    pub const CALL_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BBE);
    pub const CALL_CONTROL_POINT_OPTIONAL_OPCODES: Uuid = bluetooth_uuid_from_u16(0x2BBF);
    pub const TERMINATION_REASON: Uuid = bluetooth_uuid_from_u16(0x2BC0);
    pub const INCOMING_CALL: Uuid = bluetooth_uuid_from_u16(0x2BC1);
    pub const CALL_FRIENDLY_NAME: Uuid = bluetooth_uuid_from_u16(0x2BC2);
    pub const MUTE: Uuid = bluetooth_uuid_from_u16(0x2BC3);
    pub const SINK_ASE: Uuid = bluetooth_uuid_from_u16(0x2BC4);
    pub const SOURCE_ASE: Uuid = bluetooth_uuid_from_u16(0x2BC5);
    pub const ASE_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BC6);
    pub const BROADCAST_AUDIO_SCAN_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BC7);
    pub const BROADCAST_RECEIVE_STATE: Uuid = bluetooth_uuid_from_u16(0x2BC8);
    pub const SINK_PAC: Uuid = bluetooth_uuid_from_u16(0x2BC9);
    pub const SINK_AUDIO_LOCATIONS: Uuid = bluetooth_uuid_from_u16(0x2BCA);
    pub const SOURCE_PAC: Uuid = bluetooth_uuid_from_u16(0x2BCB);
    pub const SOURCE_AUDIO_LOCATIONS: Uuid = bluetooth_uuid_from_u16(0x2BCC);
    pub const AVAILABLE_AUDIO_CONTEXTS: Uuid = bluetooth_uuid_from_u16(0x2BCD);
    pub const SUPPORTED_AUDIO_CONTEXTS: Uuid = bluetooth_uuid_from_u16(0x2BCE);
    pub const AMMONIA_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BCF);
    pub const CARBON_MONOXIDE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD0);
    pub const METHANE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD1);
    pub const NITROGEN_DIOXIDE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD2);
    pub const NON_METHANE_VOLATILE_ORGANIC_COMPOUNDS_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD3);
    pub const OZONE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD4);
    pub const PARTICULATE_MATTER_PM1_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD5);
    pub const PARTICULATE_MATTER_PM2_5_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD6);
    pub const PARTICULATE_MATTER_PM10_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD7);
    pub const SULFUR_DIOXIDE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD8);
    pub const SULFUR_HEXAFLUORIDE_CONCENTRATION: Uuid = bluetooth_uuid_from_u16(0x2BD9);
    pub const HEARING_AID_FEATURES: Uuid = bluetooth_uuid_from_u16(0x2BDA);
    pub const HEARING_AID_PRESET_CONTROL_POINT: Uuid = bluetooth_uuid_from_u16(0x2BDB);
    pub const ACTIVE_PRESET_INDEX: Uuid = bluetooth_uuid_from_u16(0x2BDC);
}

/// Bluetooth GATT Descriptor 16-bit UUIDs
#[allow(missing_docs)]
pub mod descriptors {
    use super::bluetooth_uuid_from_u16;
    use crate::Uuid;

    pub const CHARACTERISTIC_EXTENDED_PROPERTIES: Uuid = bluetooth_uuid_from_u16(0x2900);
    pub const CHARACTERISTIC_USER_DESCRIPTION: Uuid = bluetooth_uuid_from_u16(0x2901);
    pub const CLIENT_CHARACTERISTIC_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x2902);
    pub const SERVER_CHARACTERISTIC_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x2903);
    pub const CHARACTERISTIC_PRESENTATION_FORMAT: Uuid = bluetooth_uuid_from_u16(0x2904);
    pub const CHARACTERISTIC_AGGREGATE_FORMAT: Uuid = bluetooth_uuid_from_u16(0x2905);
    pub const VALID_RANGE: Uuid = bluetooth_uuid_from_u16(0x2906);
    pub const EXTERNAL_REPORT_REFERENCE: Uuid = bluetooth_uuid_from_u16(0x2907);
    pub const REPORT_REFERENCE: Uuid = bluetooth_uuid_from_u16(0x2908);
    pub const NUMBER_OF_DIGITALS: Uuid = bluetooth_uuid_from_u16(0x2909);
    pub const VALUE_TRIGGER_SETTING: Uuid = bluetooth_uuid_from_u16(0x290A);
    pub const ENVIRONMENTAL_SENSING_CONFIGURATION: Uuid = bluetooth_uuid_from_u16(0x290B);
    pub const ENVIRONMENTAL_SENSING_MEASUREMENT: Uuid = bluetooth_uuid_from_u16(0x290C);
    pub const ENVIRONMENTAL_SENSING_TRIGGER_SETTING: Uuid = bluetooth_uuid_from_u16(0x290D);
    pub const TIME_TRIGGER_SETTING: Uuid = bluetooth_uuid_from_u16(0x290E);
    pub const COMPLETE_BR_EDR_TRANSPORT_BLOCK_DATA: Uuid = bluetooth_uuid_from_u16(0x290F);
    pub const L2CAPPSM_CHARACTERISTIC: Uuid = Uuid::from_u128(0xABDD3056_28FA_441D_A470_55A75A52553Au128);
}
