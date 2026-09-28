use std::{error::Error, fmt};

#[derive(Debug)]
pub struct DaemonError {
    kind: DaemonErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
enum DaemonErrorKind {
    Failure,
    Usage,
    RestartRequired,
}

impl DaemonError {
    pub(crate) fn failure(message: impl Into<String>) -> Self {
        Self::new(DaemonErrorKind::Failure, message)
    }

    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self::new(DaemonErrorKind::Usage, message)
    }

    pub(crate) fn restart_required(message: impl Into<String>) -> Self {
        Self::new(DaemonErrorKind::RestartRequired, message)
    }

    pub(crate) fn failure_context<E>(context: impl Into<String>, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_context(DaemonErrorKind::Failure, context, source)
    }

    pub(crate) fn restart_context<E>(context: impl Into<String>, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_context(DaemonErrorKind::RestartRequired, context, source)
    }

    fn with_context<E>(kind: DaemonErrorKind, context: impl Into<String>, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            message: format!("{}: {source}", context.into()),
            source: Some(Box::new(source)),
        }
    }

    fn new(kind: DaemonErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            DaemonErrorKind::Failure => "mbvd.failure",
            DaemonErrorKind::Usage => "mbvd.usage",
            DaemonErrorKind::RestartRequired => "mbvd.restart_required",
        }
    }

    #[must_use]
    pub fn is_restart_required(&self) -> bool {
        matches!(self.kind, DaemonErrorKind::RestartRequired)
    }

    #[must_use]
    pub fn is_usage_error(&self) -> bool {
        matches!(self.kind, DaemonErrorKind::Usage)
    }
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for DaemonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<mbv_emby::EmbyError> for DaemonError {
    fn from(source: mbv_emby::EmbyError) -> Self {
        let message = crate::classified_auth_error(source.is_auth()).to_owned();
        Self {
            kind: DaemonErrorKind::Failure,
            message,
            source: Some(Box::new(source)),
        }
    }
}

impl From<mbv_audiobookshelf::AudiobookshelfError> for DaemonError {
    fn from(source: mbv_audiobookshelf::AudiobookshelfError) -> Self {
        let message = crate::classified_abs_error(source.class);
        Self {
            kind: DaemonErrorKind::Failure,
            message,
            source: Some(Box::new(source)),
        }
    }
}

impl From<mbv_config::ConfigError> for DaemonError {
    fn from(source: mbv_config::ConfigError) -> Self {
        Self {
            kind: DaemonErrorKind::Failure,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

impl From<std::io::Error> for DaemonError {
    fn from(source: std::io::Error) -> Self {
        Self {
            kind: DaemonErrorKind::Failure,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

impl From<serde_json::Error> for DaemonError {
    fn from(source: serde_json::Error) -> Self {
        Self {
            kind: DaemonErrorKind::Failure,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}
