// Service setup types for `Config`. Included into `config`'s module scope
// (see `config.rs`), so callers reach them as `crate::config::…`.

/// Emby-specific setup (server URL + user ID) stored in config.toml `[server]`.
/// The corresponding API token is stored separately in a per-Service mode-0600
/// secret file, never in config.toml. The flat `server_url`/`username`/
/// `password`/`api_key` fields on `Config` serve backward compat during
/// migration and will be removed once all callers go through `EmbySetup`
/// and runtime service state.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EmbySetup {
    pub server_url: String,
    pub user_id: String,
    #[serde(default = "default_emby_setup_revision")]
    pub revision: u64,
}

const fn default_emby_setup_revision() -> u64 {
    1
}

impl EmbySetup {
    pub fn new(server_url: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim().trim_end_matches('/').to_string(),
            user_id: user_id.into().trim().to_string(),
            revision: default_emby_setup_revision(),
        }
    }
}

/// Audiobookshelf-specific setup persisted in config.toml. The API key is a
/// Service secret and is deliberately not part of this record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfSetup {
    pub server_url: String,
    #[serde(default = "default_audiobookshelf_setup_revision")]
    pub revision: u64,
}

const fn default_audiobookshelf_setup_revision() -> u64 {
    1
}

impl Default for AudiobookshelfSetup {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            revision: default_audiobookshelf_setup_revision(),
        }
    }
}

impl AudiobookshelfSetup {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim().trim_end_matches('/').to_string(),
            revision: default_audiobookshelf_setup_revision(),
        }
    }
}

/// Singleton Service kind identifier. Each variant represents exactly one
/// configured instance of that Service within mbv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ServiceKind {
    Emby,
    /// Reserved; not yet implemented. Added now to establish the pattern
    /// and the per-Service secret path. No Audiobookshelf auth, browsing,
    /// playback, or credential logic is introduced here.
    Audiobookshelf,
}

impl ServiceKind {
    /// Filesystem-safe name for this Service kind, used for per-Service
    /// secret file names.
    pub fn secret_name(self) -> &'static str {
        match self {
            ServiceKind::Emby => "emby",
            ServiceKind::Audiobookshelf => "audiobookshelf",
        }
    }
}
