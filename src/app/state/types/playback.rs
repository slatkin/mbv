use crate::app::state::home_latest::{is_new_in_launch_window, HomeLatestLaunchWindow};
use crate::app::state::queue_owner::QueueOrigin;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{QueueItem, QueueSlotId};
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_ws::WsEvent;
use std::collections::VecDeque;
use std::sync::mpsc;

/// Shared local-vs-remote playback seam for the TUI action layer.
#[derive(Clone, Copy)]
pub(in crate::app) struct LocalPlaybackTarget;

#[derive(Clone)]
pub(in crate::app) struct RemotePlaybackTarget {
    pub(in crate::app) session_id: String,
}

/// Reads/writes `app.cast_attachment` directly, the same way
/// `LocalPlaybackTarget` reads `app.player` -- see `playback_target_cast.rs`.
#[derive(Clone, Copy)]
pub(in crate::app) struct CastPlaybackTarget;

#[derive(Clone)]
pub(in crate::app) enum PlaybackTarget {
    Local(LocalPlaybackTarget),
    Remote(RemotePlaybackTarget),
    Cast(CastPlaybackTarget),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct PlaybackState {
    /// Whether a transport is active, on any target. A watched remote
    /// Session playing foreign content is active without a local slot.
    pub(in crate::app) active: bool,
    /// The playhead's slot in the queue this state was read from. `None`
    /// while a watched remote Session plays something the local queue does
    /// not hold: the transport is active, but no row backs it.
    pub(in crate::app) active_idx: Option<usize>,
    pub(in crate::app) position_ticks: i64,
    pub(in crate::app) runtime_ticks: i64,
    pub(in crate::app) paused: bool,
}

/// Which queue an operation refers to.
///
/// `Local` is this TUI instance's own queue and carries local-only metadata:
/// dirty state, undo history, saved-playlist source, and on-disk persistence.
/// `Remote` is the queue owned by a directly-controlled mbv daemon or remote
/// instance. A stale `Remote` UI preference is ignored unless a direct remote
/// queue is actually present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueueScope {
    Local,
    Remote,
}

/// Derived answers for the local/remote queue boundary.
///
/// The three answers intentionally differ:
/// - playback commands target `Remote` whenever a direct remote queue exists;
/// - the visible queue is `Remote` only when a direct remote queue exists and
///   the user has selected the remote scope;
/// - local queue metadata applies only to local scope while a direct remote
///   queue exists, but applies to any effective scope when no direct remote
///   queue exists because all queue state is local then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueScopeResolution {
    pub(in crate::app) has_direct_remote_queue: bool,
    pub(in crate::app) requested_visible_scope: QueueScope,
}

impl QueueScopeResolution {
    pub(in crate::app) fn new(
        has_direct_remote_queue: bool,
        requested_visible_scope: QueueScope,
    ) -> Self {
        Self {
            has_direct_remote_queue,
            requested_visible_scope,
        }
    }

