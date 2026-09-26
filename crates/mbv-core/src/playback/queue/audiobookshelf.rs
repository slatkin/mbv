use super::items::{duration_ticks_as_i64, PlaybackTitleParts, QueueItemContentId, QueueItemKind};

// ---------------------------------------------------------------------------
// AudiobookshelfQueueItem — identity, presentation, duration, progress,
// completion, and Service-scoped artwork identity. Excludes credentials,
// server URL, playback sessionId, resolved source URL, and headers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfQueueItem {
    #[serde(rename = "libraryItemId")]
    pub library_item_id: String,
    #[serde(rename = "episodeId")]
    pub episode_id: String,
    pub title: String,
    #[serde(default)]
    pub show_title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Episode synopsis/description, when the catalog carries one. Used by
    /// Home's hero detail for Audiobookshelf rows.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    /// Playback position in ticks. Zero means start from the beginning.
    #[serde(default)]
    pub position_ticks: i64,
    #[serde(default)]
    pub played: bool,
    /// Publish time in Unix seconds, when available from the catalog.
    #[serde(default)]
    pub pub_date_secs: Option<u64>,
    /// Mirrors Audiobookshelf `is_finished` (distinct from played state
    /// persisted here). Defaulted for backward compat.
    #[serde(default)]
    pub is_finished: bool,
    /// Service-scoped artwork identity (cover path, not server URL).
    #[serde(default)]
    pub cover_path: Option<String>,
}

impl AudiobookshelfQueueItem {
    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        QueueItemContentId::Audiobookshelf {
            library_item_id: self.library_item_id.clone(),
            episode_id: self.episode_id.clone(),
        }
    }

    #[must_use]
    pub fn resume_seconds(&self) -> f64 {
        if crate::api::should_resume(
            self.position_ticks,
            duration_ticks_as_i64(self.duration_ticks.unwrap_or(0)),
        ) {
            crate::api::ticks_to_seconds(self.position_ticks)
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// AudiobookshelfBookQueueItem — the book-shaped queue snapshot: content
// identity, presentation, progress, and completion keyed by `library_item_id`
// only. No episode identity. Excludes credentials, server URL, playback
// sessionId, resolved source URLs, and headers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfBookQueueItem {
    #[serde(rename = "libraryItemId")]
    pub library_item_id: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    /// Playback position in ticks. Zero means start from the beginning.
    #[serde(default)]
    pub position_ticks: i64,
    #[serde(default)]
    pub played: bool,
    /// Mirrors Audiobookshelf `is_finished` (distinct from played state).
    #[serde(default)]
    pub is_finished: bool,
    /// Service-scoped artwork identity (cover path, not server URL).
    #[serde(default)]
    pub cover_path: Option<String>,
}

impl AudiobookshelfBookQueueItem {
    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        QueueItemContentId::AudiobookshelfBook {
            library_item_id: self.library_item_id.clone(),
        }
    }

    #[must_use]
    pub fn resume_seconds(&self) -> f64 {
        if crate::api::should_resume(
            self.position_ticks,
            duration_ticks_as_i64(self.duration_ticks.unwrap_or(0)),
        ) {
            crate::api::ticks_to_seconds(self.position_ticks)
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// AudiobookshelfItem — the nested episode/book variants and their shared
// queue presentation and identity behavior.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AudiobookshelfItem {
    Episode(AudiobookshelfQueueItem),
    Book(AudiobookshelfBookQueueItem),
}

impl AudiobookshelfItem {
    #[must_use]
    pub fn resume_seconds(&self) -> f64 {
        match self {
            Self::Episode(item) => item.resume_seconds(),
            Self::Book(item) => item.resume_seconds(),
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Self::Episode(item) => &item.title,
            Self::Book(item) => &item.title,
        }
    }

    #[must_use]
    pub fn duration(&self) -> Option<u64> {
        match self {
            Self::Episode(item) => item.duration_ticks,
            Self::Book(item) => item.duration_ticks,
        }
    }

    #[must_use]
    pub fn media_kind(&self) -> &'static str {
        "Audio"
    }

    #[must_use]
    pub fn is_audio(&self) -> bool {
        true
    }

    #[must_use]
    pub fn is_music(&self) -> bool {
        false
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        false
    }

    #[must_use]
    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            Self::Episode(item) => item.cover_path.as_deref(),
            Self::Book(item) => item.cover_path.as_deref(),
        }
    }

    #[must_use]
    pub fn display_name_parts(&self) -> (String, Option<String>) {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || (item.title.clone(), None),
                    |show| (show.to_owned(), Some(item.title.clone())),
                ),
            Self::Book(item) => (item.title.clone(), None),
        }
    }

    #[must_use]
    pub fn playback_title_parts(&self) -> PlaybackTitleParts {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || PlaybackTitleParts::single(item.title.clone()),
                    |show| PlaybackTitleParts::two(item.title.clone(), show.to_owned()),
                ),
            Self::Book(item) => PlaybackTitleParts::single(item.title.clone()),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || item.title.clone(),
                    |show| format!("{show} - {}", item.title),
                ),
            Self::Book(item) => item.title.clone(),
        }
    }

    #[must_use]
    pub fn overview(&self) -> Option<&str> {
        match self {
            Self::Episode(item) => item.description.as_deref(),
            Self::Book(_) => None,
        }
    }

    #[must_use]
    pub fn runtime_ticks(&self) -> i64 {
        duration_ticks_as_i64(self.duration().unwrap_or(0))
    }

    #[must_use]
    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            Self::Episode(item) => item.position_ticks,
            Self::Book(item) => item.position_ticks,
        }
    }

    #[must_use]
    pub fn played(&self) -> bool {
        match self {
            Self::Episode(item) => item.played || item.is_finished,
            Self::Book(item) => item.played || item.is_finished,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Episode(item) => &item.episode_id,
            Self::Book(item) => &item.library_item_id,
        }
    }

    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        match self {
            Self::Episode(item) => item.content_id(),
            Self::Book(item) => item.content_id(),
        }
    }

    #[must_use]
    pub fn playlist_item_id(&self) -> &'static str {
        ""
    }

    #[must_use]
    pub fn kind(&self) -> QueueItemKind {
        match self {
            Self::Episode(_) => QueueItemKind::Audiobookshelf,
            Self::Book(_) => QueueItemKind::AudiobookshelfBook,
        }
    }
}
