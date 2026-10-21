use windows::Devices::Bluetooth::GenericAttributeProfile::GattCommunicationStatus;
use windows::Devices::Enumeration::{DevicePairingResultStatus, DeviceUnpairingResultStatus};
use windows::Foundation::IReference;

use crate::error::ErrorKind;
use crate::Result;

impl From<windows::core::Error> for crate::Error {
    fn from(err: windows::core::Error) -> Self {
        crate::Error::new(ErrorKind::Other, Some(Box::new(err)), String::new())
    }
}

struct CommunicationError(GattCommunicationStatus);

impl std::fmt::Debug for CommunicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommunicationError({})", self)
    }
}

impl std::fmt::Display for CommunicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self.0 {
            GattCommunicationStatus::Success => "success",
            GattCommunicationStatus::AccessDenied => "access denied",
            GattCommunicationStatus::Unreachable => "unreachable",
            GattCommunicationStatus::ProtocolError => "protocol error",
            _ => return write!(f, "unknown ({})", self.0 .0),
        };
        f.write_str(str)
    }
}

impl std::error::Error for CommunicationError {}

fn kind_from_communication_status(
    status: GattCommunicationStatus,
    protocol_error: windows::core::Result<IReference<u8>>,
) -> Result<ErrorKind> {
    match status {
        GattCommunicationStatus::Success => {
            unreachable!("kind_from_communication_status must not be called with GattCommunicationStatus::Success")
        }
        GattCommunicationStatus::AccessDenied => Ok(ErrorKind::NotAuthorized),
        GattCommunicationStatus::Unreachable => Ok(ErrorKind::NotConnected),
        GattCommunicationStatus::ProtocolError => Ok(ErrorKind::Protocol(protocol_error?.Value()?.into())),
        _ => Ok(ErrorKind::Other),
    }
}

pub(super) fn check_communication_status(
    status: GattCommunicationStatus,
    protocol_error: windows::core::Result<IReference<u8>>,
    message: &str,
) -> Result<()> {
    use crate::Error;
    match status {
        GattCommunicationStatus::Success => Ok(()),
        _ => Err(Error::new(
            kind_from_communication_status(status, protocol_error)?,
            Some(Box::new(CommunicationError(status))),
            message,
        )),
    }
}

struct PairingError(DevicePairingResultStatus);

impl std::fmt::Debug for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PairingError({})", self)
    }
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self.0 {
            DevicePairingResultStatus::Paired => "paired",
            DevicePairingResultStatus::AlreadyPaired => "already paired",
            DevicePairingResultStatus::NotReadyToPair => "not ready to pair",
            DevicePairingResultStatus::NotPaired => "not paired",
            DevicePairingResultStatus::ConnectionRejected => "connection rejected",
            DevicePairingResultStatus::TooManyConnections => "too many connections",
            DevicePairingResultStatus::HardwareFailure => "hardware failure",
            DevicePairingResultStatus::AuthenticationTimeout => "authentication timeout",
            DevicePairingResultStatus::AuthenticationNotAllowed => "authentication not allowed",
            DevicePairingResultStatus::AuthenticationFailure => "authentication failure",
            DevicePairingResultStatus::NoSupportedProfiles => "no supported profiles",
            DevicePairingResultStatus::ProtectionLevelCouldNotBeMet => "protection level could not be met",
            DevicePairingResultStatus::AccessDenied => "access denied",
            DevicePairingResultStatus::InvalidCeremonyData => "invalid ceremony data",
            DevicePairingResultStatus::PairingCanceled => "pairing canceled",
            DevicePairingResultStatus::OperationAlreadyInProgress => "operation already in progress",
            DevicePairingResultStatus::RequiredHandlerNotRegistered => "required handler not registered",
            DevicePairingResultStatus::RejectedByHandler => "rejected by handler",
            DevicePairingResultStatus::RemoteDeviceHasAssociation => "remote device has association",
            DevicePairingResultStatus::Failed => "failed",
            _ => return write!(f, "unknown ({})", self.0 .0),
        };
        f.write_str(str)
    }
}

fn kind_from_pairing_status(status: DevicePairingResultStatus) -> ErrorKind {
    match status {
        DevicePairingResultStatus::NotReadyToPair => ErrorKind::NotReady,
        DevicePairingResultStatus::AuthenticationTimeout => ErrorKind::Timeout,
        DevicePairingResultStatus::AuthenticationNotAllowed | DevicePairingResultStatus::AccessDenied => {
            ErrorKind::NotAuthorized
        }
        DevicePairingResultStatus::ConnectionRejected | DevicePairingResultStatus::TooManyConnections => {
            ErrorKind::ConnectionFailed
        }
        DevicePairingResultStatus::NotPaired
        | DevicePairingResultStatus::HardwareFailure
        | DevicePairingResultStatus::AuthenticationFailure
        | DevicePairingResultStatus::NoSupportedProfiles
        | DevicePairingResultStatus::ProtectionLevelCouldNotBeMet
        | DevicePairingResultStatus::InvalidCeremonyData
        | DevicePairingResultStatus::PairingCanceled
        | DevicePairingResultStatus::OperationAlreadyInProgress
        | DevicePairingResultStatus::RequiredHandlerNotRegistered
        | DevicePairingResultStatus::RejectedByHandler
        | DevicePairingResultStatus::RemoteDeviceHasAssociation
        | DevicePairingResultStatus::Failed => ErrorKind::Other,
        _ => ErrorKind::Other,
    }
}

impl std::error::Error for PairingError {}

pub(super) fn check_pairing_status(status: DevicePairingResultStatus) -> Result<()> {
    match status {
        DevicePairingResultStatus::Paired | DevicePairingResultStatus::AlreadyPaired => Ok(()),
        _ => Err(crate::Error::new(
            kind_from_pairing_status(status),
            Some(Box::new(PairingError(status))),
            String::new(),
        )),
    }
}

struct UnpairingError(DeviceUnpairingResultStatus);

impl std::fmt::Debug for UnpairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnpairingError({})", self)
    }
}

impl std::fmt::Display for UnpairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self.0 {
            DeviceUnpairingResultStatus::Unpaired => "unpaired",
            DeviceUnpairingResultStatus::AlreadyUnpaired => "already unpaired",
            DeviceUnpairingResultStatus::OperationAlreadyInProgress => "operation already in progress",
            DeviceUnpairingResultStatus::AccessDenied => "access denied",
            DeviceUnpairingResultStatus::Failed => "failed",
            _ => return write!(f, "unknown ({})", self.0 .0),
        };
        f.write_str(str)
    }
}

fn kind_from_unpairing_status(status: DeviceUnpairingResultStatus) -> ErrorKind {
    match status {
        DeviceUnpairingResultStatus::AccessDenied => ErrorKind::NotAuthorized,
        DeviceUnpairingResultStatus::OperationAlreadyInProgress | DeviceUnpairingResultStatus::Failed => {
            ErrorKind::Other
        }
        _ => ErrorKind::Other,
    }
}

impl std::error::Error for UnpairingError {}

pub(super) fn check_unpairing_status(status: DeviceUnpairingResultStatus) -> Result<()> {
    match status {
        DeviceUnpairingResultStatus::Unpaired | DeviceUnpairingResultStatus::AlreadyUnpaired => Ok(()),
        _ => Err(crate::Error::new(
            kind_from_unpairing_status(status),
            Some(Box::new(UnpairingError(status))),
            String::new(),
        )),
    }
}
