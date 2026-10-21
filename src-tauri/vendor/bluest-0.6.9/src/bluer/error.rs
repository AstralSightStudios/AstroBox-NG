use crate::error::ErrorKind;

impl From<bluer::Error> for crate::Error {
    fn from(err: bluer::Error) -> Self {
        crate::Error::new(kind_from_bluer(&err), Some(Box::new(err)), String::new())
    }
}

fn kind_from_bluer(err: &bluer::Error) -> ErrorKind {
    match err.kind {
        bluer::ErrorKind::ConnectionAttemptFailed => ErrorKind::ConnectionFailed,
        bluer::ErrorKind::Failed => ErrorKind::Other,
        bluer::ErrorKind::InvalidArguments => ErrorKind::InvalidParameter,
        bluer::ErrorKind::InvalidLength => ErrorKind::InvalidParameter,
        bluer::ErrorKind::NotAuthorized => ErrorKind::NotAuthorized,
        bluer::ErrorKind::NotReady => ErrorKind::NotReady,
        bluer::ErrorKind::NotSupported => ErrorKind::NotSupported,
        bluer::ErrorKind::NotPermitted => ErrorKind::NotAuthorized,
        bluer::ErrorKind::InvalidOffset => ErrorKind::InvalidParameter,
        bluer::ErrorKind::InvalidAddress(_) => ErrorKind::InvalidParameter,
        bluer::ErrorKind::InvalidName(_) => ErrorKind::InvalidParameter,
        bluer::ErrorKind::ServicesUnresolved => ErrorKind::NotReady,
        bluer::ErrorKind::NotFound => ErrorKind::NotFound,
        _ => ErrorKind::Other,
    }
}
