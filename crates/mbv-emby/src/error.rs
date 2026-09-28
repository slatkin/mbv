use std::{error::Error, fmt};

#[derive(Debug)]
pub struct EmbyError {
    kind: EmbyErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
enum EmbyErrorKind {
    Auth,
    Timeout,
    Http,
    Parse,
}

impl EmbyError {
    pub(crate) fn auth(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::Auth, message)
    }

    pub(crate) fn auth_context<E>(context: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_context(EmbyErrorKind::Auth, context, source)
    }

    fn with_context<E>(kind: EmbyErrorKind, context: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            message: format!("{context}: {source}"),
            source: Some(Box::new(source)),
        }
    }

    fn new(kind: EmbyErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            EmbyErrorKind::Auth => "emby.auth",
            EmbyErrorKind::Timeout => "emby.timeout",
            EmbyErrorKind::Http => "emby.http",
            EmbyErrorKind::Parse => "emby.parse",
        }
    }

    #[must_use]
    pub fn is_auth(&self) -> bool {
        matches!(self.kind, EmbyErrorKind::Auth)
    }

    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self.kind, EmbyErrorKind::Timeout)
    }
}

impl fmt::Display for EmbyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for EmbyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<ureq::Error> for EmbyError {
    fn from(source: ureq::Error) -> Self {
        let kind = if matches!(source, ureq::Error::Timeout(_)) {
            EmbyErrorKind::Timeout
        } else {
            EmbyErrorKind::Http
        };
        let message = source.to_string();
        Self {
            kind,
            message,
            source: Some(Box::new(source)),
        }
    }
}

impl From<serde_json::Error> for EmbyError {
    fn from(source: serde_json::Error) -> Self {
        let message = source.to_string();
        Self {
            kind: EmbyErrorKind::Parse,
            message,
            source: Some(Box::new(source)),
        }
    }
}

impl From<String> for EmbyError {
    fn from(message: String) -> Self {
        Self::new(EmbyErrorKind::Timeout, message)
    }
}
