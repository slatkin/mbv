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
    BoundedTimeout,
    Http,
    Parse,
    Playlist,
    Playback,
    Resolve,
}

impl EmbyError {
    pub(crate) fn auth(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::Auth, message)
    }

    pub(crate) fn playlist(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::Playlist, message)
    }

    #[must_use]
    pub fn playback(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::Playback, message)
    }

    /// A browse/reveal lookup failure inside the app: the server was not the
    /// problem, the requested item or path simply cannot be resolved.
    #[must_use]
    pub fn resolve(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::Resolve, message)
    }

    pub(crate) fn bounded_timeout(message: impl Into<String>) -> Self {
        Self::new(EmbyErrorKind::BoundedTimeout, message)
    }

    pub(crate) fn playback_context<E>(context: &str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_context(EmbyErrorKind::Playback, context, source)
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
            EmbyErrorKind::Timeout | EmbyErrorKind::BoundedTimeout => "emby.timeout",
            EmbyErrorKind::Http => "emby.http",
            EmbyErrorKind::Parse => "emby.parse",
            EmbyErrorKind::Playlist => "emby.playlist",
            EmbyErrorKind::Playback => "emby.playback",
            EmbyErrorKind::Resolve => "emby.resolve",
        }
    }

    #[must_use]
    pub fn is_auth(&self) -> bool {
        matches!(self.kind, EmbyErrorKind::Auth)
    }

    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(
            self.kind,
            EmbyErrorKind::Timeout | EmbyErrorKind::BoundedTimeout
        )
    }

    #[must_use]
    pub fn is_bounded_timeout(&self) -> bool {
        matches!(self.kind, EmbyErrorKind::BoundedTimeout)
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

impl From<crate::EmbyFailure> for EmbyError {
    fn from(failure: crate::EmbyFailure) -> Self {
        let kind = match failure.class {
            crate::EmbyFailureClass::AuthenticationRejected => EmbyErrorKind::Auth,
            crate::EmbyFailureClass::Unavailable => EmbyErrorKind::Http,
        };
        Self::new(kind, failure.message)
    }
}
