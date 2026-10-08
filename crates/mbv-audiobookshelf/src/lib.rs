//! `Audiobookshelf` Service client: catalog, playback, and socket protocol.
//!
//! Podcast and book catalogs, playback sessions, the socket worker, and setup
//! validation live here. Queue snapshots live in `mbv-queue`; shared HTTP and
//! retry primitives live in `mbv-net`.

use serde::Deserialize;
use std::backtrace::Backtrace;
use std::time::Duration;

mod catalog;
#[doc(inline)]
pub use catalog::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShelf, AudiobookshelfShelfEntry, AudiobookshelfShow, AudiobookshelfShowPage,
};
#[cfg(test)]
pub(crate) use catalog::{
    ExpandedWire, ItemsResponse, LibrariesResponse, ProgressResponse, ShelfEntryWire, ShelfWire,
    ShowWire, published_at_secs, shelf_entry_from_wire,
};
mod catalog_books;
#[doc(inline)]
pub use catalog_books::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookPage,
    AudiobookshelfBookProgress, AudiobookshelfChapter, AuthorWire, BookMediaWire, BookMetadataWire,
    BookWire, BooksResponse, audiobook_author_sort_key, book_author_display,
    first_listed_author_sort_key,
};
mod playback;
#[doc(inline)]
pub use playback::{
    AudiobookshelfAudioSource, AudiobookshelfBookPlaybackSession, AudiobookshelfPlaybackProgress,
    AudiobookshelfPlaybackSession, AudiobookshelfSourceMethod,
};
pub mod socket;

#[cfg(test)]
mod tests;

/// The identity returned by Audiobookshelf's authenticated `/api/me` request.
/// Profile and permission data deliberately stay at the HTTP boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookshelfUser {
    pub id: String,
    pub username: String,
}

/// The validated setup handed to the later transactional lifecycle seam.
/// Its Debug implementation redacts the candidate API key, which this type
/// retains until the commit seam consumes it.
pub struct AudiobookshelfValidatedSetup {
    pub setup: mbv_config::AudiobookshelfSetup,
    pub user: AudiobookshelfUser,
    api_key: String,
}

impl std::fmt::Debug for AudiobookshelfValidatedSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudiobookshelfValidatedSetup")
            .field("setup", &self.setup)
            .field("user", &self.user)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl AudiobookshelfValidatedSetup {
    #[must_use]
    pub fn new(
        setup: mbv_config::AudiobookshelfSetup,
        user: AudiobookshelfUser,
        api_key: String,
    ) -> Self {
        Self {
            setup,
            user,
            api_key,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (mbv_config::AudiobookshelfSetup, AudiobookshelfUser, String) {
        (self.setup, self.user, self.api_key)
    }
}

/// Consume a validator result only after validation has succeeded. The
/// returned identity is runtime-only and is never serialized by this seam.
pub fn commit_audiobookshelf_candidate(
    candidate: AudiobookshelfValidatedSetup,
) -> Result<(AudiobookshelfUser, u64), AudiobookshelfError> {
    let (setup, user, api_key) = candidate.into_parts();
    let revision = mbv_config::persist_audiobookshelf_setup_and_secret(&setup, &api_key)?;
    Ok((user, revision))
}

pub fn repair_audiobookshelf_candidate(
    candidate: AudiobookshelfValidatedSetup,
) -> Result<(AudiobookshelfUser, u64), AudiobookshelfError> {
    commit_audiobookshelf_candidate(candidate)
}

/// Confirmed different-server replacement. Validation is represented by the
/// candidate type; confirmation belongs to the caller and must precede this
/// destructive boundary.
pub fn replace_audiobookshelf_candidate<C, R>(
    candidate: AudiobookshelfValidatedSetup,
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<(AudiobookshelfUser, u64), AudiobookshelfError>
where
    C: FnOnce() -> Result<(), AudiobookshelfError>,
    R: FnOnce(),
{
    let (setup, user, api_key) = candidate.into_parts();
    let revision = mbv_config::replace_audiobookshelf_setup_and_secret(
        &setup,
        &api_key,
        clear_owned_state,
        restore_owned_state,
    )?;
    Ok((user, revision))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookshelfFailureClass {
    AuthenticationRejected,
    Connectivity,
    Server,
    Protocol,
    MalformedResponse,
    Unavailable,
    Persistence,
}

/// A redacted request failure. It contains a classification only: in
/// particular, no ureq error, URL, header, or response body is retained.
pub struct AudiobookshelfError {
    pub class: AudiobookshelfFailureClass,
    message: Option<String>,
    backtrace: Backtrace,
}

impl std::fmt::Debug for AudiobookshelfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudiobookshelfError")
            .field("class", &self.class)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for AudiobookshelfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = &self.message {
            return f.write_str(message);
        }
        f.write_str(match self.class {
            AudiobookshelfFailureClass::AuthenticationRejected => "authentication rejected",
            AudiobookshelfFailureClass::Connectivity => "server unavailable",
            AudiobookshelfFailureClass::Server => "server failure",
            AudiobookshelfFailureClass::Protocol => "unexpected server response",
            AudiobookshelfFailureClass::MalformedResponse => "malformed server response",
            AudiobookshelfFailureClass::Unavailable => "service unavailable",
            AudiobookshelfFailureClass::Persistence => "persistence failure",
        })
    }
}

impl std::error::Error for AudiobookshelfError {}

impl From<mbv_config::ConfigError> for AudiobookshelfError {
    fn from(error: mbv_config::ConfigError) -> Self {
        Self::with_message(AudiobookshelfFailureClass::Persistence, error.to_string())
    }
}

impl AudiobookshelfError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.class {
            AudiobookshelfFailureClass::AuthenticationRejected => {
                "audiobookshelf.authentication_rejected"
            }
            AudiobookshelfFailureClass::Connectivity => "audiobookshelf.connectivity",
            AudiobookshelfFailureClass::Server => "audiobookshelf.server",
            AudiobookshelfFailureClass::Protocol => "audiobookshelf.protocol",
            AudiobookshelfFailureClass::MalformedResponse => "audiobookshelf.malformed_response",
            AudiobookshelfFailureClass::Unavailable => "audiobookshelf.unavailable",
            AudiobookshelfFailureClass::Persistence => "audiobookshelf.persistence",
        }
    }

    #[must_use]
    pub const fn is_persistence(&self) -> bool {
        matches!(self.class, AudiobookshelfFailureClass::Persistence)
    }

    #[must_use]
    pub const fn is_authentication_rejected(&self) -> bool {
        matches!(
            self.class,
            AudiobookshelfFailureClass::AuthenticationRejected
        )
    }

    #[must_use]
    pub fn from_class(class: AudiobookshelfFailureClass) -> Self {
        Self::new(class)
    }

    fn new(class: AudiobookshelfFailureClass) -> Self {
        Self {
            class,
            message: None,
            backtrace: Backtrace::capture(),
        }
    }

    pub fn persistence(message: impl Into<String>) -> Self {
        Self::with_message(AudiobookshelfFailureClass::Persistence, message)
    }

    pub(crate) fn timeout() -> Self {
        Self::with_message(
            AudiobookshelfFailureClass::Unavailable,
            "server unavailable",
        )
    }

    fn with_message(class: AudiobookshelfFailureClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: Some(message.into()),
            backtrace: Backtrace::capture(),
        }
    }

    fn connectivity() -> Self {
        Self::new(AudiobookshelfFailureClass::Connectivity)
    }

    fn protocol() -> Self {
        Self::new(AudiobookshelfFailureClass::Protocol)
    }

    fn malformed() -> Self {
        Self::new(AudiobookshelfFailureClass::MalformedResponse)
    }
}

