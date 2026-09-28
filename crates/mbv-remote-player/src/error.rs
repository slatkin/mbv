use std::{error::Error, fmt};

#[derive(Debug)]
pub struct RemotePlayerError {
    kind: RemotePlayerErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy)]
enum RemotePlayerErrorKind {
    Endpoint,
    Protocol,
    Connection,
    RestartRequired,
    QueueOperation,
    Control,
}

impl RemotePlayerError {
    pub(crate) fn endpoint(message: impl Into<String>) -> Self {
        Self::new(RemotePlayerErrorKind::Endpoint, message)
    }

    pub(crate) fn protocol(message: impl Into<String>) -> Self {
        Self::new(RemotePlayerErrorKind::Protocol, message)
    }

    pub(crate) fn connection(message: impl Into<String>) -> Self {
        Self::new(RemotePlayerErrorKind::Connection, message)
    }

    pub(crate) fn restart_required(message: impl Into<String>) -> Self {
        Self::new(RemotePlayerErrorKind::RestartRequired, message)
    }

    pub(crate) fn queue_operation(message: impl Into<String>) -> Self {
        Self::new(RemotePlayerErrorKind::QueueOperation, message)
    }

    fn new(kind: RemotePlayerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            RemotePlayerErrorKind::Endpoint => "remote-player.endpoint",
            RemotePlayerErrorKind::Protocol => "remote-player.protocol",
            RemotePlayerErrorKind::Connection => "remote-player.connection",
            RemotePlayerErrorKind::RestartRequired => "remote-player.restart_required",
            RemotePlayerErrorKind::QueueOperation => "remote-player.queue_operation",
            RemotePlayerErrorKind::Control => "remote-player.control",
        }
    }
}

impl fmt::Display for RemotePlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RemotePlayerError {}

impl From<mbv_ctrl::CtrlError> for RemotePlayerError {
    fn from(error: mbv_ctrl::CtrlError) -> Self {
        Self::new(RemotePlayerErrorKind::Control, error.to_string())
    }
}

impl From<mbv_config::ConfigError> for RemotePlayerError {
    fn from(error: mbv_config::ConfigError) -> Self {
        Self::connection(error.to_string())
    }
}

impl From<std::io::Error> for RemotePlayerError {
    fn from(error: std::io::Error) -> Self {
        Self::connection(error.to_string())
    }
}

impl From<serde_json::Error> for RemotePlayerError {
    fn from(error: serde_json::Error) -> Self {
        Self::protocol(error.to_string())
    }
}
