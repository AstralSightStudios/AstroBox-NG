use objc2::rc::Retained;
use objc2_core_bluetooth::CBError;
use objc2_foundation::NSError;

use crate::error::{AttError, ErrorKind};

impl crate::Error {
    pub(super) fn from_recv_error(err: async_broadcast::RecvError) -> Self {
        crate::Error::new(ErrorKind::Internal, Some(Box::new(err)), "receiving delegate event")
    }

    pub(super) fn from_nserror(err: Retained<NSError>) -> Self {
        crate::Error::new(
            kind_from_nserror(&err),
            Some(Box::new(NSErrorError(err))),
            String::new(),
        )
    }

    pub(super) fn from_kind_and_nserror(kind: ErrorKind, err: Option<Retained<NSError>>) -> Self {
        match err {
            Some(err) => crate::Error::new(kind, Some(Box::new(NSErrorError(err))), String::new()),
            None => kind.into(),
        }
    }
}

fn kind_from_nserror(value: &NSError) -> ErrorKind {
    if value.domain().to_string() == "CBErrorDomain" {
        match CBError(value.code()) {
            CBError::OperationNotSupported => ErrorKind::NotSupported,
            CBError::NotConnected | CBError::PeripheralDisconnected => ErrorKind::NotConnected,
            CBError::ConnectionTimeout | CBError::EncryptionTimedOut => ErrorKind::Timeout,
            CBError::InvalidParameters | CBError::InvalidHandle | CBError::UUIDNotAllowed | CBError::UnknownDevice => {
                ErrorKind::InvalidParameter
            }
            CBError::ConnectionFailed
            | CBError::PeerRemovedPairingInformation
            | CBError::ConnectionLimitReached
            | CBError::TooManyLEPairedDevices => ErrorKind::ConnectionFailed,
            CBError::Unknown | CBError::OutOfSpace | CBError::OperationCancelled | CBError::AlreadyAdvertising => {
                ErrorKind::Other
            }
            _ => ErrorKind::Other,
        }
    } else if value.domain().to_string() == "CBATTErrorDomain" {
        let n = value.code();
        if let Ok(n) = u8::try_from(n) {
            ErrorKind::Protocol(AttError::from(n))
        } else {
            ErrorKind::Other
        }
    } else {
        ErrorKind::Other
    }
}

struct NSErrorError(Retained<NSError>);

impl std::fmt::Debug for NSErrorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl std::fmt::Display for NSErrorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.localizedDescription(), f)
    }
}

impl std::error::Error for NSErrorError {}

impl std::ops::Deref for NSErrorError {
    type Target = NSError;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
