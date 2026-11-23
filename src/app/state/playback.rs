use crate::app::state::queue_owner::QueueOrigin;
use mbv_ctrl::player::PlayerEvent;
use mbv_emby_model::EmbyItem;
use mbv_player::PlayerProxy;
use mbv_queue::{NamedQueueId, QueueItem, QueueSlotId};
use mbv_ui_model::playback::QueueLink;
use std::collections::VecDeque;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueLinkResolution {
    pub has_direct_remote_queue: bool,
    /// The GUI Client's shown entry, and `None` for the TUI Client (design D7
    /// of `gui-named-queues`). The TUI Client shows the current queue by its
    /// connection rule (ADR 0034); the GUI Client shows the entry the Queue
    /// chooser picked (ADR 0041).
    pub shown_entry: Option<ShownEntry>,
}

/// The entry the GUI Client's Queue panel shows (gui-named-queues design D7):
/// one of the local Owner's Named queues, or the connected peer Player owner.
///
/// A Client names the entry through `App::shown_entry`, so the type is `pub`
/// (change `split-tui-gui-clients`, design D4b rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownEntry {
    /// The local Owner's Named queue with this identity: the current queue, or
    /// one that rests.
    Local(NamedQueueId),
    /// The queue of the peer Player owner this Client is connected to.
    Peer,
}

impl QueueLinkResolution {
    #[must_use]
    pub fn new(has_direct_remote_queue: bool, shown_entry: Option<ShownEntry>) -> Self {
        Self {
            has_direct_remote_queue,
            shown_entry,
        }
    }

    /// The link both the shown queue and the playback target resolve to.
    ///
    /// The GUI Client follows its shown entry, whichever link the connection
    /// would pick, so a local Named queue and the player panel still resolve to
    /// one link (ADR 0034 stays true; ADR 0041 only changes the choice). A
    /// peer entry exists only while a peer is connected — a disconnect removes
    /// it — so an entry left behind by a dropped or absent peer resolves to the
    /// local Owner's queue rather than to a link that holds no queue.
    #[must_use]
    pub fn playback_target(self) -> QueueLink {
        if !self.has_direct_remote_queue {
            return QueueLink::Local;
        }
        match self.shown_entry {
            Some(ShownEntry::Local(_)) => QueueLink::Local,
            Some(ShownEntry::Peer) | None => QueueLink::Peer,
        }
    }

    #[must_use]
    pub fn local_metadata_applies(self, link: QueueLink) -> bool {
        link == QueueLink::Local || !self.has_direct_remote_queue
    }
}

/// The queue one edit, view lookup, or undone edit names (design D7 of
/// `gui-named-queues`): the Player-owner link it reaches and, on the local
/// link, the Named queue it names. `None` names the link's current queue — on
/// the wire and as the Client's `local_view` — and a GUI edit of the shown
/// queue names the shown queue, whichever entry the panel shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueTarget {
    /// The Player-owner link the edit reaches: the suspended home link for
    /// `Local` while a peer holds the active one, else the active link.
    pub(in crate::app) link: QueueLink,
    /// The Named queue the edit names, or `None` for the link's current queue.
    pub(in crate::app) queue: Option<NamedQueueId>,
}

impl QueueTarget {
    /// The target that names `link`'s current queue: the default for every
    /// path that does not edit the GUI Client's shown queue (design D7).
    #[must_use]
    pub(in crate::app) fn current(link: QueueLink) -> Self {
        Self { link, queue: None }
    }
}

/// A reversible queue edit (design D7). Undo re-sends the inverse edit to
/// the queue the edit reached: `Remove` re-appends the removed item before the
/// entry now at its former index (resolved at undo time), and `Move` moves the
/// slot back to `from`. `slot_id` is the runtime queue occurrence that was
/// moved; the owner rejects the undo as stale when that slot is gone.
#[derive(Debug)]
pub enum UndoEntry {
    Remove {
        /// The queue the edit reached: the GUI Client's shown queue, or the
        /// current queue. The inverse edit and the view lookups name it,
        /// whichever entry the panel shows when the user undoes (design D7).
        target: QueueTarget,
        item: Box<QueueItem>,
        index: usize,
    },
    Move {
        /// The queue the edit reached, as `Remove` carries it.
        target: QueueTarget,
        slot_id: QueueSlotId,
        from: usize,
    },
}

