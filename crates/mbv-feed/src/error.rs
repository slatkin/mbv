use std::{backtrace::Backtrace, error::Error, fmt, io};

#[derive(Debug)]
pub struct FeedError {
    kind: FeedErrorKind,
    backtrace: Backtrace,
}

#[derive(Debug)]
enum FeedErrorKind {
    Http(ureq::Error),
    ReadBody(&'static str, ureq::Error),
    InvalidYoutubeUrl,
    YoutubeResolve(ureq::Error),
    MissingYoutubeFeed,
    CreateDirectory {
        path: String,
        source: io::Error,
    },
    Serialize(serde_json::Error),
    Write {
        path: String,
        source: io::Error,
    },
    Rename {
        from: String,
        to: String,
        source: io::Error,
    },
}

impl FeedError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    pub(crate) fn http(source: ureq::Error) -> Self {
        Self {
            kind: FeedErrorKind::Http(source),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn read_body(context: &'static str, source: ureq::Error) -> Self {
        Self {
            kind: FeedErrorKind::ReadBody(context, source),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn invalid_youtube_url() -> Self {
        Self {
            kind: FeedErrorKind::InvalidYoutubeUrl,
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn youtube_resolve(source: ureq::Error) -> Self {
        Self {
            kind: FeedErrorKind::YoutubeResolve(source),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn missing_youtube_feed() -> Self {
        Self {
            kind: FeedErrorKind::MissingYoutubeFeed,
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn create_directory(path: String, source: io::Error) -> Self {
        Self {
            kind: FeedErrorKind::CreateDirectory { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn serialize(source: serde_json::Error) -> Self {
        Self {
            kind: FeedErrorKind::Serialize(source),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn write(path: String, source: io::Error) -> Self {
        Self {
            kind: FeedErrorKind::Write { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn rename(from: String, to: String, source: io::Error) -> Self {
        Self {
            kind: FeedErrorKind::Rename { from, to, source },
            backtrace: Backtrace::capture(),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            FeedErrorKind::Http(_) => "feed.http",
            FeedErrorKind::ReadBody(_, _) => "feed.read_body",
            FeedErrorKind::InvalidYoutubeUrl => "feed.invalid_youtube_url",
            FeedErrorKind::YoutubeResolve(_) => "feed.youtube_resolve",
            FeedErrorKind::MissingYoutubeFeed => "feed.missing_youtube_feed",
            FeedErrorKind::CreateDirectory { .. } => "feed.create_directory",
            FeedErrorKind::Serialize(_) => "feed.serialize",
            FeedErrorKind::Write { .. } => "feed.write",
            FeedErrorKind::Rename { .. } => "feed.rename",
        }
    }
}

impl fmt::Display for FeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FeedErrorKind::Http(error) => write!(f, "HTTP request failed: {error}"),
            FeedErrorKind::ReadBody(context, error) => write!(f, "{context}: {error}"),
            FeedErrorKind::InvalidYoutubeUrl => {
                f.write_str("URL is not a resolvable YouTube channel URL")
            }
            FeedErrorKind::YoutubeResolve(error) => {
                write!(f, "Failed to resolve YouTube channel: {error}")
            }
            FeedErrorKind::MissingYoutubeFeed => {
                f.write_str("YouTube channel page did not contain an RSS feed URL")
            }
            FeedErrorKind::CreateDirectory { path, source } => {
                write!(f, "create directory {path}: {source}")
            }
            FeedErrorKind::Serialize(error) => {
                write!(f, "serialize feed entry state: {error}")
            }
            FeedErrorKind::Write { path, source } => write!(f, "write {path}: {source}"),
            FeedErrorKind::Rename { from, to, source } => {
                write!(f, "rename {from} to {to}: {source}")
            }
        }
    }
}

impl Error for FeedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FeedErrorKind::Http(source)
            | FeedErrorKind::YoutubeResolve(source)
            | FeedErrorKind::ReadBody(_, source) => Some(source),
            FeedErrorKind::CreateDirectory { source, .. }
            | FeedErrorKind::Write { source, .. }
            | FeedErrorKind::Rename { source, .. } => Some(source),
            FeedErrorKind::Serialize(source) => Some(source),
            FeedErrorKind::InvalidYoutubeUrl | FeedErrorKind::MissingYoutubeFeed => None,
        }
    }
}

impl From<ureq::Error> for FeedError {
    fn from(source: ureq::Error) -> Self {
        Self::http(source)
    }
}

impl From<serde_json::Error> for FeedError {
    fn from(source: serde_json::Error) -> Self {
        Self::serialize(source)
    }
}
