use std::{backtrace::Backtrace, error::Error, fmt};

use mbv_ctrl::BuildInfo;

#[derive(Debug)]
pub struct RemotePlayerError {
    kind: RemotePlayerErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
    backtrace: Backtrace,
}

#[derive(Debug)]
enum RemotePlayerErrorKind {
    Endpoint,
    Protocol,
    Connection,
    RestartRequired,
    QueueOperation,
    Control,
    ExclusiveOwner {
        pid: u32,
    },
    OwnerShuttingDown,
    /// Boxed payload: three owned `String`s would push `RemotePlayerError`
    /// past the `result_large_err` threshold. Holds the Owner's reported
    /// identity for the refusal.
    OwnerMismatch(Box<BuildInfo>),
}

impl RemotePlayerError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

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
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn owner_shutting_down() -> Self {
        Self {
            kind: RemotePlayerErrorKind::OwnerShuttingDown,
            message: "the owner is shutting down".to_string(),
            source: None,
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn owner_mismatch(owner: BuildInfo) -> Self {
        Self {
            kind: RemotePlayerErrorKind::OwnerMismatch(Box::new(owner)),
            message: String::new(),
            source: None,
            backtrace: Backtrace::capture(),
        }
    }

    /// Test support: builds the shutting-down error for callers outside this
    /// crate (the GUI Client's attach-outcome tests), gated behind the `test`
    /// feature like the crate's other test seams.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn owner_shutting_down_for_test() -> Self {
        Self::owner_shutting_down()
    }

    /// Test support: builds the exclusive-owner error for callers outside
    /// this crate (the GUI Client's attach-outcome tests), gated behind the
    /// `test` feature like the crate's other test seams.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn exclusive_owner_for_test(pid: u32) -> Self {
        Self::exclusive_owner(pid)
    }

    /// Test support: builds the mismatch error for callers outside this crate
    /// (the `mbv` binary's startup-prompt tests and the GUI Client's attach
    /// tests), gated behind the `test` feature like the crate's other test
    /// seams.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn owner_mismatch_for_test(owner: BuildInfo) -> Self {
        Self::owner_mismatch(owner)
    }

    fn new(kind: RemotePlayerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
            backtrace: Backtrace::capture(),
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
            backtrace: Backtrace::capture(),
        }
    }

    #[must_use]
    pub fn is_owner_shutting_down(&self) -> bool {
        matches!(&self.kind, RemotePlayerErrorKind::OwnerShuttingDown)
    }

    /// The error is a local Owner mismatch: the Owner's version or wire
    /// identity differs from this binary's, so nothing identifying was sent
    /// (invariant 15).
    #[must_use]
    pub fn is_owner_mismatch(&self) -> bool {
        matches!(&self.kind, RemotePlayerErrorKind::OwnerMismatch(_))
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
            RemotePlayerErrorKind::OwnerMismatch(_) => "remote-player.owner_mismatch",
        }
    }
}

/// One side's identity as the mismatch text shows it: application version,
/// build identity, and wire identity. An empty value (an Owner that predates
/// a field) shows as `unknown`.
fn describe_identity(build: &BuildInfo) -> String {
    let or_unknown = |value: &str| {
        if value.is_empty() {
            "unknown".to_string()
        } else {
            value.to_string()
        }
    };
    format!(
        "version {} (build {}, wire {})",
        or_unknown(&build.app_version),
        or_unknown(&build.build_id),
        or_unknown(&build.wire_id),
    )
}

/// Canonical user-facing text for a local Owner mismatch: the Owner's and
/// this Client's identities, then how to proceed. The startup prompt and
/// every path that only prints the error render this one text, so the
/// guidance cannot drift between them.
fn owner_mismatch_message(owner: &BuildInfo) -> String {
    format!(
        "Owner process is running {}, but this terminal is {}; stop it with `mbv -q`, then relaunch mbv",
        describe_identity(owner),
        describe_identity(&BuildInfo::current()),
    )
}

impl fmt::Display for RemotePlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RemotePlayerErrorKind::ExclusiveOwner { pid } => {
                write!(f, "local owner process {pid} already has a client")
            }
            RemotePlayerErrorKind::OwnerMismatch(owner) => {
                f.write_str(&owner_mismatch_message(owner))
            }
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