    pub(in crate::app) fn playback_target(self) -> QueueScope {
        if self.has_direct_remote_queue {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    pub(in crate::app) fn visible_scope(self) -> QueueScope {
        if self.has_direct_remote_queue && self.requested_visible_scope == QueueScope::Remote {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    pub(in crate::app) fn local_metadata_applies(self, scope: QueueScope) -> bool {
        scope == QueueScope::Local || !self.has_direct_remote_queue
    }
}

/// A reversible queue edit. `Remove` re-inserts the item at its old position;
/// `Move` swaps the slot back from `to` to `from`. `slot_id` is the runtime
/// queue occurrence that landed at `to`, checked at undo time so a queue edit
/// made after the move is refused instead of swapping the wrong items.
#[derive(Debug)]
pub(in crate::app) enum UndoEntry {
    Remove(usize, QueueItem),
    Move {
        from: usize,
        to: usize,
        slot_id: QueueSlotId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum RemoteSlotState {
    Off,
    AttachedSession,
    DirectRemote,
    LocalDaemon,
}

/// Identity of a destination Latest surface for its independent marker state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum DestinationLatestSource {
    Emby(String),
    Audiobookshelf(String),
    Feeds,
}

/// The shell-owned snapshot of one destination's Latest items. Its new-content
/// marker is evaluated against the frozen launch window, never by the component.
#[derive(Clone, Debug)]
pub(in crate::app) struct DestinationLatestSnapshot {
    pub(in crate::app) title: String,
    pub(in crate::app) source: DestinationLatestSource,
    pub(in crate::app) items: Vec<QueueItem>,
    pub(in crate::app) has_new_content: bool,
}

impl DestinationLatestSnapshot {
    pub(in crate::app) fn new_with_launch_window(
        title: String,
        source: DestinationLatestSource,
        items: Vec<QueueItem>,
        window: HomeLatestLaunchWindow,
    ) -> Self {
        let has_new_content = items
            .iter()
            .any(|item| is_new_in_launch_window(item, window));
        Self {
            title,
            source,
            items,
            has_new_content,
        }
    }

    pub(in crate::app) fn recompute_new_content(&mut self, window: HomeLatestLaunchWindow) {
        self.has_new_content = self
            .items
            .iter()
            .any(|item| is_new_in_launch_window(item, window));
    }
}

/// Model-owned Home content (task 5.3d): the authoritative snapshot the
/// shell pushes to `HomeComponent` at its writers. Re-homed from the deleted
/// `App.home` (`HomePane`) + `App.home_loading`; `loading` mirrors the old
/// `home_loading` flag (true from startup until the first fetch completes,
/// then set false synchronously after every content computation). The
pub(in crate::app) struct HomeContent {
    pub(in crate::app) continue_items: Vec<EmbyItem>,
    pub(in crate::app) loading: bool,
}

impl HomeContent {
    /// Default Home state at shell construction: no items and `loading`
    /// true — the startup skeleton, mirroring the deleted
    /// `App.home_loading`/`construct` state.
    pub(in crate::app) fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            loading: true,
        }
    }
}

pub(in crate::app) struct SuspendedLocalSession {
    pub(in crate::app) player: PlayerProxy,
    pub(in crate::app) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(in crate::app) ws_rx: mpsc::Receiver<WsEvent>,
    pub(in crate::app) ws_send_tx: Option<mbv_ws::WsSender>,
    pub(in crate::app) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf::socket::SocketEvent>,
    pub(in crate::app) audiobookshelf_socket_tx: Option<mpsc::Sender<()>>,
    pub(in crate::app) audiobookshelf_socket_generation:
        Option<mbv_core::service_runtime::SetupGeneration>,
}

pub(in crate::app) enum PendingQueueAction {
    PlayItems {
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        /// False replaces the queue without starting playback (playlist
        /// Enter populates the queue; Space/Enter on the queue starts it).
        autostart: bool,
    },
    ClearQueue,
}

/// Which of the two existing queue-replacement executors runs once the
/// populated-queue gate is confirmed. `Pending` is the existing
/// `execute_queue_replacement` path; `Routed` carries its entry point's
/// pre-play prep, because `play_items_routed` never replaces the playback
/// queue itself — every routed caller does its own mutation around it, and
/// they differ.
pub(in crate::app) enum ReplacementExecutor {
    Routed(RoutedReplacementPrep),
    Pending,
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
    /// process owns the local queue (no direct remote, no attached session),
    /// routed default focus.
    Selection,
}

#[derive(Clone, Debug)]
pub(in crate::app) enum PlaylistMutation {
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
    pub(in crate::app) fn mutation_id(&self) -> u64 {
        match self {
            Self::Save { mutation_id, .. }
            | Self::CreateAs { mutation_id, .. }
            | Self::Replace { mutation_id, .. } => *mutation_id,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct PlaylistMutationState {
    pub(in crate::app) active: Option<PlaylistMutation>,
    pub(in crate::app) queued: VecDeque<PlaylistMutation>,
}
