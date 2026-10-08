use crate::app::state::queue_owner::QueueOrigin;
use mbv_ctrl::player::PlayerEvent;
use mbv_emby_model::EmbyItem;
use mbv_player::PlayerProxy;
use mbv_queue::{QueueItem, QueueSlotId};
use mbv_ui_model::home_latest::{HomeLatestLaunchWindow, is_new_in_launch_window};
use mbv_ui_model::playback::QueueScope;
use std::collections::VecDeque;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueScopeResolution {
    pub has_direct_remote_queue: bool,
    pub requested_visible_scope: QueueScope,
}

impl QueueScopeResolution {
    #[must_use]
    pub fn new(has_direct_remote_queue: bool, requested_visible_scope: QueueScope) -> Self {
        Self {
            has_direct_remote_queue,
            requested_visible_scope,
        }
    }

    #[must_use]
    pub fn playback_target(self) -> QueueScope {
        if self.has_direct_remote_queue {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    #[must_use]
    pub fn visible_scope(self) -> QueueScope {
        if self.has_direct_remote_queue && self.requested_visible_scope == QueueScope::Remote {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    #[must_use]
    pub fn local_metadata_applies(self, scope: QueueScope) -> bool {
        scope == QueueScope::Local || !self.has_direct_remote_queue
    }
}

/// A reversible queue edit (design D7). Undo re-sends the inverse edit to
/// the Player owner: `Remove` re-appends the removed item before the entry
/// now at its former index (resolved at undo time), and `Move` moves the
/// slot back to `from`. `slot_id` is the runtime queue occurrence that was
/// moved; the owner rejects the undo as stale when that slot is gone.
#[derive(Debug)]
pub(in crate::app) enum UndoEntry {
    Remove { item: Box<QueueItem>, index: usize },
    Move { slot_id: QueueSlotId, from: usize },
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
    pub title: String,
    pub source: DestinationLatestSource,
    pub items: Vec<QueueItem>,
    pub has_new_content: bool,
}

impl DestinationLatestSnapshot {
    #[must_use]
    pub fn new_with_launch_window(
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

    pub fn recompute_new_content(&mut self, window: HomeLatestLaunchWindow) {
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
#[derive(Debug)]
pub(in crate::app) struct HomeContent {
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
pub(in crate::app) enum PendingQueueAction {
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
