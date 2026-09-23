use super::home_latest::{is_new_in_launch_window, HomeLatestLaunchWindow};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{QueueItem, QueueSlotId};
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;
use std::collections::VecDeque;
use std::sync::mpsc;

/// Shared local-vs-remote playback seam for the TUI action layer.
#[derive(Clone, Copy)]
pub(super) struct LocalPlaybackTarget;

#[derive(Clone)]
pub(super) struct RemotePlaybackTarget {
    pub(super) session_id: String,
}

/// Reads/writes `app.cast_attachment` directly, the same way
/// `LocalPlaybackTarget` reads `app.player` -- see `playback_target_cast.rs`.
#[derive(Clone, Copy)]
pub(super) struct CastPlaybackTarget;

#[derive(Clone)]
pub(super) enum PlaybackTarget {
    Local(LocalPlaybackTarget),
    Remote(RemotePlaybackTarget),
    Cast(CastPlaybackTarget),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PlaybackState {
    /// Whether a transport is active, on any target. A watched remote
    /// Session playing foreign content is active without a local slot.
    pub(super) active: bool,
    /// The playhead's slot in the queue this state was read from. `None`
    /// while a watched remote Session plays something the local queue does
    /// not hold: the transport is active, but no row backs it.
    pub(super) active_idx: Option<usize>,
    pub(super) position_ticks: i64,
    pub(super) runtime_ticks: i64,
    pub(super) paused: bool,
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
pub(super) struct QueueScopeResolution {
    pub(super) has_direct_remote_queue: bool,
    pub(super) requested_visible_scope: QueueScope,
}

impl QueueScopeResolution {
    pub(super) fn new(has_direct_remote_queue: bool, requested_visible_scope: QueueScope) -> Self {
        Self {
            has_direct_remote_queue,
            requested_visible_scope,
        }
    }

    pub(super) fn playback_target(self) -> QueueScope {
        if self.has_direct_remote_queue {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    pub(super) fn visible_scope(self) -> QueueScope {
        if self.has_direct_remote_queue && self.requested_visible_scope == QueueScope::Remote {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    pub(super) fn local_metadata_applies(self, scope: QueueScope) -> bool {
        scope == QueueScope::Local || !self.has_direct_remote_queue
    }
}

/// A reversible queue edit. `Remove` re-inserts the item at its old position;
/// `Move` swaps the slot back from `to` to `from`. `slot_id` is the runtime
/// queue occurrence that landed at `to`, checked at undo time so a queue edit
/// made after the move is refused instead of swapping the wrong items.
#[derive(Debug)]
pub(super) enum UndoEntry {
    Remove(usize, QueueItem),
    Move {
        from: usize,
        to: usize,
        slot_id: QueueSlotId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RemoteSlotState {
    Off,
    AttachedSession,
    DirectRemote,
    LocalDaemon,
}

/// Identity of a destination Latest surface for its independent marker state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum DestinationLatestSource {
    Emby(String),
    Audiobookshelf(String),
    Feeds,
}

/// The shell-owned snapshot of one destination's Latest items. Its new-content
/// marker is evaluated against the frozen launch window, never by the component.
#[derive(Clone, Debug)]
pub(super) struct DestinationLatestSnapshot {
    pub(super) title: String,
    pub(super) source: DestinationLatestSource,
    pub(super) items: Vec<QueueItem>,
    pub(super) has_new_content: bool,
}

impl DestinationLatestSnapshot {
    #[cfg(test)]
    pub(super) fn new(
        title: String,
        source: DestinationLatestSource,
        items: Vec<QueueItem>,
    ) -> Self {
        Self::new_with_launch_window(
            title,
            source,
            items,
            HomeLatestLaunchWindow {
                previous: None,
                current: 0,
            },
        )
    }

    pub(super) fn new_with_launch_window(
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

    pub(super) fn recompute_new_content(&mut self, window: HomeLatestLaunchWindow) {
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
pub(super) struct HomeContent {
    pub(super) continue_items: Vec<EmbyItem>,
    pub(super) loading: bool,
}

impl HomeContent {
    /// Default Home state at shell construction: no items and `loading`
    /// true — the startup skeleton, mirroring the deleted
    /// `App.home_loading`/`construct` state.
    pub(super) fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            loading: true,
        }
    }
}

pub(super) struct SuspendedLocalSession {
    pub(super) player: PlayerProxy,
    pub(super) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(super) ws_rx: mpsc::Receiver<WsEvent>,
    pub(super) ws_send_tx: Option<mbv_core::ws::WsSender>,
    pub(super) audiobookshelf_socket_rx:
        mpsc::Receiver<mbv_core::audiobookshelf_socket::SocketEvent>,
    pub(super) audiobookshelf_socket_tx: Option<mpsc::Sender<()>>,
    pub(super) audiobookshelf_socket_generation: Option<mbv_core::service_runtime::SetupGeneration>,
}

pub(super) enum PendingQueueAction {
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

#[derive(Clone, Debug)]
pub(super) enum PlaylistMutation {
    Save {
        mutation_id: u64,
        queue_lineage: u64,
        source_playlist_id: String,
        item_ids: Option<Vec<String>>,
    },
    CreateAs {
        mutation_id: u64,
        coordinator_key: String,
        name: String,
        queue_lineage: u64,
        source_playlist_id: Option<String>,
        item_ids: Option<Vec<String>>,
    },
    Replace {
        mutation_id: u64,
        queue_lineage: u64,
        name: String,
        item_ids: Option<Vec<String>>,
    },
}

impl PlaylistMutation {
    pub(super) fn mutation_id(&self) -> u64 {
        match self {
            Self::Save { mutation_id, .. }
            | Self::CreateAs { mutation_id, .. }
            | Self::Replace { mutation_id, .. } => *mutation_id,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct PlaylistMutationState {
    pub(super) active: Option<PlaylistMutation>,
    pub(super) queued: VecDeque<PlaylistMutation>,
}
