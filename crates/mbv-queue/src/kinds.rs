/// Media kind of a feed subscription, declared by the user in
/// `[[feeds]]` config entries. The stored kind is the default for the
/// subscription; per-entry kind can be refined from the enclosure MIME
/// at parse time (#471). Not currently used for routing — only for
/// display and future refinement (#472).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedKind {
    Audio,
    Video,
}

impl Default for FeedKind {
    /// Absent/unparseable `kind` values default to Video (RSS video
    /// feeds predominate; MIME inference refines per entry).
    fn default() -> Self {
        FeedKind::Video
    }
}

impl FeedKind {
    /// The `kind` key as written to `config.toml` (`[[feeds]]` rows).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            FeedKind::Audio => "audio",
            FeedKind::Video => "video",
        }
    }

    /// Parse a `kind` value from config; unknown values yield `None` so
    /// callers fall back to `FeedKind::default()`.
    #[must_use]
    pub fn parse(s: &str) -> Option<FeedKind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "audio" => Some(FeedKind::Audio),
            "video" => Some(FeedKind::Video),
            _ => None,
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
    #[must_use]
    pub fn secret_name(self) -> &'static str {
        match self {
            ServiceKind::Emby => "emby",
            ServiceKind::Audiobookshelf => "audiobookshelf",
        }
    }
}
