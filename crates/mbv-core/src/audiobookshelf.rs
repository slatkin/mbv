use serde::Deserialize;
use std::time::Duration;

#[path = "audiobookshelf_catalog.rs"]
mod audiobookshelf_catalog;
pub use audiobookshelf_catalog::*;
#[path = "audiobookshelf_playback.rs"]
mod audiobookshelf_playback;
pub use audiobookshelf_playback::*;

#[cfg(test)]
#[path = "audiobookshelf_playback_tests.rs"]
mod audiobookshelf_playback_tests;

/// The identity returned by Audiobookshelf's authenticated `/api/me` request.
/// Profile and permission data deliberately stay at the HTTP boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookshelfUser {
    pub id: String,
    pub username: String,
}

/// The validated setup handed to the later transactional lifecycle seam.
/// This type intentionally has no Debug or Display implementation because it
/// retains the candidate API key until the commit seam consumes it.
pub struct AudiobookshelfValidatedSetup {
    pub setup: crate::config::AudiobookshelfSetup,
    pub user: AudiobookshelfUser,
    api_key: String,
}

impl AudiobookshelfValidatedSetup {
    pub fn new(
        setup: crate::config::AudiobookshelfSetup,
        user: AudiobookshelfUser,
        api_key: String,
    ) -> Self {
        Self {
            setup,
            user,
            api_key,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        crate::config::AudiobookshelfSetup,
        AudiobookshelfUser,
        String,
    ) {
        (self.setup, self.user, self.api_key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookshelfFailureClass {
    AuthenticationRejected,
    Connectivity,
    Server,
    Protocol,
    MalformedResponse,
    Unavailable,
}

/// A redacted request failure. It contains a classification only: in
/// particular, no ureq error, URL, header, or response body is retained.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AudiobookshelfError {
    pub class: AudiobookshelfFailureClass,
}

impl std::fmt::Debug for AudiobookshelfError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudiobookshelfError")
            .field("class", &self.class)
            .finish()
    }
}

impl std::fmt::Display for AudiobookshelfError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.class {
            AudiobookshelfFailureClass::AuthenticationRejected => "authentication rejected",
            AudiobookshelfFailureClass::Connectivity => "server unavailable",
            AudiobookshelfFailureClass::Server => "server failure",
            AudiobookshelfFailureClass::Protocol => "unexpected server response",
            AudiobookshelfFailureClass::MalformedResponse => "malformed server response",
            AudiobookshelfFailureClass::Unavailable => "service unavailable",
        })
    }
}

impl std::error::Error for AudiobookshelfError {}

impl From<String> for AudiobookshelfError {
    fn from(_: String) -> Self {
        Self::connectivity()
    }
}

impl AudiobookshelfError {
    pub(crate) const fn from_class(class: AudiobookshelfFailureClass) -> Self {
        Self::new(class)
    }

    const fn new(class: AudiobookshelfFailureClass) -> Self {
        Self { class }
    }

    const fn connectivity() -> Self {
        Self::new(AudiobookshelfFailureClass::Connectivity)
    }

    const fn protocol() -> Self {
        Self::new(AudiobookshelfFailureClass::Protocol)
    }

    const fn malformed() -> Self {
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

#[derive(Clone)]
pub struct AudiobookshelfClient {
    server_url: String,
    agent: ureq::Agent,
}

impl AudiobookshelfClient {
    pub const REQUEST_HARD_BOUND: Duration = Duration::from_secs(5);

    pub fn new(server_url: &str) -> Result<Self, AudiobookshelfError> {
        let server_url = server_url.trim().trim_end_matches('/');
        if server_url.is_empty() {
            return Err(AudiobookshelfError::protocol());
        }
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Self::REQUEST_HARD_BOUND)
            .timeout(Self::REQUEST_HARD_BOUND)
            .build();
        Ok(Self {
            server_url: server_url.to_string(),
            agent,
        })
    }

    /// Validate a new or replacement candidate entirely in memory. No config,
    /// secret, runtime identity, or Service-owned state is touched here.
    pub fn validate_setup_bounded(
        server_url: &str,
        api_key: &str,
        hard_bound: Duration,
    ) -> Result<AudiobookshelfValidatedSetup, AudiobookshelfError> {
        let setup = crate::config::AudiobookshelfSetup::new(server_url);
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
        crate::bounded::run_with_hard_bound(move || client.me(&api_key), hard_bound)
    }

    fn me(&self, api_key: &str) -> Result<AudiobookshelfUser, AudiobookshelfError> {
        let response = self.get(api_key, "/api/me")?;
        let user: AudiobookshelfMeResponse = response
            .into_json()
            .map_err(|_| AudiobookshelfError::malformed())?;
        if !user.is_active || user.id.trim().is_empty() || user.username.trim().is_empty() {
            return Err(AudiobookshelfError::malformed());
        }
        Ok(AudiobookshelfUser {
            id: user.id,
            username: user.username,
        })
    }
}
