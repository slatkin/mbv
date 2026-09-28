use std::{error::Error, fmt, io};

#[derive(Debug)]
pub struct FeedError(FeedErrorKind);

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
    pub(crate) fn http(source: ureq::Error) -> Self {
        Self(FeedErrorKind::Http(source))
    }

    pub(crate) fn read_body(context: &'static str, source: ureq::Error) -> Self {
        Self(FeedErrorKind::ReadBody(context, source))
    }

    pub(crate) fn invalid_youtube_url() -> Self {
        Self(FeedErrorKind::InvalidYoutubeUrl)
    }

    pub(crate) fn youtube_resolve(source: ureq::Error) -> Self {
        Self(FeedErrorKind::YoutubeResolve(source))
    }

    pub(crate) fn missing_youtube_feed() -> Self {
        Self(FeedErrorKind::MissingYoutubeFeed)
    }

    pub(crate) fn create_directory(path: String, source: io::Error) -> Self {
        Self(FeedErrorKind::CreateDirectory { path, source })
    }

    pub(crate) fn serialize(source: serde_json::Error) -> Self {
        Self(FeedErrorKind::Serialize(source))
    }

    pub(crate) fn write(path: String, source: io::Error) -> Self {
        Self(FeedErrorKind::Write { path, source })
    }

    pub(crate) fn rename(from: String, to: String, source: io::Error) -> Self {
        Self(FeedErrorKind::Rename { from, to, source })
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.0 {
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
        match &self.0 {
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
        match &self.0 {
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
