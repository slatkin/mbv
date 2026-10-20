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
    /// The selection whose items the view paints; the key into `items` and
    /// the single source of truth for the highlighted pill. The pill index
    /// derives on demand from the selection's folder position in the counted
    /// folders (0 = All); it is materialized only at the `LibraryPosition`
    /// save/restore boundary.
    pub selection: FeedHomeVideoSelection,
    /// Loaded items per selection: the session cache so pill ping-pong does
    /// not refetch (design D2). The lazy fetch fills one entry per selection.
    pub items: HashMap<FeedHomeVideoSelection, Vec<EmbyItem>>,
    /// Whether the current selection's fetch is still outstanding.
    pub loading: bool,
    /// The selection whose lazy fetch is in flight, `None` when no fetch is
    /// outstanding. Distinguishes a fetch-phase `loading` (an outstanding
    /// spawn, which listing completion must not clear — spec
    /// `feed-home-video-group-view`, "An unloaded selection SHALL show its
    /// loading state until its fetch returns") from the seed/listing-pending
    /// `loading` the listing completion hands off. Set at the fetch spawn,
    /// cleared when that selection's result event lands — applied or
    /// stale-dropped. A fetch that fails reports through `LibEvent::Error`
    /// and leaves the mark until the next result for the same selection.
    pub fetching: Option<FeedHomeVideoSelection>,
    /// Per-root-folder video counts. The count fan-out fills this; pill
    /// membership reads it alongside the root listing. A full refresh clears
    /// it so the next fan-out re-queries every folder. Empty until then.
    pub folder_counts: HashMap<String, usize>,
    /// How many fan-out count queries are still owed a result event. The
    /// spawn records one per folder it queries; every per-folder event —
    /// success or failure — settles one, so zero means the fan-out has
    /// quiesced and the counted row is final. Session memory; never
    /// persisted.
    pub outstanding_counts: usize,
    /// A pill index restored from `LibraryPosition::feed_selected_group` that
    /// still awaits resolution into `selection` against the counted folders
    /// (the pending-selection resolve path). Restore-only session memory:
    /// `LibraryTab::apply_library_position` sets it, and it clears once it
    /// resolves or the user picks a pill. It never drives the painted
    /// highlight, which derives from `selection`.
    pub restored_group: Option<usize>,
    pub video_cursor: usize,
    pub video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Length of the currently selected item list, without cloning it.
    #[must_use]
    pub fn selected_len(&self) -> usize {
        self.items.get(&self.selection).map_or(0, Vec::len)
    }

    /// The pill index to persist at the `LibraryPosition` save boundary: a
    /// still-unresolved restored index saves unchanged (the counted folders
    /// that would resolve it are not loaded yet), otherwise the selection
    /// decides — 0 for All, the folder's counted position plus one otherwise.
    /// A selection whose folder holds no pill (its count vanished) saves as
    /// All: the index it was picked from no longer names a pill.
    #[must_use]
    pub(crate) fn saved_group_index(&self, counted_ids: &[String]) -> usize {
        if let Some(group) = self.restored_group {
            return group;
        }
        match &self.selection {
            FeedHomeVideoSelection::All => 0,
            FeedHomeVideoSelection::Folder(id) => counted_ids
                .iter()
                .position(|counted| counted == id)
                .map_or(0, |pos| pos + 1),
        }
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
