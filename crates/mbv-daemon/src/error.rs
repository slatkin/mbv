use std::{error::Error, fmt};

#[derive(Debug)]
pub struct DaemonLibError {
    kind: DaemonLibErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy)]
enum DaemonLibErrorKind {
    OwnerContext,
    QueuePersistence,
    QueueSetup,
    PlaybackLookup,
}

impl DaemonLibError {
    pub(crate) fn owner_context(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::OwnerContext, message)
    }

    pub(crate) fn queue_persistence(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::QueuePersistence, message)
    }

    pub(crate) fn queue_setup(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::QueueSetup, message)
    }

    pub(crate) fn playback_lookup(message: impl Into<String>) -> Self {
        Self::new(DaemonLibErrorKind::PlaybackLookup, message)
    }

    fn new(kind: DaemonLibErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
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

impl Error for DaemonLibError {}

impl From<mbv_emby::EmbyError> for DaemonLibError {
    fn from(error: mbv_emby::EmbyError) -> Self {
        Self::playback_lookup(error.to_string())
    }
}

impl From<mbv_config::ConfigError> for DaemonLibError {
    fn from(error: mbv_config::ConfigError) -> Self {
        Self::queue_persistence(error.to_string())
    }
}

impl From<String> for DaemonLibError {
    fn from(message: String) -> Self {
        Self::queue_setup(message)
    }
}
