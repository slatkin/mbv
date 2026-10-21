use std::{backtrace::Backtrace, error::Error, fmt};

use crate::context::DaemonRole;
use crate::owner_lock::OwnerLockError;

#[derive(Debug)]
pub struct DaemonLibError {
    kind: DaemonLibErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
    backtrace: Backtrace,
}

#[derive(Debug, Clone, Copy)]
enum DaemonLibErrorKind {
    OwnerContext,
    QueuePersistence,
    QueueSetup,
    PlaybackLookup,
}

impl DaemonLibError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    pub(crate) fn owner_context(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::OwnerContext, message)
    }

    pub(crate) fn queue_setup(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::QueueSetup, message)
    }

    fn new(kind: DaemonLibErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
            backtrace: Backtrace::capture(),
        }
    }

    fn with_source<E>(kind: DaemonLibErrorKind, source: E) -> Self
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
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            DaemonLibErrorKind::OwnerContext => "daemon.owner_context",
            DaemonLibErrorKind::QueuePersistence => "daemon.queue_persistence",
            DaemonLibErrorKind::QueueSetup => "daemon.queue_setup",
            DaemonLibErrorKind::PlaybackLookup => "daemon.playback_lookup",
        }
    }
}

impl fmt::Display for DaemonLibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for DaemonLibError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<mbv_emby::EmbyError> for DaemonLibError {
    fn from(source: mbv_emby::EmbyError) -> Self {
        Self::with_source(DaemonLibErrorKind::PlaybackLookup, source)
    }
}

impl From<mbv_config::ConfigError> for DaemonLibError {
    fn from(source: mbv_config::ConfigError) -> Self {
        Self::with_source(DaemonLibErrorKind::QueuePersistence, source)
    }
}

/// Why the Local run entry point refused to start a daemon.
#[derive(Debug)]
pub enum LocalStartupError {
    /// The startup context named a role the Local entry does not run. The
    /// Local entry runs only the Local role; the packaged entry
    /// (`run_with_options`) starts the packaged role, and no remote output
    /// can be attached to it.
    NotLocal(DaemonRole),
    /// The daemon could not take its owner PID lock.
    OwnerLock(OwnerLockError),
}

impl fmt::Display for LocalStartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLocal(role) => write!(
                f,
                "the Local daemon entry point runs only the Local role, not the {role:?} role"
            ),
            Self::OwnerLock(source) => fmt::Display::fmt(source, f),
        }
    }
}

impl Error for LocalStartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotLocal(_) => None,
            Self::OwnerLock(source) => Some(source),
        }
    }
}

impl From<OwnerLockError> for LocalStartupError {
    fn from(source: OwnerLockError) -> Self {
        Self::OwnerLock(source)
    }
}
