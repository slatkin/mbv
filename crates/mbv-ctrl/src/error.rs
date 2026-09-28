use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CtrlError(CtrlErrorKind);

#[derive(Debug)]
enum CtrlErrorKind {
    MissingCapability(String),
    InvalidCredential,
    IncompatibleProtocol(u32),
}

impl CtrlError {
    pub(crate) fn missing_capability(capability: &str) -> Self {
        Self(CtrlErrorKind::MissingCapability(capability.to_owned()))
    }

    pub(crate) fn invalid_credential() -> Self {
        Self(CtrlErrorKind::InvalidCredential)
    }

    pub(crate) fn incompatible_protocol(version: u32) -> Self {
        Self(CtrlErrorKind::IncompatibleProtocol(version))
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.0 {
            CtrlErrorKind::MissingCapability(_) => "ctrl.missing_capability",
            CtrlErrorKind::InvalidCredential => "ctrl.invalid_credential",
            CtrlErrorKind::IncompatibleProtocol(_) => "ctrl.incompatible_protocol",
        }
    }
}

impl fmt::Display for CtrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            CtrlErrorKind::MissingCapability(capability) => {
                write!(f, "peer missing daemon protocol capability: {capability}")
            }
            CtrlErrorKind::InvalidCredential => f.write_str("invalid Control credential"),
            CtrlErrorKind::IncompatibleProtocol(version) => write!(
                f,
                "incompatible daemon protocol version: peer={version} local={}",
                super::CTRL_PROTOCOL_VERSION
            ),
        }
    }
}

impl Error for CtrlError {}

// Older consumers stringify the ctrl handshake error at their boundary.
impl From<CtrlError> for String {
    fn from(error: CtrlError) -> Self {
        error.to_string()
    }
}
