// Queue/library position state types. Included into `config`'s module scope
// (see `config.rs`), so callers reach them as `crate::config::…`.

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueueSource {
    Playlist {
        id: Option<String>,
        name: String,
    },
    Album,
    Series,
    Shuffle,
    Remote,
    Collection {
        collection_type: String,
    },
    #[default]
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct QueueState {
    #[serde(default)]
    pub source: QueueSource,
    // Full items, not just IDs: restoring the queue must be instant and local
    // (no network round-trip), so everything needed to display and play it
    // has to already be on disk. A separate best-effort background fetch
    // refreshes played/position state from the server afterward.
    #[serde(default)]
    pub items: Vec<crate::playback_queue::QueueItem>,
    #[serde(default)]
    pub cursor: usize,
    /// Provider-qualified anchor. The raw ID below remains readable for old
    /// snapshots, but is only used when it identifies one provider uniquely.
    #[serde(default)]
    pub last_played_content_id: Option<crate::playback_queue::QueueItemContentId>,
    pub last_played_item_id: Option<String>,
    pub last_played_completed: bool,
    // Per-item resume positions saved at quit time. Used on restore to override stale Emby
    // UserData when a fresh launch races with Emby's async position write.
    #[serde(default)]
    pub positions: std::collections::HashMap<String, i64>,
}

impl QueueState {
    /// Returns only the Emby items from the queue state.
    /// Used by the UI layer (PlayerTab) and ctrl boundary, which only
    /// operate on Emby items. Skips both Feed and Audiobookshelf items.
    pub fn emby_items(&self) -> Vec<crate::api::EmbyItem> {
        self.items
            .iter()
            .filter_map(|qi| match qi {
                crate::playback_queue::QueueItem::Emby(e) => Some((**e).clone()),
                crate::playback_queue::QueueItem::Feed(_) => None,
                crate::playback_queue::QueueItem::Audiobookshelf(_) => None,
                crate::playback_queue::QueueItem::AudiobookshelfBook(_) => None,
            })
            .collect()
    }

    /// Consumes self and returns the inner `Vec<QueueItem>`.
    pub fn into_queue_items(self) -> Vec<crate::playback_queue::QueueItem> {
        self.items
    }

    /// Creates a `QueueState` from `Vec<EmbyItem>`, wrapping each as
    /// `QueueItem::Emby`. Convenience for tests and callers that only
    /// deal with Emby items.
    pub fn from_emby_items(
        items: Vec<crate::api::EmbyItem>,
        cursor: usize,
        source: QueueSource,
    ) -> Self {
        Self {
            items: items
                .into_iter()
                .map(|item| crate::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
            source,
            last_played_item_id: None,
            last_played_content_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }
    }
}

/// One library position per library (#361 collapsed the old two-scope
/// `{default, power}` split -- there is only one view now, so there is only
/// one saved position per library).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPositionState {
    #[serde(default)]
    pub libraries: std::collections::HashMap<String, LibraryPosition>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPosition {
    #[serde(default)]
    pub levels: Vec<LibraryPositionLevel>,
    #[serde(default)]
    pub feed_selected_group: usize,
    #[serde(default)]
    pub feed_video_cursor: usize,
    #[serde(default)]
    pub feed_video_scroll: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPositionLevel {
    pub parent_id: String,
    pub title: String,
    #[serde(default)]
    pub focused_item_id: Option<String>,
    #[serde(default)]
    pub cursor_index: usize,
    #[serde(default)]
    pub item_types: Option<String>,
    #[serde(default)]
    pub unplayed_only: bool,
    #[serde(default)]
    pub sort_by: String,
    #[serde(default)]
    pub sort_order: String,
    /// Index of the active letter-range pill (see `app::render::power::LetterFilter`),
    /// for the top level of a large library. `None` = unfiltered / not applicable.
    #[serde(default)]
    pub letter_filter_index: Option<usize>,
    /// The library's unfiltered item count for restoring the letter pill row.
    #[serde(default)]
    pub library_total: Option<usize>,
}
