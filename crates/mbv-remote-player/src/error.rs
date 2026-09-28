use std::{error::Error, fmt};

#[derive(Debug)]
pub struct RemotePlayerError {
    kind: RemotePlayerErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
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
            source: None,
        }
    }

    fn with_source<E>(kind: RemotePlayerErrorKind, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            message: source.to_string(),
            source: Some(Box::new(source)),
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

impl Error for RemotePlayerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<mbv_ctrl::CtrlError> for RemotePlayerError {
    fn from(source: mbv_ctrl::CtrlError) -> Self {
        Self::with_source(RemotePlayerErrorKind::Control, source)
    }
}

impl From<mbv_config::ConfigError> for RemotePlayerError {
    fn from(source: mbv_config::ConfigError) -> Self {
        Self::with_source(RemotePlayerErrorKind::Connection, source)
    }
}

impl From<std::io::Error> for RemotePlayerError {
    fn from(source: std::io::Error) -> Self {
        Self::with_source(RemotePlayerErrorKind::Connection, source)
    }
}

impl From<serde_json::Error> for RemotePlayerError {
    fn from(source: serde_json::Error) -> Self {
        Self::with_source(RemotePlayerErrorKind::Protocol, source)
    }
}
