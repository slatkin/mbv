use mbv_emby_model::EmbyItem;
use mbv_feed::IdleFeedItem;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Instant;

/// Which list the feed home-video group view shows: every video under the
/// library root, or one root folder's videos.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FeedHomeVideoSelection {
    #[default]
    All,
    Folder(String),
}

#[derive(Clone, Debug, Default)]
pub struct FeedHomeVideoState {
    /// The selection whose items the view paints; the key into `items`.
    pub selection: FeedHomeVideoSelection,
    /// Loaded items per selection: the session cache so pill ping-pong does
    /// not refetch (design D2). The lazy fetch fills one entry per selection.
    pub items: HashMap<FeedHomeVideoSelection, Vec<EmbyItem>>,
    /// Whether the current selection's fetch is still outstanding.
    pub loading: bool,
    /// Per-root-folder video counts. The count fan-out fills this; pill
    /// membership reads it alongside the root listing. Empty until then.
    pub folder_counts: HashMap<String, usize>,
    /// Pill-facing selected-group index: 0 means All, 1-based otherwise.
    /// Persisted through `LibraryPosition::feed_selected_group` and resolved
    /// into `selection` against the root listing once the listing has loaded.
    pub selected_group: usize,
    /// Whether a refresh deferred its forced count re-query to the root
    /// listing's next completion: a partial refresh must not replace the
    /// counts from its page-1 folders, but it still owes the session a forced
    /// re-query of every folder once the listing fully repaginates. The
    /// listing-completion path passes it to the fan-out, which consumes it
    /// exactly when the forced fan-out is requested. Session-local; never
    /// persisted.
    pub pending_count_refresh: bool,
    pub video_cursor: usize,
    pub video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Length of the currently selected item list, without cloning it.
    #[must_use]
    pub fn selected_len(&self) -> usize {
        self.items.get(&self.selection).map_or(0, Vec::len)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SavePlaylistStage {
    EnterName,
    RenamePlaylist { id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavePlaylistDialog {
    pub input: String,
    pub stage: SavePlaylistStage,
}

#[derive(Debug)]
pub struct IdleFeed {
    pub items: Vec<IdleFeedItem>,
    pub current_index: usize,
    pub last_rotation: Instant,
    pub last_fetch: Instant,
    pub items_tx: mpsc::Sender<Vec<IdleFeedItem>>,
    pub items_rx: mpsc::Receiver<Vec<IdleFeedItem>>,
}
