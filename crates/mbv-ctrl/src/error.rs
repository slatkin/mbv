use std::{backtrace::Backtrace, error::Error, fmt};

#[derive(Debug)]
pub struct CtrlError {
    kind: CtrlErrorKind,
    backtrace: Backtrace,
}

#[derive(Debug)]
enum CtrlErrorKind {
    MissingCapability(String),
    InvalidCredential,
    IncompatibleProtocol(u32),
}

impl CtrlError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    pub(crate) fn missing_capability(capability: &str) -> Self {
        Self {
            kind: CtrlErrorKind::MissingCapability(capability.to_owned()),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn invalid_credential() -> Self {
        Self {
            kind: CtrlErrorKind::InvalidCredential,
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn incompatible_protocol(version: u32) -> Self {
        Self {
            kind: CtrlErrorKind::IncompatibleProtocol(version),
            backtrace: Backtrace::capture(),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            CtrlErrorKind::MissingCapability(_) => "ctrl.missing_capability",
            CtrlErrorKind::InvalidCredential => "ctrl.invalid_credential",
            CtrlErrorKind::IncompatibleProtocol(_) => "ctrl.incompatible_protocol",
        }
    }
}

impl fmt::Display for CtrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
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
