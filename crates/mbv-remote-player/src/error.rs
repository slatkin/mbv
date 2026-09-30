use std::{error::Error, fmt};

#[derive(Debug)]
pub struct RemotePlayerError {
    kind: RemotePlayerErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone)]
enum RemotePlayerErrorKind {
    Endpoint,
    Protocol,
    Connection,
    RestartRequired,
    QueueOperation,
    Control,
    ExclusiveOwner { pid: u32 },
    OwnerShuttingDown,
    OwnerBuildMismatch { app_version: String },
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

    pub(crate) fn exclusive_owner(pid: u32) -> Self {
        Self {
            kind: RemotePlayerErrorKind::ExclusiveOwner { pid },
            message: String::new(),
            source: None,
        }
    }

    pub(crate) fn owner_shutting_down() -> Self {
        Self {
            kind: RemotePlayerErrorKind::OwnerShuttingDown,
            message: "the owner is shutting down".to_string(),
            source: None,
        }
    }

    pub(crate) fn owner_build_mismatch_error(app_version: impl Into<String>) -> Self {
        Self {
            kind: RemotePlayerErrorKind::OwnerBuildMismatch {
                app_version: app_version.into(),
            },
            message: String::new(),
            source: None,
        }
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
    pub fn is_owner_shutting_down(&self) -> bool {
        matches!(&self.kind, RemotePlayerErrorKind::OwnerShuttingDown)
    }

    #[must_use]
    pub fn owner_build_mismatch(&self) -> Option<&str> {
        match &self.kind {
            RemotePlayerErrorKind::OwnerBuildMismatch { app_version } => Some(app_version),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_exclusive_owner(&self) -> bool {
        matches!(&self.kind, RemotePlayerErrorKind::ExclusiveOwner { .. })
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            RemotePlayerErrorKind::Endpoint => "remote-player.endpoint",
            RemotePlayerErrorKind::Protocol => "remote-player.protocol",
            RemotePlayerErrorKind::Connection => "remote-player.connection",
            RemotePlayerErrorKind::RestartRequired => "remote-player.restart_required",
            RemotePlayerErrorKind::QueueOperation => "remote-player.queue_operation",
            RemotePlayerErrorKind::Control => "remote-player.control",
            RemotePlayerErrorKind::ExclusiveOwner { .. } => "remote-player.exclusive_owner",
            RemotePlayerErrorKind::OwnerShuttingDown => "remote-player.owner_shutting_down",
            RemotePlayerErrorKind::OwnerBuildMismatch { .. } => {
                "remote-player.owner_build_mismatch"
            }
        }
    }
}

impl fmt::Display for RemotePlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RemotePlayerErrorKind::ExclusiveOwner { pid } => {
                write!(f, "local owner process {pid} already has a client")
            }
            RemotePlayerErrorKind::OwnerBuildMismatch { app_version } => write!(
                f,
                "Owner process is running version {app_version}, but this mbv is version {}; run `mbv -q` to restart it",
                env!("CARGO_PKG_VERSION")
            ),
            _ => f.write_str(&self.message),
        }
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
