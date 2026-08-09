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
    pub fn as_str(self) -> &'static str {
        match self {
            FeedKind::Audio => "audio",
            FeedKind::Video => "video",
        }
    }

    /// Parse a `kind` value from config; unknown values yield `None` so
    /// callers fall back to `FeedKind::default()`.
    pub fn parse(s: &str) -> Option<FeedKind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "audio" => Some(FeedKind::Audio),
            "video" => Some(FeedKind::Video),
            _ => None,
        }
    }
}

/// One user-configured feed subscription (`[[feeds]]` in config.toml).
/// The subscription list is the only persisted feed data: per-entry
/// playback state is never saved (#471 MVP).
#[derive(Debug, Clone)]
pub struct FeedSubscription {
    /// Display name shown in the Feeds tab and the management overlay.
    pub name: String,
    /// Feed URL (RSS 2.0 or Atom).
    pub url: String,
    /// Default media kind for this subscription's entries.
    pub kind: FeedKind,
}
