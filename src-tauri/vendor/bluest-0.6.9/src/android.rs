use java_spaghetti::{CastError, Local};

use self::bindings::java::lang::Throwable;
use crate::error::ErrorKind;

pub mod adapter;
pub mod characteristic;
pub mod descriptor;
pub mod device;
pub mod l2cap_channel;
pub mod service;

pub(crate) mod bindings;

/// A platform-specific device identifier.
/// On android it contains the Bluetooth address in the format `AB:CD:EF:01:23:45`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceId(pub(crate) String);

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl From<Local<'_, Throwable>> for crate::Error {
    fn from(e: Local<'_, Throwable>) -> Self {
        Self::new(ErrorKind::Internal, None, format!("{:?}", e))
    }
}

impl From<CastError> for crate::Error {
    fn from(e: CastError) -> Self {
        Self::new(ErrorKind::Internal, None, format!("{:?}", e))
    }
}

struct JavaIterator<'env>(Local<'env, bindings::java::util::Iterator>);

impl<'env> Iterator for JavaIterator<'env> {
    type Item = Local<'env, bindings::java::lang::Object>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.hasNext().unwrap() {
            let obj = self.0.next().unwrap().unwrap();
            // upgrade lifetime to the original env.
            let obj = unsafe { Local::from_raw(self.0.env(), obj.into_raw()) };
            Some(obj)
        } else {
            None
        }
    }
}

trait OptionExt<T> {
    fn non_null(self) -> Result<T, crate::Error>;
}

impl<T> OptionExt<T> for Option<T> {
    #[track_caller]
    fn non_null(self) -> Result<T, crate::Error> {
        self.ok_or_else(|| crate::Error::new(ErrorKind::Internal, None, "Java call unexpectedly returned null."))
    }
}
