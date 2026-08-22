use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::{QueueItem, QueueSlotId};
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

/// Which destination a Home "Latest" pill belongs to: an Emby library (view)
/// id, an Audiobookshelf podcast library id, or the single flattened Feeds
/// pill. This is the merge key — each provider/library only ever touches its
/// own entries when populating `HomePane.latest`.
/// `Audiobookshelf`/`Feeds` variants are constructed by Parts 2 and 3 of
/// #543; matching on them here already keeps the merge keyed per provider.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum HomeLatestSource {
    Emby(String),
    Audiobookshelf(String),
    Feeds,
}

impl HomeLatestSource {
    /// Solid string identity for persistence: `"emby:<id>"`, `"abs:<id>"`,
    /// or `"feeds"`. Restoring by identity (not section index) lets Home
    /// leave the pill unselected until a section matching it actually arrives
    /// asynchronously.
    pub(super) fn pref_key(&self) -> String {
        match self {
            HomeLatestSource::Emby(id) => format!("emby:{id}"),
            HomeLatestSource::Audiobookshelf(id) => format!("abs:{id}"),
            HomeLatestSource::Feeds => "feeds".into(),
        }
    }

    pub(super) fn from_pref_key(key: &str) -> Option<Self> {
        let (prefix, id) = key.split_once(':').unwrap_or((key, ""));
        match prefix {
            "emby" => Some(HomeLatestSource::Emby(id.to_string())),
            "abs" => Some(HomeLatestSource::Audiobookshelf(id.to_string())),
            "feeds" => Some(HomeLatestSource::Feeds),
            _ => None,
        }
    }
}

pub(super) struct HomePane {
    pub(super) continue_items: Vec<EmbyItem>,
    pub(super) continue_cursor: usize,
    pub(super) latest: Vec<(String, HomeLatestSource, Vec<QueueItem>, usize)>, // (title, source, items, cursor)
    pub(super) section: usize, // 0=continue, 1..=latest
    /// Flat cursor for the home list (spans continue_items then all latest sections).
    pub(super) home_cursor: usize,
    /// Viewport scroll offset for the home list.
    pub(super) home_scroll: usize,
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
    },
    ClearQueue,
}

pub(super) struct RemoteReanchorPopup {
    pub(super) targets: Vec<(usize, String)>,
    pub(super) cursor: usize,
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
        source_playlist_id: String,
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

#[derive(Clone, Debug)]
pub(super) struct RemoteQueueProjection {
    pub(super) session_id: String,
    pub(super) epoch: u64,
    pub(super) queue_lineage: u64,
    pub(super) occurrence_slots: HashMap<u64, QueueSlotId>,
    pub(super) slot_occurrences: HashMap<QueueSlotId, u64>,
}