#[derive(Debug, Deserialize)]
struct AudiobookshelfMeResponse {
    id: String,
    username: String,
    #[serde(rename = "isActive")]
    is_active: bool,
}

#[derive(Clone, Debug)]
pub struct AudiobookshelfClient {
    server_url: String,
    agent: ureq::Agent,
}

impl AudiobookshelfClient {
    pub const REQUEST_HARD_BOUND: Duration = Duration::from_secs(5);

    pub fn new(server_url: impl AsRef<str>) -> Result<Self, AudiobookshelfError> {
        let server_url = server_url.as_ref().trim().trim_end_matches('/');
        if server_url.is_empty() {
            return Err(AudiobookshelfError::protocol());
        }
        let agent = mbv_net::native_tls_agent(
            mbv_net::HttpService::Audiobookshelf,
            Some(Self::REQUEST_HARD_BOUND),
            Some(Self::REQUEST_HARD_BOUND),
        );
        Ok(Self {
            server_url: server_url.to_string(),
            agent,
        })
    }

    /// Install an in-memory mock transport (see `mock_http`).
    #[must_use]
    pub fn with_test_agent(mut self, agent: ureq::Agent) -> Self {
        self.agent = agent;
        self
    }

    /// Validate a new or replacement candidate entirely in memory. No config,
    /// secret, runtime identity, or Service-owned state is touched here.
    pub fn validate_setup_bounded(
        server_url: impl AsRef<str>,
        api_key: impl AsRef<str>,
        hard_bound: Duration,
    ) -> Result<AudiobookshelfValidatedSetup, AudiobookshelfError> {
        let server_url = server_url.as_ref();
        let api_key = api_key.as_ref();
        let setup = mbv_config::AudiobookshelfSetup::new(server_url);
        if setup.server_url.is_empty() || api_key.trim().is_empty() {
            return Err(AudiobookshelfError::protocol());
        }
        let client = Self::new(&setup.server_url)?;
        let user = client.me_bounded(api_key, hard_bound)?;
        Ok(AudiobookshelfValidatedSetup {
            setup,
            user,
            api_key: api_key.to_string(),
        })
    }

    pub fn me_bounded(
        &self,
        api_key: &str,
        hard_bound: Duration,
    ) -> Result<AudiobookshelfUser, AudiobookshelfError> {
        let api_key = api_key.to_string();
        if api_key.trim().is_empty() {
            return Err(AudiobookshelfError::protocol());
        }
        let client = self.clone();
        mbv_net::bounded::run_with_hard_bound_or_error(
            move || client.me(&api_key),
            AudiobookshelfError::timeout,
            hard_bound,
        )
    }

    fn me(&self, api_key: &str) -> Result<AudiobookshelfUser, AudiobookshelfError> {
        let mut response = self.get(api_key, "/api/me")?;
        let user: AudiobookshelfMeResponse = response.body_mut().read_json().map_err(|error| {
            tracing::debug!(name: "audiobookshelf.user.response_invalid", target: "audiobookshelf", error = %error, "invalid user response");
            AudiobookshelfError::malformed()
        })?;
        if !user.is_active || user.id.trim().is_empty() || user.username.trim().is_empty() {
            return Err(AudiobookshelfError::malformed());
        }
        Ok(AudiobookshelfUser {
            id: user.id,
            username: user.username,
        })
    }
}