impl UndoEntry {
    /// The queue the edit reached: the inverse edit, the undo-time view
    /// lookups, and the undo bookkeeping all name it (design D7).
    #[must_use]
    pub(in crate::app) fn target(&self) -> QueueTarget {
        match self {
            Self::Remove { target, .. } | Self::Move { target, .. } => *target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteSlotState {
    DirectRemote,
    LocalDaemon,
}

/// Identity of a destination Latest surface for its independent marker state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DestinationLatestSource {
    Emby(String),
    Audiobookshelf(String),
    Feeds,
}

/// The shell-owned snapshot of one Emby destination's Latest items. It feeds
/// only the destination Latest projection; the new-content marker is
/// evaluated from Home's `Recently added` rows (spec
/// `destination-latest-modes`, "Latest markers follow their destination
/// pills"), never from this snapshot.
#[derive(Clone, Debug)]
pub(in crate::app) struct DestinationLatestSnapshot {
    pub items: Vec<QueueItem>,
}

impl DestinationLatestSnapshot {
    #[must_use]
    pub fn new(items: Vec<QueueItem>) -> Self {
        Self { items }
    }
}

/// Home content (task 5.3d): the authoritative Continue Watching snapshot
/// the presenting Client's shell pushes to its Home surface at its writers.
/// `loading` mirrors the deleted `home_loading` flag (true from startup
/// until the first fetch completes, then set false synchronously after every
/// content computation). The `App` stores the snapshot (`App.home_content`):
/// the launch Home load and a rebuilt startup-drain snapshot are the launch
/// sources, and the Client core hands a rebuilt snapshot back to its Client
/// ([`crate::app::StartupWorkerDrain`]) for the shell's own projection half.
#[derive(Clone, Debug)]
pub struct HomeContent {
    pub continue_items: Vec<EmbyItem>,
    pub loading: bool,
}

impl Default for HomeContent {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeContent {
    /// Default Home state at shell construction: no items and `loading`
    /// true — the startup skeleton, mirroring the deleted
    /// `App.home_loading`/`construct` state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            loading: true,
        }
    }
}

#[derive(Debug)]
pub(in crate::app) struct SuspendedLocalSession {
    pub player: PlayerProxy,
    pub player_rx: mpsc::Receiver<PlayerEvent>,
}

#[derive(Debug)]
pub enum PendingQueueAction {
    PlayItems {
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    },
    ClearQueue,
}

/// Which queue-replacement executor runs once the populated-queue gate is
/// confirmed. `Pending` is the plain `execute_queue_replacement` path.
/// `PlaylistsSidebar` runs it, then closes the Playlists sidebar and focuses
/// the Queue so the replaced queue is visible. `Routed` carries its entry
/// point's pre-play prep, because `play_items_routed` never replaces the
/// playback queue itself — every routed caller does its own mutation around
/// it, and they differ.
#[derive(Debug)]
pub(in crate::app) enum ReplacementExecutor {
    Routed(RoutedReplacementPrep),
    Pending,
    PlaylistsSidebar,
}

/// The per-entry-point pre-play prep a `Routed` replacement replays before
/// handing the resolved items to `play_items_routed`. Each variant mirrors one
/// gated routed entry point exactly: when the playback queue is rebuilt, when
/// queue state is persisted, and whether the Queue panel takes focus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum RoutedReplacementPrep {
    /// Album/artist track (`replace_and_route_album_queue`): Album source,
    /// rebuild always, save unless a direct-remote owner holds the queue,
    /// routed default focus.
    Album,
    /// `play_music_albums`: rebuild and save always, keep the Library focused.
    MusicAlbums,
    /// `play_folder`: rebuild always, force Queue focus, save always (the
    /// folder-play callers used to save after `play_folder` returned).
    Folder,
    /// `shuffle_folder`: rebuild always, force Queue focus, save unless a
    /// direct-remote owner holds the queue.
    ShuffleFolder,
    /// Context-menu Play/Shuffle selection: rebuild and save only when this
    /// process owns the local queue (no direct remote),
    /// routed default focus.
    Selection,
}

#[derive(Clone, Debug)]
pub enum PlaylistMutation {
    Save {
        mutation_id: u64,
        origin: QueueOrigin,
        source_playlist_id: String,
        item_ids: Option<Vec<String>>,
    },
    CreateAs {
        mutation_id: u64,
        coordinator_key: String,
        name: String,
        origin: QueueOrigin,
        source_playlist_id: Option<String>,
        item_ids: Option<Vec<String>>,
    },
    Replace {
        mutation_id: u64,
        origin: QueueOrigin,
        name: String,
        item_ids: Option<Vec<String>>,
    },
}

impl PlaylistMutation {
    #[must_use]
    pub fn mutation_id(&self) -> u64 {
        match self {
            Self::Save { mutation_id, .. }
            | Self::CreateAs { mutation_id, .. }
            | Self::Replace { mutation_id, .. } => *mutation_id,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct PlaylistMutationState {
    pub active: Option<PlaylistMutation>,
    pub queued: VecDeque<PlaylistMutation>,
}
