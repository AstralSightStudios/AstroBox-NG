//! Custom Bluetooth pairing agent.

use async_trait::async_trait;

use crate::Device;

/// Bluetooth input/output capabilities for pairing
///
/// See the Bluetooth Core Specification, Vol 3, Part H, §2.3.2
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum IoCapability {
    /// Can display a passkey but not accept user input
    DisplayOnly,
    /// Can display a passkey and request simple confirmation from the user
    DisplayYesNo,
    /// Can request a passkey from the user but not display anything
    KeyboardOnly,
    /// Cannot display anything to or request anything from the user
    NoInputNoOutput,
    /// Can display a passkey to and/or request a passkey or confirmation from the user
    KeyboardDisplay,
}

/// An error indicating the pairing request has been rejected
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct PairingRejected;

impl std::fmt::Display for PairingRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("pairing rejected")
    }
}

impl std::error::Error for PairingRejected {}

/// An error returned when trying to convert an invalid value value into a [`Passkey`]
///
/// `Passkey`s must be a 6-digit numeric value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InvalidPasskey(());

impl std::fmt::Display for InvalidPasskey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid passkey")
    }
}

impl std::error::Error for InvalidPasskey {}

/// A Bluetooth 6-digit passkey
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Passkey(u32);

impl Passkey {
    /// Creates a new `Passkey` from a `u32`
    pub fn new(n: u32) -> Self {
        assert!(n <= 999_999);
        Passkey(n)
    }
}

impl std::fmt::Display for Passkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06}", self.0)
    }
}

impl From<Passkey> for u32 {
    fn from(val: Passkey) -> Self {
        val.0
    }
}

impl std::convert::TryFrom<u32> for Passkey {
    type Error = InvalidPasskey;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value <= 999_999 {
            Ok(Passkey(value))
        } else {
            Err(InvalidPasskey(()))
        }
    }
}

impl std::str::FromStr for Passkey {
    type Err = InvalidPasskey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>()
            .map_err(|_| InvalidPasskey(()))
            .and_then(Passkey::try_from)
    }
}

/// A custom pairing agent responsible for interacting with the user during the peripheral pairing process.
#[async_trait]
pub trait PairingAgent: Send + Sync {
    /// The input/output capabilities of this agent
    fn io_capability(&self) -> IoCapability;

    /// Request pairing confirmation from the user.
    ///
    /// Must be supported if `io_capability` is `DisplayYesNo`, `KeyboardOnly`, `NoInputOutput`, or `KeyboardDisplay`
    async fn confirm(&self, _device: &Device) -> Result<(), PairingRejected> {
        Err(PairingRejected)
    }

    /// Request pairing confirmation from the user. The `passkey` should be displayed for validation.
    ///
    /// Must be supported if `io_capability` is `DisplayYesNo`, `KeyboardOnly`, or `KeyboardDisplay`
    async fn confirm_passkey(&self, _device: &Device, _passkey: Passkey) -> Result<(), PairingRejected> {
        Err(PairingRejected)
    }

    /// Request a 6 digit numeric passkey from the user.
    ///
    /// Must be supported if `io_capability` is `KeyboardOnly` or `KeyboardDisplay`
    async fn request_passkey(&self, _device: &Device) -> Result<Passkey, PairingRejected> {
        Err(PairingRejected)
    }

    /// Display a 6 digit numeric passkey to the user.
    ///
    /// The passkey should be displayed until the async pair operation that triggered this method completes or is
    /// cancelled.
    ///
    /// Must be supported if `io_capability` is `DisplayOnly`, `DisplayYesNo`, or `KeyboardDisplay`
    fn display_passkey(&self, _device: &Device, _passkey: Passkey) {}
}

/// The simplest possible pairing agent.
///
/// This agent does not interact with the user and automatically confirms any pairing requests that do not require
/// input or output. This allows for "JustWorks" pairing which provides encryption but not authentication or protection
/// from man-in-the-middle attacks.
pub struct NoInputOutputPairingAgent;

#[async_trait]
impl PairingAgent for NoInputOutputPairingAgent {
    fn io_capability(&self) -> IoCapability {
        IoCapability::NoInputNoOutput
    }

    async fn confirm(&self, _device: &Device) -> Result<(), PairingRejected> {
        Ok(())
    }
}
