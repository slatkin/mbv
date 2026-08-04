use mbv_core::api::MediaItem;
use mbv_core::playback_queue::QueueSlotId;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::mpsc;

/// Shared local-vs-remote playback seam for the TUI action layer.
#[derive(Clone, Copy)]
pub(super) struct LocalPlaybackTarget;

#[derive(Clone)]
pub(super) struct RemotePlaybackTarget {
    pub(super) session_id: String,
}

#[derive(Clone)]
pub(super) enum PlaybackTarget {
    Local(LocalPlaybackTarget),
    Remote(RemotePlaybackTarget),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PlaybackState {
    pub(super) active: bool,
    pub(super) active_idx: usize,
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
pub(super) enum QueueScope {
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
    Remove(usize, Box<MediaItem>),
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

pub(super) struct HomePane {
    pub(super) continue_items: Vec<MediaItem>,
    pub(super) continue_cursor: usize,
    pub(super) latest: Vec<(String, String, Vec<MediaItem>, usize)>, // (title, lib_id, items, cursor)
    pub(super) section: usize,                                       // 0=continue, 1..=latest
    /// Flat cursor for the home list (spans continue_items then all latest sections).
    pub(super) home_cursor: usize,
    /// Viewport scroll offset for the home list.
    pub(super) home_scroll: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArtistHeaderSelection {
    pub first_album_id: String,
    pub artist_label: String,
}

pub(super) struct SuspendedLocalSession {
    pub(super) player: PlayerProxy,
    pub(super) player_rx: mpsc::Receiver<PlayerEvent>,
    pub(super) ws_rx: mpsc::Receiver<WsEvent>,
    pub(super) ws_send_tx: Option<mbv_core::ws::WsSender>,
}

pub(super) enum PendingQueueAction {
    PlayItems {
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
    },
    ClearQueue,
}

pub(super) struct RemoteReanchorPopup {
    pub(super) targets: Vec<(usize, String)>,
    pub(super) cursor: usize,
}

#[derive(Clone, Debug)]
pub(super) struct RemoteConsumeOperation {
    pub(super) operation_id: u64,
    pub(super) mutation_id: u64,
    pub(super) session_id: String,
    pub(super) tracking_id: u64,
    pub(super) epoch: u64,
    pub(super) occurrence_id: u64,
    pub(super) playlist_id: String,
    pub(super) entry_id: String,
    pub(super) media_id: String,
    pub(super) queue_slot_id: Option<QueueSlotId>,
    pub(super) queue_lineage: u64,
}

#[derive(Clone, Debug)]
pub(super) enum PlaylistMutation {
    ConsumeValidate {
        mutation_id: u64,
        operation_id: u64,
        session_id: String,
        tracking_id: u64,
        epoch: u64,
        occurrence_id: u64,
        entry_id: String,
        media_id: String,
    },
    ConsumeDelete {
        mutation_id: u64,
        operation_id: u64,
        session_id: String,
        tracking_id: u64,
        epoch: u64,
        occurrence_id: u64,
        entry_id: String,
        media_id: String,
    },
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
        source_playlist_id: String,
        name: String,
        item_ids: Option<Vec<String>>,
    },
}

impl PlaylistMutation {
    pub(super) fn mutation_id(&self) -> u64 {
        match self {
            Self::ConsumeValidate { mutation_id, .. }
            | Self::ConsumeDelete { mutation_id, .. }
            | Self::Save { mutation_id, .. }
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

#[derive(Clone, Debug)]
pub(super) struct RemoteQueueProjection {
    pub(super) session_id: String,
    pub(super) epoch: u64,
    pub(super) queue_lineage: u64,
    pub(super) occurrence_slots: HashMap<u64, QueueSlotId>,
    pub(super) slot_occurrences: HashMap<QueueSlotId, u64>,
}
