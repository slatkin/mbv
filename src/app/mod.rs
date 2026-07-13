mod action;
mod actions;
pub(crate) mod images;
mod input;
mod input_resolver;
pub(crate) mod layout;
pub(crate) mod palette;
pub mod render;
mod search;
mod settings;
pub(crate) mod stay_alive;
pub(crate) mod ui_util;

use self::search::SearchSubsystem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
// Set only by SIGHUP or stdin POLLHUP (terminal vanished). Never set by q/SIGTERM.
// The watchdog's forced exit arms only on this flag so clean q-quits are never raced.
static TERMINAL_GONE: AtomicBool = AtomicBool::new(false);

pub(super) const QUEUE_VIEW_POWER: u8 = 1;
pub(super) const QUEUE_VIEW_COUNT: u8 = 2;
pub(super) const POWER_LEFT_WIDTH_DEFAULT: u16 = 40;
pub(super) const POWER_LEFT_WIDTH_STEP: u16 = 5;
/// Width reserved on the right of the tab bar for the volume badge (+ gap/arrow).
pub(super) const TABBAR_RIGHT_RESERVE: u16 = 17;
/// Small left margin so tabs don't sit flush against the terminal edge. The
/// control pill used to live here (hence the old, larger reservation); it now
/// renders in the status bar (see `render_status_bar`) and no longer needs
/// room in the tab row.
pub(super) const TABBAR_LEFT_RESERVE: u16 = 2;

/// Shared local-vs-remote playback seam for the TUI action layer.
#[derive(Clone, Copy)]
struct LocalPlaybackTarget;

#[derive(Clone)]
struct RemotePlaybackTarget {
    session_id: String,
}

#[derive(Clone)]
enum PlaybackTarget {
    Local(LocalPlaybackTarget),
    Remote(RemotePlaybackTarget),
}

extern "C" fn handle_quit_signal(signum: i32) {
    QUIT_REQUESTED.store(true, Ordering::Relaxed);
    if signum == 1 {
        // SIGHUP — terminal closed
        TERMINAL_GONE.store(true, Ordering::Relaxed);
    }
}

fn install_signal_handlers() {
    extern "C" {
        fn signal(signum: i32, handler: unsafe extern "C" fn(i32)) -> usize;
    }
    unsafe {
        signal(1, handle_quit_signal); // SIGHUP — terminal closed
        signal(15, handle_quit_signal); // SIGTERM — process termination
    }
}

// Returns true if stdin (fd 0) has POLLHUP — the PTY master was closed.
fn stdin_has_hup() -> bool {
    let mut pfd = libc::pollfd {
        fd: 0,
        events: 0,
        revents: 0,
    };
    unsafe { libc::poll(&mut pfd, 1, 0) > 0 && (pfd.revents & libc::POLLHUP as libc::c_short) != 0 }
}

// Watchdog thread: detects terminal close (SIGHUP or stdin POLLHUP) and
// ensures the mpv window closes and the process exits even when the main event
// loop is wedged in a blocking crossterm epoll call (which SA_RESTART prevents
// SIGHUP from interrupting). Calls player stop directly — bypassing the event
// loop — so the mpv window closes within one wait_event(0.5) tick. The player
// thread then reports stopped to Emby on its own. Force-exits after 15s as a
// backstop for hung Emby HTTP calls.
//
// The forced exit is gated on TERMINAL_GONE (set only by SIGHUP/stdin POLLHUP),
// never on QUIT_REQUESTED alone. A clean q-quit sets QUIT_REQUESTED but not
// TERMINAL_GONE, so the watchdog stops mpv but never races report_stopped.
fn start_quit_watchdog(quit_handle: Option<mbv_core::player::QuitHandle>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let hup = stdin_has_hup();
            if hup {
                TERMINAL_GONE.store(true, Ordering::Relaxed);
            }
            if TERMINAL_GONE.load(Ordering::Relaxed) || QUIT_REQUESTED.load(Ordering::Relaxed) {
                QUIT_REQUESTED.store(true, Ordering::Relaxed);
                if let Some(ref h) = quit_handle {
                    h.stop();
                }
                if TERMINAL_GONE.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(15));
                    std::process::exit(0);
                }
                return; // clean quit — let the main thread finish report_stopped
            }
        }
    });
}

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};

use ratatui_image::picker::Picker;
use ratatui_image::thread::{ResizeRequest, ResizeResponse};

use mbv_core::api::{parse_mbv_direct_tcp_port, EmbyClient, MediaItem};
use mbv_core::playback_queue::{
    PlaybackQueue, QueueMutationResult, QueueSlotId, RefreshMergeResult, RemoveSlotResult,
};
use mbv_core::player::{Player, PlayerCommand, PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;

#[derive(Clone, Debug)]
enum ContextAction {
    Play,
    PlayFolder(String),
    ShuffleFolder(String),
    Enqueue,
    EnqueueFolder(Box<MediaItem>),
    MarkPlayed(String),
    MarkItemsPlayed(Vec<String>),
    MarkUnplayed(String),
    MarkItemsUnplayed(Vec<String>),
    RemoveFromContinueWatching,
    RemoveFromQueue(usize),
    GoToLibrary(String, String), // (item_id, item_type)
}

struct ContextMenuEntry {
    label: &'static str,
    action: Option<ContextAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MultiSelectKind {
    HiddenLibraries,
    HiddenLatest,
    MyLanguages,
    FeedViewLibraries,
}

struct MultiSelectPopup {
    kind: MultiSelectKind,
    items: Vec<(String, String, bool)>, // (name_lower, display_name, is_hidden)
    cursor: usize,
}

struct ContextMenu {
    x: u16,
    y: u16,
    entries: Vec<ContextMenuEntry>,
    cursor: usize,
}

impl ContextMenu {
    fn first_selectable(entries: &[ContextMenuEntry]) -> usize {
        entries
            .iter()
            .position(|entry| entry.action.is_some())
            .unwrap_or(0)
    }

    fn move_cursor(&mut self, delta: i64) {
        if self.entries.is_empty() {
            return;
        }
        let mut idx = self.cursor as i64;
        loop {
            let next = idx + delta;
            if next < 0 || next >= self.entries.len() as i64 {
                return;
            }
            idx = next;
            if self.entries[idx as usize].action.is_some() {
                self.cursor = idx as usize;
                return;
            }
        }
    }
}

struct LibSearch {
    query: String,
    items: Vec<mbv_core::api::MediaItem>,
    results: Vec<usize>, // indices into items, sorted by score desc
    cursor: usize,       // position within results
    scroll: usize,       // viewport scroll offset for the results list
    loading: bool,       // true while full-library fetch is in flight
}

struct BrowseLevel {
    parent_id: String,
    title: String,
    items: Vec<MediaItem>,
    total_count: usize,
    cursor: usize,
    scroll: usize, // viewport scroll offset for the list
    item_types: Option<String>,
    unplayed_only: bool,
    sort_by: String,
    sort_order: String,
    loading: bool,
    all_items: Option<Vec<MediaItem>>, // prefetched full list for instant search
}

impl BrowseLevel {
    /// Whether every item reported by the server for this level has been
    /// fetched into `items` (i.e. pagination is complete).
    fn is_fully_loaded(&self) -> bool {
        self.items.len() >= self.total_count
    }
}

#[derive(Clone)]
struct FeedHomeVideoGroup {
    folder: MediaItem,
    items: Vec<MediaItem>,
}

#[derive(Clone, Default)]
struct FeedHomeVideoState {
    all_items: Vec<MediaItem>,
    groups: Vec<FeedHomeVideoGroup>,
    loading: bool,
    selected_group: usize,
    video_cursor: usize,
    video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Clamped selected-group index: 0 means "all items", 1-based otherwise.
    /// Centralizes the `selected_group.min(groups.len())` clamp so it can't
    /// drift between the several call sites that need it.
    fn selected_group_index(&self) -> usize {
        self.selected_group.min(self.groups.len())
    }

    /// Length of the currently selected item list, without cloning it.
    fn selected_len(&self) -> usize {
        let group = self.selected_group_index();
        if group == 0 {
            self.all_items.len()
        } else {
            self.groups
                .get(group - 1)
                .map(|g| g.items.len())
                .unwrap_or(0)
        }
    }
}

enum SavePlaylistStage {
    EnterName,
    ConfirmOverwrite { existing_id: String },
}

struct SavePlaylistDialog {
    input: String,
    stage: SavePlaylistStage,
}

const PAGE_SIZE: usize = 100;
const PREFETCH_AHEAD: usize = 25;

enum LibEvent {
    Loaded {
        lib_idx: usize,
        parent_id: String,
        level: BrowseLevel,
    },
    PageAppended {
        lib_idx: usize,
        parent_id: String,
        items: Vec<MediaItem>,
        total_count: usize,
    },
    Refreshed {
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        items: Vec<MediaItem>,
        total_count: usize,
    },
    SearchItemsLoaded {
        lib_idx: usize,
        parent_id: String,
        items: Vec<MediaItem>,
    },
    AllItemsPrefetched {
        lib_idx: usize,
        parent_id: String,
        items: Vec<MediaItem>,
    },
    FeedHomeVideoAggregated {
        lib_idx: usize,
        parent_id: String,
        all_items: Vec<MediaItem>,
        groups: Vec<FeedHomeVideoGroup>,
    },
    AlbumYearFetched {
        album_id: String,
        year: u32,
    },
    AlbumArtistFetched {
        album_id: String,
        artist: String,
    },
    /// Track list for the album currently highlighted in Power View's
    /// album-folder listing, fetched proactively (#145) so the inline album
    /// detail pane has data without a nav_stack drilldown.
    AlbumTracksFetched {
        album_id: String,
        tracks: Vec<MediaItem>,
    },
    /// `switch_tab`: true for user-initiated navigation (switch to the lib tab),
    /// false for startup restore (just populate nav_stack, stay on current tab).
    NavigateTo {
        lib_idx: usize,
        nav_stack: Vec<BrowseLevel>,
        switch_tab: bool,
    },
    PlaylistsLoaded(Vec<MediaItem>),
    PlaylistItemsLoaded {
        playlist_id: String,
        items: Vec<MediaItem>,
    },
    /// Best-effort background refresh of played/position state for the queue
    /// that `restore_queue_state` already populated synchronously from disk.
    /// See `spawn_enrich_queue_state`.
    #[rustfmt::skip]
    QueueEnriched { items: Vec<MediaItem> },
    Error(String),
}

enum SessionEvent {
    Loaded(Vec<mbv_core::api::SessionInfo>),
    ItemRefreshed(String, Box<mbv_core::api::MediaItem>), // (item_id, fresh)
    Error(String),
}

#[derive(Clone, Default)]
struct PlayerTab {
    items: Vec<MediaItem>,
    queue_cursor: usize,
    queue: PlaybackQueue,
}

impl PlayerTab {
    fn new(items: Vec<MediaItem>, queue_cursor: usize) -> Self {
        let queue_cursor = queue_cursor.min(items.len().saturating_sub(1));
        let queue = PlaybackQueue::from_items(items.clone(), None);
        Self {
            items,
            queue_cursor,
            queue,
        }
    }

    fn set_items(&mut self, items: Vec<MediaItem>, queue_cursor: usize) {
        *self = Self::new(items, queue_cursor);
    }

    fn queue_model_matches_items(&self) -> bool {
        self.queue.slots().len() == self.items.len()
            && self
                .queue
                .slots()
                .iter()
                .zip(&self.items)
                .all(|(slot, item)| same_queue_occurrence(&slot.item, item))
    }

    fn sync_queue_model_from_items_if_needed(&mut self) {
        if self.queue_model_matches_items() {
            let updates: Vec<_> = self
                .queue
                .slots()
                .iter()
                .zip(&self.items)
                .map(|(slot, item)| (slot.slot_id, item.clone()))
                .collect();
            for (slot_id, item) in updates {
                let _ = self.queue.update_slot_item(slot_id, item);
            }
        } else {
            self.queue = PlaybackQueue::from_items(self.items.clone(), None);
        }
    }

    fn sync_items_from_queue_model(&mut self) {
        self.items = self
            .queue
            .slots()
            .iter()
            .map(|slot| slot.item.clone())
            .collect();
        self.clamp_cursor();
    }

    fn sync_active_slot(&mut self, active_index: Option<usize>) {
        self.sync_queue_model_from_items_if_needed();
        let active_slot_id = active_index.and_then(|index| self.resolve_slot_at(index));
        if let Some(slot_id) = active_slot_id {
            let _ = self.queue.set_active_slot(slot_id);
        } else {
            self.queue.clear_active_slot();
        }
    }

    fn merge_refresh(&mut self, fetched_items: Vec<MediaItem>) -> RefreshMergeResult {
        self.sync_queue_model_from_items_if_needed();
        let result = self.queue.merge_refresh(fetched_items);
        self.sync_items_from_queue_model();
        result
    }

    fn clamp_cursor(&mut self) {
        if self.items.is_empty() {
            self.queue_cursor = 0;
        } else {
            self.queue_cursor = self.queue_cursor.min(self.items.len() - 1);
        }
    }

    fn slot_id_at(&mut self, index: usize) -> Option<QueueSlotId> {
        self.sync_queue_model_from_items_if_needed();
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    /// Read-only resolution of a display index to the slot currently at that
    /// position. Unlike `slot_id_at`, this does not rebuild the shadow; callers
    /// in the event path want the queue exactly as it stands now.
    fn resolve_slot_at(&self, index: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(index).map(|slot| slot.slot_id)
    }

    fn slot_id_matches_at(&self, index: usize, slot_id: QueueSlotId) -> bool {
        self.queue_model_matches_items()
            && self
                .queue
                .slots()
                .get(index)
                .is_some_and(|slot| slot.slot_id == slot_id)
    }

    fn remove_slot_at(&mut self, index: usize) -> Option<MediaItem> {
        let slot_id = self.slot_id_at(index)?;
        let removed = match self.queue.remove_slot(slot_id) {
            RemoveSlotResult::Removed(slot) => slot.item,
            RemoveSlotResult::RequiresActiveConfirmation(_) | RemoveSlotResult::NotFound => {
                return None;
            }
        };
        self.sync_items_from_queue_model();
        Some(removed)
    }

    fn insert_item_at(&mut self, index: usize, item: MediaItem) {
        self.sync_queue_model_from_items_if_needed();
        self.queue.insert(index, item);
        self.sync_items_from_queue_model();
        self.queue_cursor = index.min(self.items.len().saturating_sub(1));
    }

    fn append_item(&mut self, item: MediaItem) {
        self.sync_queue_model_from_items_if_needed();
        self.queue.append(item);
        self.sync_items_from_queue_model();
    }

    fn append_items(&mut self, items: Vec<MediaItem>) {
        self.sync_queue_model_from_items_if_needed();
        for item in items {
            self.queue.append(item);
        }
        self.sync_items_from_queue_model();
    }

    fn move_slot(&mut self, slot_id: QueueSlotId, to: usize) -> bool {
        self.sync_queue_model_from_items_if_needed();
        if !matches!(
            self.queue.move_slot(slot_id, to),
            QueueMutationResult::Applied(())
        ) {
            return false;
        }
        self.sync_items_from_queue_model();
        self.queue_cursor = to.min(self.items.len().saturating_sub(1));
        true
    }

    fn clear(&mut self) {
        self.set_items(Vec::new(), 0);
    }
}

fn same_queue_occurrence(left: &MediaItem, right: &MediaItem) -> bool {
    left.id == right.id && left.playlist_item_id == right.playlist_item_id
}

struct LocalDaemonBootstrap {
    player_tab: PlayerTab,
    queue_source: crate::config::QueueSource,
    last_played_item_id: Option<String>,
    last_played_completed: bool,
    adopt_queue: Option<(Vec<MediaItem>, usize, crate::config::QueueSource)>,
    /// Per-item resume positions carried over from the saved queue snapshot
    /// (see `QueueState::positions`), so the same best-effort enrichment that
    /// `restore_queue_state` performs for plain local playback also happens
    /// for a cold daemon adopting a saved queue. Empty when there's nothing
    /// to enrich (remote-populated queue, or no saved state).
    positions: std::collections::HashMap<String, i64>,
}

fn bootstrap_local_daemon_queue(
    remote_items: Vec<MediaItem>,
    remote_cursor: usize,
    remote_source: crate::config::QueueSource,
    saved_state: Option<crate::config::QueueState>,
) -> LocalDaemonBootstrap {
    if !remote_items.is_empty() {
        return LocalDaemonBootstrap {
            player_tab: PlayerTab::new(remote_items, remote_cursor),
            queue_source: remote_source,
            last_played_item_id: None,
            last_played_completed: false,
            adopt_queue: None,
            positions: Default::default(),
        };
    }

    let Some(state) = saved_state.filter(|state| !state.items.is_empty()) else {
        return LocalDaemonBootstrap {
            player_tab: PlayerTab::new(remote_items, remote_cursor),
            queue_source: remote_source,
            last_played_item_id: None,
            last_played_completed: false,
            adopt_queue: None,
            positions: Default::default(),
        };
    };

    let cursor = self::actions::queue_restore_cursor(
        &state.items,
        state.cursor,
        state.last_played_item_id.as_deref(),
        state.last_played_completed,
    );
    LocalDaemonBootstrap {
        player_tab: PlayerTab::new(state.items.clone(), cursor),
        queue_source: state.source.clone(),
        last_played_item_id: state.last_played_item_id.clone(),
        last_played_completed: state.last_played_completed,
        adopt_queue: Some((state.items, cursor, state.source)),
        positions: state.positions,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlaybackState {
    active: bool,
    active_idx: usize,
    position_ticks: i64,
    runtime_ticks: i64,
    paused: bool,
}

/// Which queue an operation refers to.
///
/// `Local` is this TUI instance's own queue and carries local-only metadata:
/// dirty state, undo history, saved-playlist source, and on-disk persistence.
/// `Remote` is the queue owned by a directly-controlled mbv daemon or remote
/// instance. A stale `Remote` UI preference is ignored unless a direct remote
/// queue is actually present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueScope {
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
struct QueueScopeResolution {
    has_direct_remote_queue: bool,
    requested_visible_scope: QueueScope,
}

impl QueueScopeResolution {
    fn new(has_direct_remote_queue: bool, requested_visible_scope: QueueScope) -> Self {
        Self {
            has_direct_remote_queue,
            requested_visible_scope,
        }
    }

    fn playback_target(self) -> QueueScope {
        if self.has_direct_remote_queue {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    fn visible_scope(self) -> QueueScope {
        if self.has_direct_remote_queue && self.requested_visible_scope == QueueScope::Remote {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    fn local_metadata_applies(self, scope: QueueScope) -> bool {
        scope == QueueScope::Local || !self.has_direct_remote_queue
    }
}

/// A reversible queue edit. `Remove` re-inserts the item at its old position;
/// `Move` swaps the slot back from `to` to `from`. `slot_id` is the runtime
/// queue occurrence that landed at `to`, checked at undo time so a queue edit
/// made after the move is refused instead of swapping the wrong items.
#[derive(Debug)]
enum UndoEntry {
    Remove(usize, Box<MediaItem>),
    Move {
        from: usize,
        to: usize,
        slot_id: QueueSlotId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemoteSlotState {
    Off,
    AttachedSession,
    DirectRemote,
    LocalDaemon,
}

struct HomePane {
    continue_items: Vec<MediaItem>,
    continue_cursor: usize,
    latest: Vec<(String, String, Vec<MediaItem>, usize)>, // (title, lib_id, items, cursor)
    section: usize,                                       // 0=continue, 1..=latest
    /// Flat cursor for the power-view home list (spans continue_items then all latest sections).
    power_home_cursor: usize,
    /// Viewport scroll offset for the power-view home list.
    power_home_scroll: usize,
}

struct LibraryTab {
    library: MediaItem,
    nav_stack: Vec<BrowseLevel>,
    search: Option<LibSearch>,
    feed_home_video: Option<FeedHomeVideoState>,
    power_detail_item: Option<MediaItem>, // movie pinned for detail view in power left panel
    power_detail_scroll: usize,           // scroll offset into the overview lines
    /// `Some(idx)` = track-selection mode is active for the album currently
    /// shown inline at the album-folder-listing nav level (#145 task 3);
    /// `idx` indexes into that album's cached track list
    /// (`App::album_tracks_cache`). `None` = normal album-list navigation.
    album_track_focus: Option<usize>,
}

struct SuspendedLocalSession {
    player: PlayerProxy,
    player_rx: mpsc::Receiver<PlayerEvent>,
    ws_rx: mpsc::Receiver<WsEvent>,
    ws_send_tx: Option<mbv_core::ws::WsSender>,
}

pub struct App {
    client: Arc<Mutex<EmbyClient>>,
    player: PlayerProxy,
    /// Handle to the live MPRIS D-Bus registration, if one was started for
    /// this session (`App::new` / `App::new_remote` both start one; test
    /// construction via `build()` does not). `None` in tests so they never
    /// spin up a real D-Bus connection.
    ///
    /// `switch_to_direct_remote` and `restore_local_mode` call
    /// `mpris::rebind` on this whenever they swap `player` between a local
    /// `Player` and a `RemotePlayer` (#175): MPRIS must always publish
    /// whichever one currently owns playback, not whatever was live when
    /// the D-Bus service was first registered.
    mpris: Option<crate::mpris::MprisHandle>,
    player_rx: mpsc::Receiver<PlayerEvent>,
    ws_rx: mpsc::Receiver<WsEvent>,
    tab_idx: usize,
    home: HomePane,
    libs: Vec<LibraryTab>,
    player_tab: PlayerTab,
    remote_player_tab: Option<PlayerTab>,
    status: String,
    status_expires: Option<Instant>,
    hidden_libraries: Vec<String>,
    hidden_latest: Vec<String>,
    music_levels: Vec<String>,
    // Per-frame layout geometry from last render, used for mouse hit-testing.
    // See src/app/layout.rs for the grouping rationale.
    layout: layout::AppLayout,
    terminal_width: u16,
    terminal_height: u16,

    home_panel_section_offset: usize,
    home_cards_section_offset: usize,
    /// True from startup until the first `fetch_home` completes. While true,
    /// the home view doesn't yet know how many remote sections exist, so the
    /// renderer fills the reserved area with skeleton placeholders instead of
    /// collapsing to just the sections that happen to be populated so far.
    home_loading: bool,
    mouse_col: u16,
    mouse_row: u16,
    last_click_time: Instant,
    last_click_pos: (u16, u16),
    last_drag_seek: Instant,
    confirm_remove_idx: Option<usize>, // playlist index pending removal confirmation
    pending_delete_idx: Option<usize>, // deferred removal of now-playing item after Stopped event
    pending_queue_removal: Option<(QueueSlotId, bool)>, // deferred removal (slot, is_audio) after TrackChanged index-shifts
    confirm_clear_queue: bool,
    queue_undo_stack: Vec<UndoEntry>,
    remote_queue_undo_stack: Vec<UndoEntry>,
    pending_remote_move_cursor: Option<usize>,
    skip_intro_end_ticks: Option<i64>,
    next_up_item: Option<MediaItem>,
    queue_view: u8,
    queue_group: bool, // list view: group audio by album / episodes by series
    power_focus: PowerFocus,
    power_left_tab: usize, // 0 = Home/CW, 1..=libs.len() = library index
    power_left_width: u16,
    power_left_tab_pending: usize, // restored from prefs; applied once libs have loaded
    power_queue_scroll: usize,
    // Whether the power-view queue is currently relocated to the bottom of the
    // right column (low-height layout). Sticky with hysteresis so a small,
    // transient change in the card image's rendered height (e.g. switching
    // from a season poster to an episode thumbnail while browsing seasons)
    // doesn't flip the whole right-panel layout and cause a visible reflow.
    power_queue_relocated: bool,
    home_card_view: bool,
    last_played_item_id: Option<String>,
    last_played_completed: bool,
    card_image_states:
        std::collections::HashMap<String, Option<ratatui_image::thread::ThreadProtocol>>,
    image_lru: std::collections::VecDeque<String>,
    image_cache_size: usize,
    card_image_loading: std::collections::HashSet<String>,
    last_card_height: u16,
    pending_image_fetches: std::collections::VecDeque<images::ImageFetchReq>,
    image_fetches_active: usize,
    card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    /// Registers a freshly created per-cache-key `ResizeRequest` receiver
    /// with the resize worker thread (see `spawn_resize_worker`), so the
    /// worker can service many concurrently-alive `ThreadProtocol`s off the
    /// render thread while still routing each `ResizeResponse` back to the
    /// right `card_image_states` entry (#164). `ResizeRequest`/`ResizeResponse`
    /// carry no key of their own — that's why each cache key gets its own
    /// dedicated channel instead of sharing one globally.
    resize_register_tx: ResizeRegisterTx,
    /// Completed off-thread resize+encode results, tagged with the
    /// `card_image_states` cache key they belong to. Drained once per
    /// event-loop tick alongside `card_image_rx` (#164).
    resize_response_rx: ResizeResponseRx,
    image_picker: Option<Picker>,
    context_menu: Option<ContextMenu>,
    show_help: bool,
    show_settings: bool,
    settings_cursor: usize,
    settings_scroll: usize,
    settings_save_at: Option<Instant>,
    confirm_logout: bool,
    multiselect_popup: Option<MultiSelectPopup>,
    help_scroll: u16,
    system_notifications: bool,
    notif_failed: bool,
    notif_action_tx: mpsc::Sender<String>,
    notif_action_rx: mpsc::Receiver<String>,
    lib_tx: mpsc::Sender<LibEvent>,
    lib_rx: mpsc::Receiver<LibEvent>,
    search: SearchSubsystem,
    sessions: Vec<mbv_core::api::SessionInfo>,
    sessions_cursor: usize,
    sessions_scroll: usize,
    sessions_loading: bool,
    show_sessions: bool,
    playlists: Vec<MediaItem>,
    playlists_cursor: usize,
    playlists_scroll: usize,
    playlists_loading: bool,
    show_playlists: bool,
    playlists_open: Option<MediaItem>, // playlist currently being browsed
    playlists_open_items: Vec<MediaItem>,
    playlists_open_cursor: usize,
    playlists_open_scroll: usize,
    playlists_open_loading: bool,
    queue_source: crate::config::QueueSource,
    queue_dirty: bool,
    pending_queue_action: Option<PendingQueueAction>,
    show_save_playlist_modal: bool,
    use_nerd_fonts: bool,
    indicator_style: render::indicators::IndicatorStyle,
    panel_mode: crate::config::PanelMode,
    ws_send_tx: Option<mbv_core::ws::WsSender>,
    last_keepalive: Instant,
    last_capabilities: Instant,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
    connected_session_id: Option<String>,
    connected_session_state: Option<mbv_core::api::SessionInfo>,
    last_session_poll: Instant,
    session_miss_count: u8, // consecutive polls that didn't find the connected session
    remote_pos_s: i64,      // monotonic position estimate for the connected remote
    remote_pos_at: Instant, // when remote_pos_s was last anchored
    remote_api_pos_advanced_at: Instant, // last time the API position actually moved forward
    remote_seek_pending_until: Instant, // suppress poll pos-reconcile after a seek
    runtime_zero_since: Option<Instant>, // when runtime_s first became 0 for the current item (fast-poll cap)
    suspended_local: Option<SuspendedLocalSession>,
    force_clear: bool,
    tab_scroll: usize,
    ui_volume: u8,
    pre_mute_volume: Option<u8>,
    mute_on: bool,
    last_scroll_at: Instant,
    last_nav_at: Instant,
    album_year_cache: std::collections::HashMap<String, u32>,
    album_year_loading: std::collections::HashSet<String>,
    album_artist_cache: std::collections::HashMap<String, String>,
    album_artist_loading: std::collections::HashSet<String>,
    pending_album_artist_fetches: std::collections::VecDeque<String>,
    album_artist_fetches_active: usize,
    /// Track lists for the album currently highlighted in Power View's
    /// album-folder listing, fetched proactively so the inline album detail
    /// pane (#145) has data without requiring the user to drill in first.
    /// Keyed by album id, mirroring `album_artist_cache`'s never-evicted
    /// lifetime.
    album_tracks_cache: std::collections::HashMap<String, Vec<MediaItem>>,
    album_tracks_loading: std::collections::HashSet<String>,
    save_playlist_dialog: Option<SavePlaylistDialog>,
    image_protocol: Option<String>,
    image_protocol_enabled: bool,
    confirm_rescan: bool,
    queue_scope: QueueScope,
    /// The relay's out-of-band control channel (ADR 0005), present only
    /// when running as a stay-alive inferior under a relay. `None` in bare
    /// mode and for `new_remote` (thin client to `mbvd`).
    stay_alive_ctrl: Option<stay_alive::StayAliveCtrl>,
    /// Whether a terminal-client is currently attached to the pty. Always
    /// `true` outside stay-alive mode (`stay_alive_ctrl` is `None` there, so
    /// this field is never consulted). Set `false` by `try_quit`'s detach
    /// path right after a successful `send_detach()`, and back to `true` by
    /// the T5 reattach-refresh (`take_attach_pending()`).
    ///
    /// Exists because `Terminal::clear()` unconditionally queries the
    /// cursor position over the pty (crossterm `get_cursor_position()`,
    /// a blocking DSR round-trip) even for a fullscreen viewport. The
    /// run loop keeps ticking and taking input while detached (that's the
    /// point of stay-alive), so without this guard, the very next
    /// `force_clear` — triggered by any number of ordinary UI actions,
    /// unrelated to detach — blocks for several seconds with no
    /// terminal-client left to answer, then errors out and kills the whole
    /// process: a silent `exit(1)` if idle, or a SIGSEGV if it races a live
    /// mpv Vulkan render thread during the resulting early-return teardown
    /// (issue #156).
    attached: bool,
    #[cfg(test)]
    _test_state_dir_guard: Option<crate::config::TestStateDirGuard>,
}

struct AppInit {
    client: std::sync::Arc<std::sync::Mutex<EmbyClient>>,
    player: mbv_core::player::PlayerProxy,
    player_rx: std::sync::mpsc::Receiver<mbv_core::player::PlayerEvent>,
    ws_rx: std::sync::mpsc::Receiver<WsEvent>,
    ws_send_tx: Option<mbv_core::ws::WsSender>,
    player_tab: PlayerTab,
    remote_player_tab: Option<PlayerTab>,
    initial_queue_scope: QueueScope,
    system_notifications: bool,
    image_protocol: Option<String>,
    image_protocol_enabled: bool,
    hidden_libraries: Vec<String>,
    hidden_latest: Vec<String>,
    music_levels: Vec<String>,
    use_nerd_fonts: bool,
    indicator_style: render::indicators::IndicatorStyle,
    image_cache_size: usize,
    start_on_queue: bool,
    lib_tx: mpsc::Sender<LibEvent>,
    lib_rx: mpsc::Receiver<LibEvent>,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
    card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    notif_action_tx: mpsc::Sender<String>,
    notif_action_rx: mpsc::Receiver<String>,
    search_tx: mpsc::Sender<Result<Vec<MediaItem>, String>>,
    search_rx: mpsc::Receiver<Result<Vec<MediaItem>, String>>,
    stay_alive_ctrl: Option<stay_alive::StayAliveCtrl>,
}

/// Registers a per-cache-key `ResizeRequest` receiver with the resize
/// worker thread; see `spawn_resize_worker`.
type ResizeRegisterTx = mpsc::Sender<(String, mpsc::Receiver<ResizeRequest>)>;
/// Completed off-thread resize+encode results, tagged with the
/// `card_image_states` cache key they belong to; see `spawn_resize_worker`.
type ResizeResponseRx = mpsc::Receiver<(String, ResizeResponse)>;

/// Spawns the single background worker that performs
/// `StatefulProtocol::resize_encode()` — resample + terminal-protocol encode
/// (e.g. kitty's base64 payload) — off the render thread (#164).
///
/// `ResizeRequest`/`ResizeResponse` (from `ratatui_image::thread`) carry no
/// identifying key of their own, so a single shared request channel can't
/// tell the worker which `card_image_states` entry a given request came
/// from. Instead, each cache key gets its own dedicated `ResizeRequest`
/// channel (created in `App::new_thread_protocol`), registered with this
/// worker over `resize_register_tx`. The worker round-robins a `try_recv`
/// poll across all registered per-key receivers — still entirely off the
/// render thread — and tags each result with its key before sending it back
/// over the single shared `resize_response_rx`.
///
/// A per-key receiver whose sender has been dropped (its `ThreadProtocol`
/// evicted from `card_image_states`, e.g. by LRU eviction) is simply
/// removed from the poll set; it never produces a response. A panic inside
/// `resize_encode()` is caught so it cannot silently stall every other
/// in-flight or future resize request on this worker — only that one
/// image's response is lost, same failure mode as the request simply never
/// arriving.
fn spawn_resize_worker() -> (ResizeRegisterTx, ResizeResponseRx) {
    let (register_tx, register_rx) = mpsc::channel::<(String, mpsc::Receiver<ResizeRequest>)>();
    let (response_tx, response_rx) = mpsc::channel::<(String, ResizeResponse)>();
    std::thread::spawn(move || {
        let mut receivers: Vec<(String, mpsc::Receiver<ResizeRequest>)> = Vec::new();
        loop {
            loop {
                match register_rx.try_recv() {
                    Ok(pair) => receivers.push(pair),
                    Err(mpsc::TryRecvError::Empty) => break,
                    // App is gone; nothing left to serve.
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            let mut did_work = false;
            let mut i = 0;
            while i < receivers.len() {
                match receivers[i].1.try_recv() {
                    Ok(request) => {
                        did_work = true;
                        let key = receivers[i].0.clone();
                        // catch_unwind: a panic here must not kill this
                        // long-lived worker thread, which would silently
                        // stall every other key's resize requests forever.
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            request.resize_encode()
                        }));
                        if let Ok(Ok(response)) = result {
                            let _ = response_tx.send((key, response));
                        }
                        i += 1;
                    }
                    Err(mpsc::TryRecvError::Empty) => i += 1,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        receivers.remove(i);
                    }
                }
            }
            if !did_work {
                std::thread::sleep(Duration::from_millis(4));
            }
        }
    });
    (register_tx, response_rx)
}

enum PendingQueueAction {
    PlayItems {
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
    },
    ClearQueue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum PowerFocus {
    Queue, // left panel (queue list below the card)
    #[default]
    Left, // right panel (library browser); driven by power_left_tab
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingKey {
    StayAlive,
    SavePlaylistOnQuit,
    StartOnQueue,
    AlwaysPlayNext,
    ConsumeVideos,
    ConsumeAudio,
    SavePlaylistOnConsume,
    SavePlaylistOnConsumeAudio,
    AlwaysSkipIntro,
    ImageProtocol,
    HiddenLibraries,
    HiddenLatest,
    ShowAudioWindow,
    UseMpvConfig,
    NoScripts,
    Autoload,
    ShowSysTrayIcon,
    SystemNotifications,
    MyLanguages,
    SubtitleMode,
    FeedViewLibraries,
    SubtitleLanguage,
    AudioLanguage,
    LogOut,
}

// Sections rendered as YELLOW blocks in a 2×2 grid.
// LogOut is rendered separately as a plain line below the grid.
static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
    (
        "[general]",
        &[
            SettingKey::StayAlive,
            SettingKey::SavePlaylistOnQuit,
            SettingKey::AlwaysSkipIntro,
            SettingKey::SystemNotifications,
            SettingKey::ImageProtocol,
            SettingKey::HiddenLibraries,
            SettingKey::HiddenLatest,
            SettingKey::FeedViewLibraries,
        ],
    ),
    (
        "[queue]",
        &[
            SettingKey::StartOnQueue,
            SettingKey::AlwaysPlayNext,
            SettingKey::ConsumeVideos,
            SettingKey::ConsumeAudio,
            SettingKey::SavePlaylistOnConsume,
            SettingKey::SavePlaylistOnConsumeAudio,
        ],
    ),
    (
        "[mpv]",
        &[
            SettingKey::ShowAudioWindow,
            SettingKey::UseMpvConfig,
            SettingKey::NoScripts,
            SettingKey::Autoload,
        ],
    ),
    (
        "[playback]",
        &[
            SettingKey::MyLanguages,
            SettingKey::SubtitleMode,
            SettingKey::SubtitleLanguage,
            SettingKey::AudioLanguage,
        ],
    ),
    ("[daemon]", &[SettingKey::ShowSysTrayIcon]),
    ("[actions]", &[SettingKey::LogOut]),
];

const SESSIONS_PANEL_W: u16 = 40;
const HELP_PANEL_W: u16 = 40;
const SETTINGS_PANEL_W: u16 = 40;
const PLAYLISTS_PANEL_W: u16 = 40;
const HOME_MIN_SECTION_H: u16 = 7; // 1 header row + 6 content rows (3 two-line items)
impl App {
    pub(super) fn power_left_width_max_for_terminal(terminal_width: u16) -> u16 {
        POWER_LEFT_WIDTH_DEFAULT.max(terminal_width.saturating_mul(3) / 5)
    }

    pub(super) fn normalize_power_left_width(width: u16, terminal_width: u16) -> u16 {
        width.clamp(
            POWER_LEFT_WIDTH_DEFAULT,
            Self::power_left_width_max_for_terminal(terminal_width),
        )
    }

    pub(super) fn clamp_power_left_width(&mut self) -> bool {
        let normalized =
            Self::normalize_power_left_width(self.power_left_width, self.terminal_width);
        if normalized == self.power_left_width {
            return false;
        }
        self.power_left_width = normalized;
        true
    }

    fn remote_slot_state(&self) -> RemoteSlotState {
        if self.connected_session_id.is_some() {
            RemoteSlotState::AttachedSession
        } else if self.player.is_remote() {
            if self.has_remote_queue() {
                RemoteSlotState::DirectRemote
            } else {
                RemoteSlotState::LocalDaemon
            }
        } else {
            RemoteSlotState::Off
        }
    }

    fn can_disconnect_remote(&self) -> bool {
        !matches!(
            self.remote_slot_state(),
            RemoteSlotState::Off | RemoteSlotState::LocalDaemon
        )
    }

    fn disconnect_remote(&mut self) {
        match self.remote_slot_state() {
            RemoteSlotState::AttachedSession => {
                self.connected_session_id = None;
                self.connected_session_state = None;
                self.session_miss_count = 0;
                self.remote_pos_s = 0;
                self.flash_status("Disconnected from remote session".to_string());
            }
            RemoteSlotState::DirectRemote => {
                self.restore_local_mode("Disconnected from direct remote session");
            }
            RemoteSlotState::LocalDaemon => {
                self.flash_status("Local daemon mode stays connected".to_string());
            }
            RemoteSlotState::Off => {
                self.flash_status("No remote session to disconnect".to_string());
            }
        }
    }

    fn sessions_overlay_footer(&self) -> &'static str {
        if self.can_disconnect_remote() {
            "[↵]conn [d]disc [r]refresh [Esc]close"
        } else {
            "[↵]conn [r]refresh [Esc]close"
        }
    }

    fn extrapolated_remote_position(remote_pos_s: i64, elapsed: Duration) -> i64 {
        remote_pos_s + elapsed.as_secs() as i64
    }

    fn ui_config_snapshot(&self) -> crate::config::UiConfig {
        let indicator_style = match self.indicator_style {
            render::indicators::IndicatorStyle::Brackets => "brackets",
            render::indicators::IndicatorStyle::Chips => "chips",
            render::indicators::IndicatorStyle::Outlined => "outlined",
            render::indicators::IndicatorStyle::Dots => "dots",
            render::indicators::IndicatorStyle::Pipes => "pipes",
            render::indicators::IndicatorStyle::KeyValue => "keyvalue",
            render::indicators::IndicatorStyle::Powerline => "powerline",
        };
        crate::config::UiConfig {
            image_protocol: self.image_protocol.clone(),
            show_log_tab: false,
            image_cache_size: self.image_cache_size,
            use_nerd_fonts: self.use_nerd_fonts,
            indicator_style: indicator_style.to_string(),
        }
    }

    fn build(init: AppInit) -> Self {
        let prefs = Self::load_prefs();
        let (resize_register_tx, resize_response_rx) = spawn_resize_worker();
        App {
            #[cfg(test)]
            _test_state_dir_guard: crate::config::TestStateDirGuard::new_if_unset(),
            client: init.client,
            player: init.player,
            mpris: None,
            player_rx: init.player_rx,
            ws_rx: init.ws_rx,
            ws_send_tx: init.ws_send_tx,
            player_tab: init.player_tab,
            remote_player_tab: init.remote_player_tab,
            system_notifications: init.system_notifications,
            image_protocol: init.image_protocol,
            image_protocol_enabled: init.image_protocol_enabled,
            hidden_libraries: init.hidden_libraries,
            hidden_latest: init.hidden_latest,
            music_levels: init.music_levels,
            use_nerd_fonts: init.use_nerd_fonts,
            indicator_style: init.indicator_style,
            image_cache_size: init.image_cache_size,
            tab_idx: if init.start_on_queue {
                1
            } else {
                prefs["tab_idx"].as_u64().unwrap_or(0) as usize
            },
            lib_tx: init.lib_tx,
            lib_rx: init.lib_rx,
            search: SearchSubsystem::new(init.search_tx, init.search_rx),
            sessions_tx: init.sessions_tx,
            sessions_rx: init.sessions_rx,
            card_image_tx: init.card_image_tx,
            card_image_rx: init.card_image_rx,
            resize_register_tx,
            resize_response_rx,
            notif_action_tx: init.notif_action_tx,
            notif_action_rx: init.notif_action_rx,
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
                power_home_cursor: 0,
                power_home_scroll: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            layout: layout::AppLayout {
                power: layout::LayoutPower {
                    detail_page_h: 5,
                    ..Default::default()
                },
                ..Default::default()
            },
            terminal_width: 80,
            terminal_height: 24,

            home_panel_section_offset: 0,
            home_cards_section_offset: 0,
            home_loading: true,
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_click_pos: (u16::MAX, u16::MAX),
            confirm_remove_idx: None,
            pending_delete_idx: None,
            pending_queue_removal: None,
            confirm_clear_queue: false,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
            pending_remote_move_cursor: None,
            skip_intro_end_ticks: None,
            next_up_item: None,
            queue_view: prefs["playlist_view"].as_u64().unwrap_or(0).min(1) as u8,
            queue_group: true,
            power_focus: PowerFocus::default(),
            power_left_tab: 0,
            power_left_width: prefs["power_left_width"]
                .as_u64()
                .map(|v| (v as u16).max(POWER_LEFT_WIDTH_DEFAULT))
                .unwrap_or(POWER_LEFT_WIDTH_DEFAULT),
            power_left_tab_pending: prefs["power_left_tab"].as_u64().unwrap_or(0) as usize,
            power_queue_scroll: 0,
            power_queue_relocated: false,
            home_card_view: false,
            ui_volume: prefs["ui_volume"].as_u64().unwrap_or(100).min(200) as u8,
            pre_mute_volume: prefs["pre_mute_volume"].as_u64().map(|v| v as u8),
            mute_on: prefs["mute_on"].as_bool().unwrap_or(false),
            last_played_item_id: None,
            last_played_completed: false,
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            image_picker: None,
            show_help: false,
            show_settings: false,
            settings_cursor: 0,
            settings_scroll: 0,
            settings_save_at: None,
            confirm_logout: false,
            multiselect_popup: None,
            help_scroll: 0,
            notif_failed: false,
            context_menu: None,
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_scroll: 0,
            sessions_loading: false,
            show_sessions: false,
            playlists: Vec::new(),
            playlists_cursor: 0,
            playlists_scroll: 0,
            playlists_loading: false,
            show_playlists: false,
            playlists_open: None,
            playlists_open_items: Vec::new(),
            playlists_open_cursor: 0,
            playlists_open_scroll: 0,
            playlists_open_loading: false,
            queue_source: crate::config::QueueSource::Unknown,
            queue_dirty: false,
            pending_queue_action: None,
            show_save_playlist_modal: false,
            panel_mode: crate::config::PanelMode::default(),
            last_keepalive: Instant::now(),
            last_capabilities: Instant::now(),
            connected_session_id: None,
            connected_session_state: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            remote_api_pos_advanced_at: Instant::now() - Duration::from_secs(60),
            remote_seek_pending_until: Instant::now() - Duration::from_secs(1),
            runtime_zero_since: None,
            suspended_local: None,
            force_clear: false,
            tab_scroll: 0,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
            last_nav_at: Instant::now() - Duration::from_secs(1),
            album_year_cache: std::collections::HashMap::new(),
            album_year_loading: std::collections::HashSet::new(),
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_loading: std::collections::HashSet::new(),
            pending_album_artist_fetches: std::collections::VecDeque::new(),
            album_artist_fetches_active: 0,
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            confirm_rescan: false,
            queue_scope: init.initial_queue_scope,
            stay_alive_ctrl: init.stay_alive_ctrl,
            attached: true,
        }
    }

    pub fn new(client: EmbyClient) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (ws_tx, ws_rx) = mpsc::channel();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<Result<Vec<MediaItem>, String>>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let server_url = client.config.server_url.clone();
        let token = client.token.clone();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let system_notifications = client.config.system_notifications;
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
        let start_on_queue = client.config.start_on_queue;
        let always_play_next = client.config.always_play_next;
        let always_skip_intro = client.config.always_skip_intro;
        crate::config::evict_old_image_cache();
        let ws_url = client.ws_url();
        let ws_send_tx = mbv_core::ws::start(ws_url, ws_tx);
        let ws_send_tx_app = ws_send_tx.clone();
        // Prefer local config; fall back to Emby server prefs only on first run (all empty).
        let subtitle_prefs = if client.config.subtitle_mode.is_empty()
            && client.config.subtitle_lang.is_empty()
            && client.config.audio_lang.is_empty()
        {
            client.get_user_subtitle_prefs().unwrap_or_default()
        } else {
            mbv_core::player::SubtitlePrefs {
                mode: client.config.subtitle_mode.clone(),
                subtitle_lang: client.config.subtitle_lang.clone(),
                audio_lang: client.config.audio_lang.clone(),
            }
        };
        let raw_player = Player::new(
            server_url,
            token,
            client.config.show_audio_window,
            client.config.use_mpv_config,
            client.config.no_scripts,
            always_play_next,
            always_skip_intro,
            subtitle_prefs,
            player_tx,
            Some(ws_send_tx),
        );
        let player_status = raw_player.status.clone();
        let player_cmd_tx = raw_player.cmd_tx.clone();
        let mpris_handle = crate::mpris::start(
            player_status,
            move |cmd| {
                if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                    let _ = tx.send(cmd);
                }
            },
            None,
        );
        let player = PlayerProxy::local(raw_player, always_play_next);
        let client_arc = Arc::new(Mutex::new(client));
        {
            let c = client_arc.clone();
            std::thread::spawn(move || {
                let mut probe = c.lock().unwrap().clone();
                probe.probe_chapter_api();
                c.lock().unwrap().chapter_api_available = probe.chapter_api_available;
            });
        }
        let mut app = Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: Some(ws_send_tx_app),
            player_tab: PlayerTab::default(),
            remote_player_tab: None,
            initial_queue_scope: QueueScope::Local,
            system_notifications,
            image_protocol,
            image_protocol_enabled,
            hidden_libraries,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
            start_on_queue,
            lib_tx,
            lib_rx,
            sessions_tx,
            sessions_rx,
            card_image_tx,
            card_image_rx,
            notif_action_tx,
            notif_action_rx,
            search_tx,
            search_rx,
            stay_alive_ctrl: stay_alive::StayAliveCtrl::from_env(),
        });
        app.mpris = Some(mpris_handle);
        app
    }

    /// `is_local_daemon` distinguishes the two daemon-connection modes:
    /// - `true`: this is the same-machine `mbv -d` daemon, auto-detected at
    ///   startup (`DaemonEndpoint::Local`). This should behave exactly like
    ///   a plain local session — one unified queue, normal queue-state
    ///   persistence — the only difference is that the daemon owns mpv
    ///   instead of an in-process `Player`. No Local/Remote split, no pill.
    /// - `false`: a genuinely remote/network daemon (explicit
    ///   `--daemon-endpoint`/`daemon_client_endpoint`). Here a separate
    ///   `remote_player_tab` is kept so the user can browse locally while a
    ///   daemon elsewhere plays something else, with the Local/Remote scope
    ///   pill to switch between them (mirroring `switch_to_direct_remote`'s
    ///   mid-session upgrade case).
    pub fn new_remote(
        client: EmbyClient,
        remote: mbv_core::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        is_local_daemon: bool,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<mbv_core::ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<Result<Vec<MediaItem>, String>>();
        let ui_config = crate::config::load_ui_config().unwrap_or_default();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let always_play_next = client.config.always_play_next;
        let start_on_queue = client.config.start_on_queue;
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
        crate::config::evict_old_image_cache();
        let client_arc = Arc::new(Mutex::new(client));
        {
            let c = client_arc.clone();
            std::thread::spawn(move || {
                let mut probe = c.lock().unwrap().clone();
                probe.probe_chapter_api();
                c.lock().unwrap().chapter_api_available = probe.chapter_api_available;
            });
        }
        let remote_items = remote.items.lock().unwrap().clone();
        let remote_cursor = remote.status.lock().unwrap().current_idx;
        let remote_queue_source = remote.queue_source.lock().unwrap().clone();
        let initial_queue_scope = if !is_local_daemon && !remote_items.is_empty() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        };
        let local_daemon_bootstrap = is_local_daemon.then(|| {
            bootstrap_local_daemon_queue(
                remote_items.clone(),
                remote_cursor,
                remote_queue_source.clone(),
                crate::config::load_queue_state(),
            )
        });
        // `adopt_queue` returns false when the ctrl socket is already dead
        // (the command send failed); tracked so construction doesn't
        // silently carry on with a queue the daemon never actually adopted
        // (#119 task 5) — see `handle_failed_local_daemon_adoption` below.
        let local_daemon_adoption_failed = local_daemon_bootstrap
            .as_ref()
            .and_then(|bootstrap| bootstrap.adopt_queue.clone())
            .is_some_and(|(items, cursor, source)| !remote.adopt_queue(items, cursor, source));
        // Start MPRIS against this `RemotePlayer` (#175, previously done in
        // `main.rs::run_remote_app` before this constructor even ran).
        // Moved here so App owns the resulting handle and can `rebind` it
        // later if `switch_to_direct_remote` / `restore_local_mode` swap
        // which target owns playback.
        let mpris_remote = remote.clone();
        let mpris_handle = crate::mpris::start(
            mpris_remote.status.clone(),
            move |cmd| {
                mpris_remote.send_command(cmd);
            },
            Some(remote.disconnected_flag()),
        );
        let player = PlayerProxy::remote(remote, always_play_next);
        let (player_tab, remote_player_tab) = if is_local_daemon {
            // Local daemon: one unified queue, exactly like plain local
            // playback — no separate remote_player_tab, no scope pill.
            (
                local_daemon_bootstrap.as_ref().unwrap().player_tab.clone(),
                None,
            )
        } else {
            // Remote/network daemon: keep a separate remote queue so the
            // user can browse locally while the daemon plays elsewhere.
            (
                PlayerTab::default(),
                Some(PlayerTab::new(remote_items, remote_cursor)),
            )
        };
        let mut app = Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            player_tab,
            remote_player_tab,
            initial_queue_scope,
            system_notifications: false,
            image_protocol,
            image_protocol_enabled,
            hidden_libraries,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
            start_on_queue,
            lib_tx,
            lib_rx,
            sessions_tx,
            sessions_rx,
            card_image_tx,
            card_image_rx,
            notif_action_tx,
            notif_action_rx,
            search_tx,
            search_rx,
            stay_alive_ctrl: None,
        });
        app.mpris = Some(mpris_handle);
        if is_local_daemon {
            let bootstrap = local_daemon_bootstrap.unwrap();
            app.queue_source = bootstrap.queue_source;
            app.last_played_item_id = bootstrap.last_played_item_id;
            app.last_played_completed = bootstrap.last_played_completed;
            if !bootstrap.positions.is_empty() {
                app.spawn_enrich_queue_state(bootstrap.positions);
            }
        } else {
            app.queue_source = remote_queue_source;
        }
        if local_daemon_adoption_failed {
            app.handle_failed_local_daemon_adoption();
        }
        app
    }

    /// Routes a local-daemon queue adoption whose command send failed (dead
    /// ctrl socket, see `new_remote`) through the same disconnect handling a
    /// live `PlayerEvent::RemoteDisconnected` uses, instead of silently
    /// continuing to build on optimistic queue state the daemon never
    /// actually received (#119 task 5).
    fn handle_failed_local_daemon_adoption(&mut self) {
        self.handle_player_event(PlayerEvent::RemoteDisconnected(
            "local daemon connection lost while restoring the saved queue".to_string(),
        ));
    }

    fn has_remote_queue(&self) -> bool {
        self.remote_player_tab.is_some()
    }

    fn has_direct_remote_queue(&self) -> bool {
        self.player.is_remote() && self.has_remote_queue()
    }

    fn queue_for_scope(&self, scope: QueueScope) -> &PlayerTab {
        match scope {
            QueueScope::Local => &self.player_tab,
            QueueScope::Remote => self
                .remote_player_tab
                .as_ref()
                .expect("remote queue scope requires remote queue"),
        }
    }

    fn queue_for_scope_mut(&mut self, scope: QueueScope) -> &mut PlayerTab {
        match scope {
            QueueScope::Local => &mut self.player_tab,
            QueueScope::Remote => self
                .remote_player_tab
                .as_mut()
                .expect("remote queue scope requires remote queue"),
        }
    }

    fn undo_stack_for_scope_mut(&mut self, scope: QueueScope) -> &mut Vec<UndoEntry> {
        match scope {
            QueueScope::Local => &mut self.queue_undo_stack,
            QueueScope::Remote => &mut self.remote_queue_undo_stack,
        }
    }

    fn queue_scope_resolution(&self) -> QueueScopeResolution {
        QueueScopeResolution::new(self.has_direct_remote_queue(), self.queue_scope)
    }

    fn local_queue_metadata_applies(&self, scope: QueueScope) -> bool {
        self.queue_scope_resolution().local_metadata_applies(scope)
    }

    fn queue_scope_is_playback(&self, scope: QueueScope) -> bool {
        scope == self.playback_target_queue_scope()
    }

    fn action_queue_scope(&self, action: &PendingQueueAction) -> QueueScope {
        match action {
            PendingQueueAction::PlayItems { .. } => self.playback_target_queue_scope(),
            PendingQueueAction::ClearQueue => self.visible_queue_scope(),
        }
    }

    fn action_touches_local_queue(&self, action: &PendingQueueAction) -> bool {
        self.local_queue_metadata_applies(self.action_queue_scope(action))
    }

    fn clear_local_queue_metadata(&mut self) {
        self.queue_source = crate::config::QueueSource::Unknown;
        self.queue_dirty = false;
        self.queue_undo_stack.clear();
    }

    fn persist_local_queue_state_if_needed(&self, scope: QueueScope) {
        if self.local_queue_metadata_applies(scope) {
            self.save_queue_state();
        }
    }

    fn replace_direct_remote_queue(&mut self, items: Vec<MediaItem>, cursor: usize) {
        let cursor = cursor.min(items.len().saturating_sub(1));
        self.player
            .send_command(crate::player::PlayerCommand::ReplaceQueue {
                items: items.clone(),
                start_idx: cursor,
            });
        if let Some(queue) = self.remote_player_tab.as_mut() {
            queue.set_items(items, cursor);
        }
    }

    fn sync_direct_remote_queue_after_edit(&mut self, scope: QueueScope) {
        if scope == QueueScope::Remote && self.has_direct_remote_queue() {
            let (items, cursor) = {
                let queue = self
                    .remote_player_tab
                    .as_ref()
                    .expect("direct remote queue requires remote queue");
                (queue.items.clone(), queue.queue_cursor)
            };
            self.replace_direct_remote_queue(items, cursor);
        }
    }

    fn playback_target_queue_scope(&self) -> QueueScope {
        self.queue_scope_resolution().playback_target()
    }

    fn replace_playback_queue(&mut self, items: Vec<MediaItem>, cursor: usize) {
        let cursor = cursor.min(items.len().saturating_sub(1));
        match self.playback_target_queue_scope() {
            QueueScope::Local => {
                self.player_tab.set_items(items, cursor);
            }
            QueueScope::Remote => {
                let queue = self
                    .remote_player_tab
                    .as_mut()
                    .expect("direct remote playback queue requires remote queue");
                queue.set_items(items, cursor);
            }
        }
    }

    fn visible_queue_scope(&self) -> QueueScope {
        self.queue_scope_resolution().visible_scope()
    }

    fn displayed_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.visible_queue_scope())
    }

    fn displayed_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.visible_queue_scope())
    }

    fn playback_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.playback_target_queue_scope())
    }

    fn playback_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.playback_target_queue_scope())
    }

    fn merge_refreshed_queue(
        &mut self,
        scope: QueueScope,
        fetched_items: Vec<MediaItem>,
    ) -> RefreshMergeResult {
        let queue_len = self.queue_for_scope(scope).items.len();
        let sync_player_prunes =
            scope == self.playback_target_queue_scope() && !self.has_direct_remote_queue();
        let active_index = if scope == self.playback_target_queue_scope() {
            let st = self.player.status.lock().unwrap();
            (st.active && st.current_idx < queue_len).then_some(st.current_idx)
        } else {
            None
        };
        let (result, pre_refresh_indices) = {
            let queue = self.queue_for_scope_mut(scope);
            queue.sync_active_slot(active_index);
            let pre_refresh_indices = sync_player_prunes.then(|| {
                queue
                    .queue
                    .slots()
                    .iter()
                    .enumerate()
                    .map(|(index, slot)| (slot.slot_id, index))
                    .collect::<std::collections::HashMap<_, _>>()
            });
            (queue.merge_refresh(fetched_items), pre_refresh_indices)
        };
        if let Some(pre_refresh_indices) = pre_refresh_indices {
            let mut pruned_indices: Vec<_> = result
                .pruned_slots
                .iter()
                .filter_map(|slot_id| pre_refresh_indices.get(slot_id).copied())
                .collect();
            pruned_indices.sort_unstable_by(|left, right| right.cmp(left));
            for index in pruned_indices {
                self.player.send_command(PlayerCommand::QueueRemove(index));
            }
        }
        result
    }

    /// Whether the previous/next transport controls (playback-header mouse
    /// buttons and, implicitly, the `P`/`N` keys) are currently at a usable
    /// queue position: `(prev_available, next_available)`.
    ///
    /// A connected remote session exposes no queue-position/length fields in
    /// `SessionInfo` (see `mbv_core::api::SessionInfo`), so there is no way to
    /// tell whether it's at a queue boundary; both remain available there,
    /// mirroring `Command::PreviousTrack`/`Command::NextTrack`'s dispatch, which
    /// calls `session_jump_track` unconditionally for a connected session with
    /// no boundary check. Local playback uses `PlayerStatus::previous_idx`/
    /// `next_idx`, which already fold in `active` and `queue_len`.
    pub(super) fn transport_prev_next_available(&self) -> (bool, bool) {
        if self.connected_session_id.is_some() {
            return (true, true);
        }
        let st = self.player.status.lock().unwrap();
        (st.previous_idx().is_some(), st.next_idx().is_some())
    }

    /// Slot-keyed check of whether a completed/stopped queue slot should be
    /// consumed, given a player-reported completion (`consume`) and the
    /// type-specific consume flags. Returns `(should_consume, is_audio)` —
    /// callers that act on the removal need `is_audio` afterward to route to
    /// `on_video_consumed`/`on_audio_consumed`. Resolves the audio/video
    /// flag from the queue model by slot identity instead of raw index.
    fn should_consume_slot(&self, slot_id: QueueSlotId, consume: bool) -> (bool, bool) {
        let item = self.playback_queue().queue.slot(slot_id).map(|s| &s.item);
        let is_video = item.is_some_and(|i| i.is_video());
        let is_audio = item.is_some_and(|i| i.is_audio());
        let (consume_videos, consume_audio) = {
            let cfg = &self.client.lock().unwrap().config;
            (cfg.consume_videos, cfg.consume_audio)
        };
        let should_consume =
            consume && ((is_video && consume_videos) || (is_audio && consume_audio));
        log::info!(target: "consume", "consume check: slot_id={slot_id:?} consume={consume} \
            is_video={is_video} consume_videos={consume_videos} \
            is_audio={is_audio} consume_audio={consume_audio} => {should_consume}");
        (should_consume, is_audio)
    }

    /// Removes the given slot from the currently active playback queue by
    /// identity and, if something was actually removed, tells the player to
    /// drop the slot's current index from its own internal queue copy.
    /// Uses `consume_slot` rather than `remove_slot` so a slot that is
    /// currently marked active in the model (set via `set_active_slot`) can
    /// still be consumed; the active-confirmation gate on `remove_slot` only
    /// applies to explicit user-initiated removal. Returns the removed
    /// item's id, or `None` if the slot no longer exists.
    fn consume_slot_from_active_playback_queue(&mut self, slot_id: QueueSlotId) -> Option<String> {
        let idx = self.playback_queue().queue.slot_index(slot_id)?;
        let removed = match self.playback_queue_mut().queue.consume_slot(slot_id) {
            QueueMutationResult::Applied(slot) => slot,
            QueueMutationResult::NotFound => return None,
        };
        self.playback_queue_mut().sync_items_from_queue_model();
        self.player.send_command(PlayerCommand::QueueRemove(idx));
        Some(removed.item.id)
    }

    fn set_queue_scope(&mut self, scope: QueueScope) {
        self.queue_scope = if scope == QueueScope::Remote && self.has_direct_remote_queue() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        };
        self.power_queue_scroll = 0;
    }

    fn session_direct_endpoint(
        &self,
        sess: &mbv_core::api::SessionInfo,
    ) -> Option<mbv_core::remote_player::DaemonEndpoint> {
        if !sess.client.eq_ignore_ascii_case("mbv") {
            return None;
        }
        if let Some(port) = parse_mbv_direct_tcp_port(&sess.supported_commands) {
            if let Ok(ip) = sess.host.parse::<std::net::Ipv4Addr>() {
                return Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
                    std::net::SocketAddr::from((ip, port)),
                ));
            }
            log::warn!(
                target: "sessions",
                "mbv session {:?} advertised direct tcp port {} but host {:?} was not an IPv4 address",
                sess.device_name,
                port,
                sess.host
            );
        }
        let client = self.client.lock().unwrap();
        sess.device_name
            .eq_ignore_ascii_case(&client.device_name)
            .then_some(mbv_core::remote_player::DaemonEndpoint::Local)
    }

    fn switch_to_direct_remote(
        &mut self,
        sess: &mbv_core::api::SessionInfo,
        remote: mbv_core::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
    ) {
        let initial_items = remote.items.lock().unwrap().clone();
        let has_initial_items = !initial_items.is_empty();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.client.lock().unwrap().config.always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below:
        // MPRIS (if this session has a live registration) must follow this
        // new ctrl-owning target too, or it stays wired to whatever owned
        // playback before the takeover -- see #175.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
            };
            self.suspended_local = Some(suspended);
        } else {
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }

        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            crate::mpris::rebind(
                handle,
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(disconnected),
            );
        }

        self.remote_player_tab = Some(PlayerTab::new(initial_items, initial_cursor));
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now() - Duration::from_secs(60);
        self.remote_seek_pending_until = Instant::now() - Duration::from_secs(1);
        self.runtime_zero_since = None;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        if has_initial_items {
            self.set_queue_scope(QueueScope::Remote);
        } else {
            self.set_queue_scope(QueueScope::Local);
        }
        self.show_sessions = false;
        self.flash_status(format!("Connected directly to {}", sess.device_name));
    }

    fn restore_local_mode(&mut self, status: &str) {
        if !self.player.is_remote() {
            self.player.stop();
        }
        self.player.join();
        if let Some(suspended) = self.suspended_local.take() {
            self.player = suspended.player;
            self.player_rx = suspended.player_rx;
            self.ws_rx = suspended.ws_rx;
            self.ws_send_tx = suspended.ws_send_tx;
            // Mirror `switch_to_direct_remote`'s rebind (#175): the suspended
            // local `Player` is being restored as the ctrl-owning target
            // again, so MPRIS (if registered) must follow it back rather
            // than staying wired to the just-abandoned remote session.
            if let Some(handle) = &self.mpris {
                let sender = self.player.command_sender();
                crate::mpris::rebind(
                    handle,
                    self.player.status.clone(),
                    move |cmd| sender(cmd),
                    self.player.disconnected_flag(),
                );
            }
        }
        self.remote_player_tab = None;
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        self.set_queue_scope(QueueScope::Local);
        self.flash_status_high(status.to_string());
    }

    fn connect_to_session(&mut self, sess: &mbv_core::api::SessionInfo) {
        if !self.player.is_remote() {
            if let Some(endpoint) = self.session_direct_endpoint(sess) {
                let auth_token = self.client.lock().unwrap().token.clone();
                match mbv_core::remote_player::RemotePlayer::connect_endpoint(
                    &endpoint,
                    &auth_token,
                ) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_direct_remote(sess, remote, remote_rx);
                        return;
                    }
                    Err(e) => {
                        log::warn!(
                            target: "sessions",
                            "direct daemon upgrade failed for device={:?} endpoint={endpoint}: {}",
                            sess.device_name,
                            e
                        );
                    }
                }
            }
        }

        let id = sess.id.clone();
        let name = sess.device_name.clone();
        log::info!(
            target: "sessions",
            "connect: device={name:?} pos={}s runtime={}s",
            sess.position_s,
            sess.runtime_s
        );
        self.connected_session_id = Some(id);
        self.connected_session_state = Some(sess.clone());
        self.session_miss_count = 0;
        self.remote_pos_s = sess.position_s;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now();
        self.show_sessions = false;
        self.flash_status(format!("Connected to {name}"));
        self.spawn_sessions_load();
    }

    /// Query the terminal for its image protocol (sixel/kitty/iterm2/etc,
    /// via `Picker::from_query_stdio`, falling back to halfblocks), then
    /// apply `self.image_protocol`'s override if it names one of the known
    /// protocols. Shared by the startup init in `run` and the reattach
    /// -refresh handler (T5) below, which both need to (re)detect the
    /// attached terminal's capabilities the same way -- at startup, and
    /// again on every stay-alive reattach since a different terminal may
    /// now be attached.
    fn build_image_picker(&self) -> Picker {
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.image_protocol.clone();
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let proto = protocol_override
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "sixel" => Some(ProtocolType::Sixel),
                "kitty" => Some(ProtocolType::Kitty),
                "iterm2" => Some(ProtocolType::Iterm2),
                "halfblocks" => Some(ProtocolType::Halfblocks),
                _ => None, // "auto" or unknown: use picker's detected protocol
            });
        if let Some(proto) = proto {
            picker.set_protocol_type(proto);
        }
        picker
    }

    /// Whether the run loop should touch the terminal this tick. `false`
    /// only while a stay-alive session is detached (`self.attached ==
    /// false`) — see the `attached` field doc for why `Terminal::clear()`
    /// must never be called in that state (issue #156). Skipping renders
    /// while detached loses nothing: the next attach's reattach-refresh
    /// (`take_attach_pending()`) forces `force_clear` and a full repaint.
    fn wants_terminal_render(
        &self,
        had_events: bool,
        last_render: Instant,
        render_interval: Duration,
    ) -> bool {
        self.attached
            && (had_events || self.force_clear || last_render.elapsed() >= render_interval)
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode.
        self.image_picker = Some(self.build_image_picker());

        self.status = "Loading...".into();
        self.home_loading = true;
        terminal.draw(|f| self.render(f))?;

        {
            let c = self.client.lock().unwrap();
            c.register_capabilities();
        }

        match self.fetch_home() {
            Ok(()) => self.status.clear(),
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
        self.home_loading = false;
        self.restore_queue_state();
        terminal.draw(|f| self.render(f))?;

        // Installed unconditionally, even when this process is a stay-alive
        // inferior under a relay (`self.stay_alive_ctrl.is_some()`). That is
        // intentional, not incidental: the relay is the SIGHUP *firewall*
        // for the launching shell (it ignores SIGHUP and setsid()s so
        // closing the terminal that ran `mbv -a` can't kill it), but it is
        // NOT a firewall against the relay process itself dying. The relay
        // keeps its own extra fd on `pty.slave` open for its whole
        // lifetime specifically so the pty master never EOFs during normal
        // attach/detach/reattach cycling (`relay.rs::start_inferior`) --
        // under that normal operation this inferior's tty fds (0/1/2, its
        // controlling terminal per `become_pty_slave`'s setsid+TIOCSCTTY)
        // never see a real HUP condition, so this watchdog is a no-op.
        // But if the relay process itself crashes, every fd it held onto
        // the pty master closes with it, and the kernel delivers a real
        // SIGHUP to this inferior as the pty's session leader -- at that
        // point nothing else is left to supervise the player, so falling
        // back to this watchdog's normal "terminal is gone, stop and exit"
        // behavior is the correct fail-safe rather than something to gate
        // off for stay-alive.
        install_signal_handlers();
        start_quit_watchdog(self.player.quit_handle());

        // Stay-alive tray (T7, issue #156): the minimal head that makes an
        // alive session attended. Driven over the existing in-process
        // Player mpsc, not a ctrl socket -- ADR 0004's daemon-owned tray
        // (mbvd's own tray, `mbv_core::daemon::run_with_options`) is a
        // separate surface entirely. Only present when running as the
        // inferior under a relay; persists across detach/reattach since it
        // lives in the app, not the terminal-client. Kept alive for the
        // whole function (dropped only when `run` returns, i.e. on real quit).
        let _tray_handle = if self.stay_alive_ctrl.is_some() {
            let show_systray_icon = self.client.lock().unwrap().config.show_systray_icon;
            // `local_cmd_tx()` is `Some` here because a stay-alive session
            // (`stay_alive_ctrl.is_some()`) is only ever constructed via
            // `App::new`, which always builds `self.player` as
            // `PlayerProxy::local`; the event loop that can later swap it
            // to `PlayerProxy::remote` (`switch_to_direct_remote`,
            // triggered by connecting to another session) hasn't started
            // yet at this point in `run`. Capturing the `Arc` now, rather
            // than reading `self.player` from inside the tray later, keeps
            // tray transport controls targeting the in-process `Player`
            // even if the user connects to a remote session afterwards --
            // see `PlayerProxy::local_cmd_tx` for why that's safe.
            if show_systray_icon {
                if let Some(cmd_tx) = self.player.local_cmd_tx() {
                    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::sync_channel::<()>(1);
                    let handle =
                        crate::tray::spawn(shutdown_tx, self.player.status.clone(), cmd_tx);
                    // Tray Quit -> the same graceful-quit path as `mbv -q` /
                    // SIGTERM (T3): self-SIGTERM reuses all of QUIT_REQUESTED's
                    // existing save/stop/exit plumbing instead of duplicating it.
                    std::thread::spawn(move || {
                        if shutdown_rx.recv().is_ok() {
                            unsafe {
                                libc::raise(libc::SIGTERM);
                            }
                        }
                    });
                    handle
                } else {
                    log::warn!(
                        target: "tray",
                        "stay-alive session has no local player command channel; skipping tray"
                    );
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut last_render = Instant::now() - Duration::from_secs(2);

        'outer: loop {
            let mut had_events = false;
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(ev) = self.player_rx.try_recv() {
                had_events = true;
                if self.handle_player_event(ev) {
                    continue 'outer;
                }
            }

            while let Ok(action) = self.notif_action_rx.try_recv() {
                had_events = true;
                match action.as_str() {
                    "skip_intro:skip" => {
                        if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                            let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                            self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                            self.player.send_command(PlayerCommand::SkipIntroDismiss);
                            self.status.clear();
                        }
                    }
                    "next_up:play" => {
                        if let Some(item) = self.next_up_item.take() {
                            if let Some(idx) = self
                                .playback_queue()
                                .items
                                .iter()
                                .position(|i| i.id == item.id)
                            {
                                let label = item.playback_label();
                                self.player.send_command(PlayerCommand::JumpTo(idx));
                                self.playback_queue_mut().queue_cursor = idx;
                                self.flash_status(label);
                            }
                        }
                        self.status.clear();
                    }
                    "next_up:skip" => {
                        self.next_up_item = None;
                        self.player.send_command(PlayerCommand::NextUpDismiss);
                        self.status.clear();
                    }
                    "clear:yes" => {
                        if self.confirm_clear_queue {
                            self.confirm_clear_queue = false;
                            self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                        }
                    }
                    "__notif_failed__" => {
                        self.notif_failed = true;
                    }
                    _ => {} // dismissed, "ignore", "cancel", or empty: leave TUI prompt untouched
                }
            }

            while let Ok(ev) = self.lib_rx.try_recv() {
                had_events = true;
                self.handle_lib_event(ev);
            }

            let search_outcome = self.search.drain_results();
            if search_outcome.received > 0 {
                had_events = true;
                for error in search_outcome.errors {
                    self.flash_status_high(format!("Search error: {error}"));
                }
            }

            while let Ok(ev) = self.sessions_rx.try_recv() {
                had_events = true;
                match ev {
                    SessionEvent::Loaded(sessions) => {
                        let old_id = self
                            .sessions
                            .get(self.sessions_cursor)
                            .map(|s| s.id.clone());
                        self.sessions = sessions;
                        self.sessions_loading = false;
                        self.last_session_poll = Instant::now();
                        if let Some(id) = old_id {
                            if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
                                self.sessions_cursor = pos;
                            } else {
                                self.sessions_cursor = self
                                    .sessions_cursor
                                    .min(self.sessions.len().saturating_sub(1));
                                if !self.sessions.is_empty() {
                                    log::warn!(target: "sessions", "selected session gone; cursor clamped");
                                }
                            }
                        }
                        // Update connected session state; auto-disconnect if gone
                        if let Some(ref conn_id) = self.connected_session_id.clone() {
                            if let Some(s) = self.sessions.iter().find(|s| &s.id == conn_id) {
                                // Maintain a monotonic position estimate within a single video.
                                // Reset the anchor only when the playing item ID changes.
                                // Avoid keying on runtime or title — the API occasionally returns
                                // missing RunTimeTicks (as_i64 returns None → 0) or a slightly
                                // different name, which would spuriously reset the position anchor
                                // every poll and prevent smooth interpolation.
                                let now = Instant::now();
                                let prev_item_id = self
                                    .connected_session_state
                                    .as_ref()
                                    .and_then(|p| p.now_playing_item_id.as_deref());
                                let item_changed = s.now_playing_item_id.as_deref() != prev_item_id;
                                if item_changed {
                                    // Refresh the previous item so played/progress reflects
                                    // what the remote client reported to the server.
                                    if let Some(prev_id) = self
                                        .connected_session_state
                                        .as_ref()
                                        .and_then(|p| p.now_playing_item_id.clone())
                                    {
                                        let client = self.client.lock().unwrap().clone();
                                        let tx = self.sessions_tx.clone();
                                        std::thread::spawn(move || {
                                            if let Ok(mut items) = client
                                                .get_items_by_ids(std::slice::from_ref(&prev_id))
                                            {
                                                if let Some(fresh) = items.pop() {
                                                    let _ = tx.send(SessionEvent::ItemRefreshed(
                                                        prev_id,
                                                        Box::new(fresh),
                                                    ));
                                                }
                                            }
                                        });
                                    }
                                }
                                // Detect playback via API position advancing, not IsPaused.
                                // Some Emby clients always report IsPaused=true even while playing;
                                // the only reliable signal is that PositionTicks keeps moving.
                                let prev_api_pos = self
                                    .connected_session_state
                                    .as_ref()
                                    .map_or(0, |p| p.position_s);
                                if s.position_s > prev_api_pos {
                                    self.remote_api_pos_advanced_at = now;
                                }
                                // Extrapolate if API advanced recently (within 2× the ~11s report
                                // interval). After that window lapses we treat it as paused/stopped.
                                let api_active =
                                    self.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
                                let seek_pending = now < self.remote_seek_pending_until;
                                if seek_pending && !item_changed {
                                    // A seek was just dispatched; hold the optimistic position until
                                    // the API catches up. Once the API reports the new position (or
                                    // the window expires) we fall through to normal reconciliation.
                                    log::debug!(target: "sessions",
                                        "pos hold (seek pending): api={}s remote_pos_s={}s",
                                        s.position_s, self.remote_pos_s);
                                } else if item_changed {
                                    log::debug!(target: "sessions",
                                        "pos reset (item change): api_pos={}s → remote_pos_s {}s→{}s",
                                        s.position_s, self.remote_pos_s, s.position_s);
                                    self.remote_pos_s = s.position_s;
                                    self.remote_api_pos_advanced_at = now;
                                    self.remote_seek_pending_until = now - Duration::from_secs(1);
                                } else if api_active {
                                    let elapsed = self.remote_pos_at.elapsed().as_secs_f64();
                                    let extrapolated = Self::extrapolated_remote_position(
                                        self.remote_pos_s,
                                        self.remote_pos_at.elapsed(),
                                    );
                                    let new_pos = s.position_s.max(extrapolated);
                                    log::debug!(target: "sessions",
                                        "pos extrap: api={}s paused={} elapsed={:.2}s → remote_pos_s {}s→{}s",
                                        s.position_s, s.is_paused, elapsed, self.remote_pos_s, new_pos);
                                    self.remote_pos_s = new_pos;
                                } else {
                                    log::debug!(target: "sessions",
                                        "pos idle (no api advance in 22s): api_pos={}s → remote_pos_s {}s→{}s",
                                        s.position_s, self.remote_pos_s, s.position_s);
                                    self.remote_pos_s = s.position_s;
                                }
                                if !seek_pending || item_changed {
                                    self.remote_pos_at = now;
                                }
                                if item_changed {
                                    if let Some(new_idx) =
                                        s.now_playing_item_id.as_ref().and_then(|id| {
                                            self.player_tab.items.iter().position(|it| &it.id == id)
                                        })
                                    {
                                        self.player_tab.queue_cursor = new_idx;
                                    }
                                    self.runtime_zero_since = None;
                                }
                                self.connected_session_state = Some(s.clone());
                                self.session_miss_count = 0;
                                // Remote hasn't started playing yet — repoll sooner.
                                // Cap fast-poll at 30 s: if runtime stays 0 that long the
                                // remote client likely won't report it and we stop hammering.
                                if s.runtime_s == 0 {
                                    let since =
                                        self.runtime_zero_since.get_or_insert_with(Instant::now);
                                    if since.elapsed() < Duration::from_secs(30) {
                                        self.last_session_poll =
                                            Instant::now() - Duration::from_millis(500);
                                    }
                                } else {
                                    self.runtime_zero_since = None;
                                }
                            } else {
                                self.session_miss_count += 1;
                                if self.session_miss_count >= 3 {
                                    log::warn!(target: "sessions", "connected session gone; disconnecting");
                                    self.flash_status_high(
                                        "Remote session ended; disconnected".to_string(),
                                    );
                                    self.connected_session_id = None;
                                    self.connected_session_state = None;
                                    self.session_miss_count = 0;
                                    self.remote_pos_s = 0;
                                } else {
                                    log::warn!(target: "sessions", "connected session not in poll ({}/3); holding", self.session_miss_count);
                                }
                            }
                        }
                    }
                    SessionEvent::ItemRefreshed(item_id, fresh) => {
                        if let Some(slot) =
                            self.player_tab.items.iter_mut().find(|i| i.id == item_id)
                        {
                            *slot = *fresh;
                        }
                    }
                    SessionEvent::Error(e) => {
                        self.sessions_loading = false;
                        self.flash_status_high(format!("Sessions error: {e}"));
                    }
                }
            }

            while let Ok((item_id, img_opt)) = self.card_image_rx.try_recv() {
                had_events = true;
                self.card_image_loading.remove(&item_id);
                // A spawned fetch always sends exactly one result, so the in-flight
                // count is balanced here; free the slot and start any queued fetch.
                self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
                // Image was decoded off-thread; wrap it in a ThreadProtocol.
                // The expensive resize+encode (StatefulProtocol::resize_encode,
                // including kitty's base64 payload encode) now happens lazily
                // off the render thread on first draw instead of blocking it
                // — see `spawn_resize_worker` and the `ResizeResponse` drain
                // below (#164). This only builds the cheap unresized protocol.
                let state: Option<ratatui_image::thread::ThreadProtocol> =
                    img_opt.and_then(|dyn_img| {
                        let picker = self.image_picker.clone()?;
                        Some(self.new_thread_protocol(&picker, dyn_img, &item_id))
                    });
                if state.is_some() {
                    self.image_lru.retain(|k| k != &item_id);
                    self.image_lru.push_back(item_id.clone());
                    while self.image_lru.len() > self.image_cache_size {
                        if let Some(evict) = self.image_lru.pop_front() {
                            self.card_image_states.remove(&evict);
                        }
                    }
                }
                self.card_image_states.insert(item_id, state);
            }
            self.drain_image_fetches();

            // Apply completed off-thread resize+encode results (#164). A
            // response for an evicted/replaced/absent key is silently
            // dropped here; `update_resized_protocol` also guards on
            // ThreadProtocol's internal id, so a stale response racing a
            // newer resize request for the same (still-present) key is a
            // no-op too.
            while let Ok((key, response)) = self.resize_response_rx.try_recv() {
                had_events = true;
                if let Some(Some(state)) = self.card_image_states.get_mut(&key) {
                    state.update_resized_protocol(response);
                }
            }

            while let Ok(ev) = self.ws_rx.try_recv() {
                had_events = true;
                self.handle_ws_event(ev);
            }

            if let Some(at) = self.settings_save_at {
                if Instant::now() >= at {
                    let cfg = self.client.lock().unwrap().config.clone();
                    crate::config::save_config_with_ui(&cfg, &self.ui_config_snapshot());
                    self.settings_save_at = None;
                }
            }

            // Periodic session poll when connected to a remote session
            if self.connected_session_id.is_some()
                && self.last_session_poll.elapsed() >= Duration::from_secs(1)
                && !self.sessions_loading
            {
                self.spawn_sessions_load();
            }

            // Keep this session visible to other Emby clients
            if let Some(ref tx) = self.ws_send_tx {
                if self.last_keepalive.elapsed() >= Duration::from_secs(30) {
                    let _ = tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
                    self.last_keepalive = Instant::now();
                }
            }
            if self.ws_send_tx.is_some()
                && self.last_capabilities.elapsed() >= Duration::from_secs(600)
            {
                let client = self.client.lock().unwrap().clone();
                std::thread::spawn(move || client.register_capabilities());
                self.last_capabilities = Instant::now();
            }

            // Break instead of propagating I/O errors: when the terminal closes
            // (SIGHUP), poll/read fail because the fd is gone. Breaking lets the
            // post-loop cleanup run (player.stop + join) so the mpv window closes.
            let poll_ready = match event::poll(Duration::from_millis(50)) {
                Ok(r) => r,
                Err(_) => break,
            };
            if poll_ready {
                had_events = true;
                let ev = match event::read() {
                    Ok(ev) => ev,
                    Err(_) => break,
                };
                let is_home_card_nav = self.home_card_view && self.tab_idx == 0;
                match ev {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        let nav_code =
                            is_home_card_nav && matches!(key.code, KeyCode::Left | KeyCode::Right);
                        if self.handle_key(key) {
                            break;
                        }
                        // Drain queued duplicate nav keys to prevent scroll backlog.
                        if nav_code {
                            while event::poll(Duration::ZERO).unwrap_or(false) {
                                match event::read() {
                                    Ok(Event::Key(k))
                                        if k.kind == KeyEventKind::Press && k.code == key.code => {}
                                    Ok(other) => {
                                        match other {
                                            Event::Key(k) if k.kind == KeyEventKind::Press => {
                                                if self.handle_key(k) {
                                                    break 'outer;
                                                }
                                            }
                                            Event::Mouse(m) => self.handle_mouse(m),
                                            _ => {}
                                        }
                                        break;
                                    }
                                    Err(_) => break 'outer,
                                }
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        let nav_scroll = is_home_card_nav
                            && matches!(
                                mouse.kind,
                                crossterm::event::MouseEventKind::ScrollUp
                                    | crossterm::event::MouseEventKind::ScrollDown
                            )
                            && self
                                .layout
                                .home
                                .home_rect
                                .contains((mouse.column, mouse.row).into());
                        self.handle_mouse(mouse);
                        // Drain queued scroll events to prevent scroll backlog.
                        if nav_scroll {
                            while event::poll(Duration::ZERO).unwrap_or(false) {
                                match event::read() {
                                    Ok(Event::Mouse(m))
                                        if matches!(
                                            m.kind,
                                            crossterm::event::MouseEventKind::ScrollUp
                                                | crossterm::event::MouseEventKind::ScrollDown
                                        ) => {}
                                    Ok(other) => {
                                        match other {
                                            Event::Key(k) if k.kind == KeyEventKind::Press => {
                                                if self.handle_key(k) {
                                                    break 'outer;
                                                }
                                            }
                                            Event::Mouse(m) => self.handle_mouse(m),
                                            _ => {}
                                        }
                                        break;
                                    }
                                    Err(_) => break 'outer,
                                }
                            }
                        }
                    }
                    // Terminal resize (SIGWINCH via pty winsize, or a real
                    // terminal resize in bare mode): reflow + re-emit images
                    // only. Distinct from the `client attached` handler
                    // below — a resize must not re-detect capabilities or
                    // re-capture the mouse, and (unlike attach) only fires
                    // when the size actually changed. Also fixes the
                    // standalone (non-stay-alive) resize-corruption bug:
                    // ratatui's diffing alone left stale content on-screen
                    // after a size change, since raw image escape sequences
                    // aren't tracked in its buffer.
                    Event::Resize(_, _) => {
                        self.force_clear = true;
                        self.card_image_states.clear();
                        self.card_image_loading.clear();
                    }
                    _ => {}
                }
            }

            // `client attached` (T5 reattach-refresh, ADR 0005): the
            // superset of a resize, fired on EVERY attach regardless of
            // size — a stay-alive reattach in a different terminal (e.g.
            // kitty -> foot) must show correct art with no manual resize.
            // Re-run capability detection (DA1/XTGETTCAP round-trips
            // through the pty to whatever real terminal is now attached),
            // rebuild the image picker, re-capture the mouse (capture is
            // otherwise only ever set once, at `init_terminal`), and force
            // a full repaint with every visible image re-emitted.
            if stay_alive::StayAliveCtrl::take_attach_pending() {
                had_events = true;
                self.attached = true;
                // build_image_picker runs Picker::from_query_stdio() on the run-loop thread
                // and relies on being the sole stdin consumer at this moment; the kitty→foot
                // reattach case in the manual test matrix exercises this.
                self.image_picker = Some(self.build_image_picker());
                self.card_image_states.clear();
                self.card_image_loading.clear();
                let _ = crossterm::execute!(
                    terminal.backend_mut(),
                    crossterm::event::EnableMouseCapture
                );
                self.force_clear = true;
                log::info!(target: "stay_alive", "reattach-refresh: capabilities re-detected, images invalidated");
            }

            self.sync_volume_from_player();

            // Keep active playback/progress responsive at ~150 ms whenever a
            // local player is active or a remote session is connected; fall
            // back to 1 s when fully idle. Remote queue views need the fast
            // cadence even if the active item match is temporarily unavailable.
            let render_interval = {
                let playback = self.effective_playback_state();
                if playback.active || self.connected_session_state.is_some() {
                    Duration::from_millis(150)
                } else {
                    Duration::from_secs(1)
                }
            };
            if self.wants_terminal_render(had_events, last_render, render_interval) {
                if self.force_clear {
                    self.force_clear = false;
                    if let Err(e) = terminal.clear() {
                        log::error!(target: "run_loop", "terminal.clear() failed: {e:?} (kind={:?})", e.kind());
                        return Err(e.into());
                    }
                }
                if let Err(e) = terminal.draw(|f| self.render(f)) {
                    log::error!(target: "run_loop", "terminal.draw() failed: {e:?} (kind={:?})", e.kind());
                    return Err(e.into());
                }
                last_render = Instant::now();
            }
        }

        // Signal quit (SIGHUP/SIGTERM — terminal closed or process termination).
        // Stop player and join its thread so the mpv window closes and
        // report_stopped completes before we exit. The player thread closes
        // the window before making the HTTP call (see SingleSession/QueueSession
        // run()), so the window disappears promptly even if the HTTP call takes a
        // moment.
        if QUIT_REQUESTED.load(Ordering::Relaxed) {
            let (was_playing, current_idx, position_ticks, last_valid_pos) = {
                let st = self.player.status.lock().unwrap();
                (
                    st.active,
                    st.current_idx,
                    st.position_ticks,
                    st.last_valid_pos,
                )
            };
            log::info!(target: "player", "quit: was_playing={was_playing} idx={current_idx} position_ticks={position_ticks} last_valid_pos={last_valid_pos}");
            if was_playing && !self.has_direct_remote_queue() {
                if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                    if position_ticks > 0 && !item.is_audio() {
                        item.playback_position_ticks = position_ticks;
                    }
                    self.last_played_item_id = Some(item.id.clone());
                }
            }
            self.save_queue_state_no_clear();
            if !self.player.is_remote() {
                self.player.stop();
                self.player.join_or_timeout(Duration::from_secs(5));
            }
            let _ = restore_terminal(terminal);
            return Ok(());
        }

        // Leave the daemon's player running when the TUI disconnects; only
        // stop the player when we own it locally.
        let (was_playing, current_idx, last_valid_pos) = {
            let st = self.player.status.lock().unwrap();
            (st.active, st.current_idx, st.last_valid_pos)
        };
        if !self.player.is_remote() {
            self.player.stop();
        }
        self.player.join();
        // Update the playing item's position before saving — the PlayerEvent::Stopped
        // that carries this update is never processed after we break out of the event loop.
        // Use last_valid_pos (never zeroed during track transitions) rather than
        // position_ticks (transiently 0 when QueueSession advances to the next track).
        if was_playing && !self.has_direct_remote_queue() {
            if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                if last_valid_pos > 0 && !item.is_audio() {
                    item.playback_position_ticks = last_valid_pos;
                }
                self.last_played_item_id = Some(item.id.clone());
            }
        }
        self.save_queue_state_no_clear();
        let _ = restore_terminal(terminal); // ignore errors — terminal may be gone (SIGHUP)
        Ok(())
    }

    /// Mirror mpv's actual volume into `ui_volume` and persist it, so volume
    /// changes made inside the mpv window (not just via mbv's keys) are kept and
    /// restored on the next launch. Skipped while controlling a remote session
    /// (the remote owns its volume) and while temporarily muted (so a mute
    /// doesn't clobber the saved level with 0).
    fn sync_volume_from_player(&mut self) {
        if self.connected_session_id.is_some() {
            return;
        }
        if self.pre_mute_volume.is_some() {
            return;
        }
        let player_vol = {
            let s = self.player.status.lock().unwrap();
            if s.active {
                Some(s.volume.clamp(0, 200) as u8)
            } else {
                None
            }
        };
        if let Some(v) = player_vol {
            if v != self.ui_volume {
                self.ui_volume = v;
                self.save_prefs();
            }
        }
    }

    /// Handle a PlayerEvent received from the player thread.
    /// Returns true if the caller's event loop should `continue` (skip render for this tick).
    fn handle_player_event(&mut self, ev: PlayerEvent) -> bool {
        match ev {
            PlayerEvent::Stopped {
                idx,
                position_ticks,
                played,
                consume,
                progress_report_accepted,
                error,
            } => {
                log::info!(target: "player", "Stopped event: idx={idx} position_ticks={}s played={played} error={error:?}",
                    position_ticks / mbv_core::api::TICKS_PER_SECOND);
                if self.player.is_remote_disconnected() {
                    self.next_up_item = None;
                    self.skip_intro_end_ticks = None;
                    self.restore_local_mode("Daemon disconnected — returned to local mode");
                    self.refresh_after_stop();
                    return true;
                }
                let is_delete = self.pending_delete_idx.take() == Some(idx);
                let preserve_local_state = !self.has_direct_remote_queue();
                // Resolve the raw mpv index to a slot right away, against
                // the queue exactly as it stands now (syncing the shadow
                // first for callers — tests, mainly — that assign `items`
                // directly without building the model).
                self.playback_queue_mut()
                    .sync_queue_model_from_items_if_needed();
                let slot_id = self.playback_queue().resolve_slot_at(idx);
                match slot_id {
                    Some(slot_id) => {
                        if !is_delete {
                            let position = if played {
                                0
                            } else if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                                if position_ticks > 0 && !slot.item.is_audio() {
                                    position_ticks
                                } else {
                                    slot.item.playback_position_ticks
                                }
                            } else {
                                0
                            };
                            let queue = self.playback_queue_mut();
                            let _ = queue.queue.apply_progress(slot_id, position, played);
                            if progress_report_accepted {
                                let _ = queue.queue.mark_progress_sync_pending(slot_id);
                            }
                            queue.sync_items_from_queue_model();
                            if played {
                                log::info!(target: "player", "Stopped: marked played, position reset to 0");
                            } else if position_ticks > 0 {
                                log::info!(target: "player", "Stopped: saved position={}s", position_ticks / mbv_core::api::TICKS_PER_SECOND);
                            } else {
                                log::info!(target: "player", "Stopped: position not saved (position_ticks={position_ticks})");
                            }
                        }
                        if preserve_local_state {
                            if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                                self.last_played_item_id = Some(slot.item.id.clone());
                                self.last_played_completed = played;
                            }
                        }
                    }
                    None => {
                        log::warn!(target: "player", "Stopped: idx={idx} maps to no live slot; \
                            skipping progress update");
                    }
                }
                self.next_up_item = None;
                self.skip_intro_end_ticks = None;
                self.status.clear();
                if is_delete {
                    let allow_undo = !self.player.is_remote();
                    // This IS the confirmed stop-and-remove of the now-playing
                    // slot, so it must go through the model's confirmed-removal
                    // API — the gated `remove_slot` (used by `remove_slot_at`)
                    // now refuses the active slot, which TrackChanged marks
                    // active in real playback. `remove_active_slot_confirmed`
                    // removes by index lookup and also clears `active_slot_id`,
                    // and is safe even if the slot happens to be non-active.
                    let item = match slot_id {
                        Some(slot_id) => {
                            match self
                                .playback_queue_mut()
                                .queue
                                .remove_active_slot_confirmed(slot_id)
                            {
                                RemoveSlotResult::Removed(slot) => {
                                    self.playback_queue_mut().sync_items_from_queue_model();
                                    self.player.send_command(PlayerCommand::QueueRemove(idx));
                                    Some(slot.item)
                                }
                                RemoveSlotResult::RequiresActiveConfirmation(_)
                                | RemoveSlotResult::NotFound => None,
                            }
                        }
                        None => None,
                    };
                    if let Some(item) = item {
                        let queue = self.playback_queue_mut();
                        if queue.items.is_empty() {
                            queue.queue_cursor = 0;
                        } else {
                            queue.queue_cursor =
                                queue.queue_cursor.min(queue.items.len().saturating_sub(1));
                        }
                        if allow_undo {
                            self.queue_undo_stack
                                .push(UndoEntry::Remove(idx, Box::new(item)));
                        }
                    }
                } else {
                    let (should_consume, is_audio) = match slot_id {
                        Some(slot_id) => self.should_consume_slot(slot_id, consume),
                        None => (false, false),
                    };
                    if should_consume {
                        let slot_id = slot_id.expect("should_consume implies a resolved slot");
                        let removed_id = self.consume_slot_from_active_playback_queue(slot_id);
                        let queue = self.playback_queue_mut();
                        if queue.items.is_empty() {
                            queue.queue_cursor = 0;
                        } else {
                            queue.queue_cursor =
                                queue.queue_cursor.min(queue.items.len().saturating_sub(1));
                        }
                        log::info!(target: "consume", "Stopped-path: removed slot_id={slot_id:?} \
                            removed_id={removed_id:?}");
                        if removed_id.is_none() {
                            log::warn!(target: "consume", "Stopped-path: slot_id={slot_id:?} not \
                                found, removal SKIPPED");
                        }
                        if is_audio {
                            self.on_audio_consumed();
                        } else {
                            self.on_video_consumed();
                        }
                    }
                }
                self.playback_queue_mut().queue.clear_active_slot();
                self.refresh_after_stop();
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
            }
            PlayerEvent::TrackCompleted {
                idx,
                position_ticks,
                played,
                consume,
                progress_report_accepted,
            } => {
                // Resolve the raw mpv index to a slot right away, against the
                // queue exactly as it stands now — the shadow (`items`) may
                // still need building for tests/older callers that assign
                // `items` directly, so sync first.
                self.playback_queue_mut()
                    .sync_queue_model_from_items_if_needed();
                let Some(slot_id) = self.playback_queue().resolve_slot_at(idx) else {
                    log::warn!(target: "consume", "TrackCompleted: idx={idx} maps to no live slot; dropping");
                    return false;
                };
                let position = if played {
                    0
                } else if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                    // Only record meaningful progress (≥ 30 s) for video;
                    // audio and startup noise keep the prior value.
                    if position_ticks >= 300_000_000 && !slot.item.is_audio() {
                        position_ticks
                    } else {
                        slot.item.playback_position_ticks
                    }
                } else {
                    return false;
                };
                let queue = self.playback_queue_mut();
                let _ = queue.queue.apply_progress(slot_id, position, played);
                if progress_report_accepted {
                    let _ = queue.queue.mark_progress_sync_pending(slot_id);
                }
                queue.sync_items_from_queue_model();
                let (should_consume, is_audio) = self.should_consume_slot(slot_id, consume);
                if should_consume {
                    self.pending_queue_removal = Some((slot_id, is_audio));
                }
            }
            PlayerEvent::TrackChanged(idx) => {
                self.skip_intro_end_ticks = None;
                self.next_up_item = None;
                if self.status.starts_with("Next up:") {
                    self.status.clear();
                }
                // Resolve the incoming index to a slot *before* draining any
                // deferred consume: `idx` is the player's report from
                // before it was told (via the QueueRemove sent below) that
                // the completed slot was removed, so it still lines up with
                // the queue's current, pre-removal shape.
                self.playback_queue_mut()
                    .sync_queue_model_from_items_if_needed();
                let target_slot_id = self.playback_queue().resolve_slot_at(idx);

                if let Some((slot_id, was_audio)) = self.pending_queue_removal.take() {
                    let len_before = self.playback_queue().items.len();
                    let removed_id = self.consume_slot_from_active_playback_queue(slot_id);
                    let len_after = len_before - removed_id.is_some() as usize;
                    log::info!(target: "consume", "TrackChanged: consuming pending removal slot_id={slot_id:?} \
                        new_idx={idx} len_before={len_before} len_after={len_after} removed_id={removed_id:?}");
                    if removed_id.is_none() {
                        log::warn!(target: "consume", "TrackChanged: slot_id={slot_id:?} not found, \
                            removal SKIPPED");
                    }
                    if was_audio {
                        self.on_audio_consumed();
                    } else {
                        self.on_video_consumed();
                    }
                }

                // Activate the resolved slot by identity (order-independent,
                // unlike raw index arithmetic) and derive the display
                // cursor from its post-removal position — this stays
                // correct regardless of where the just-consumed slot sat
                // relative to `idx`.
                let adjusted = match target_slot_id {
                    Some(slot_id) => {
                        let _ = self.playback_queue_mut().queue.set_active_slot(slot_id);
                        self.playback_queue()
                            .queue
                            .slot_index(slot_id)
                            .unwrap_or(idx)
                    }
                    None => {
                        log::warn!(target: "player", "TrackChanged: idx={idx} maps to no live \
                            slot; skipping activation");
                        idx
                    }
                };
                self.player.status.lock().unwrap().current_idx = adjusted;
                self.playback_queue_mut().queue_cursor = adjusted;
                if !self.has_direct_remote_queue() {
                    if let Some(item) = self.playback_queue().items.get(adjusted) {
                        self.last_played_item_id = Some(item.id.clone());
                    }
                }
                if !self.has_direct_remote_queue() {
                    let queue = self.playback_queue();
                    log::info!(target: "consume", "TrackChanged: post-save queue len={} ids={:?}",
                        queue.items.len(), queue.items.iter().map(|i| &i.id).collect::<Vec<_>>());
                    self.save_queue_state();
                }
            }
            PlayerEvent::QueueNextUp { next_idx } => {
                if let Some(item) = self.playback_queue().items.get(next_idx).cloned() {
                    let item_id = item.id.clone();
                    let show_title = item.series_name.clone();
                    let ep_title = item.name.clone();
                    let artist = item.artist.clone();
                    let label = item.playback_label();
                    self.next_up_item = Some(item.clone());
                    let next_up_msg = format!("Next up: {} (Y/n)", label);
                    self.notify_with_actions(
                        &item.name,
                        "Next up?",
                        &[("next_up:play", "Play Now"), ("next_up:skip", "Skip")],
                    );
                    self.status = next_up_msg;
                    self.status_expires = None;
                    // Daemon sends NextUpShow to mpv directly; only send from local player.
                    if !self.player.is_remote() {
                        self.player.send_command(PlayerCommand::NextUpShow {
                            item_id,
                            show_title,
                            ep_title,
                            artist,
                        });
                    }
                }
            }
            PlayerEvent::NextUpThreshold { .. } => {
                // Series episodes now use play_queue; this only fires for movies
                // (always_play_next=false or non-series content). No action needed.
            }
            PlayerEvent::NextUpPlay => {
                log::warn!(target: "app", "next-up: play triggered");
                if let Some(item) = self.next_up_item.take() {
                    let label = item.playback_label();
                    if let Some(idx) = self
                        .playback_queue()
                        .items
                        .iter()
                        .position(|i| i.id == item.id)
                    {
                        self.player.send_command(PlayerCommand::JumpTo(idx));
                        self.playback_queue_mut().queue_cursor = idx;
                        self.flash_status(label);
                    } else {
                        log::warn!(target: "app", "next-up: item not in queue, cannot jump");
                    }
                } else {
                    log::warn!(target: "app", "next-up: NextUpPlay fired but next_up_item is None");
                }
            }
            PlayerEvent::QueueUpdated {
                items,
                cursor,
                source,
            } => {
                let cursor = if self.has_direct_remote_queue() {
                    self.pending_remote_move_cursor
                        .take()
                        .filter(|pending_cursor| *pending_cursor < items.len())
                        .unwrap_or(cursor)
                } else {
                    cursor
                };
                let queue = self.playback_queue_mut();
                queue.set_items(items, cursor);
                if !self.has_direct_remote_queue() {
                    self.queue_source = source;
                }
            }
            PlayerEvent::IntroStarted { intro_end_ticks } => {
                self.skip_intro_end_ticks = Some(intro_end_ticks);
                let playing_title = self
                    .playback_queue()
                    .items
                    .get(self.playback_queue().queue_cursor)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| "mbv".into());
                self.notify_with_actions(
                    &playing_title,
                    "Skip intro?",
                    &[("skip_intro:skip", "Skip"), ("skip_intro:ignore", "Ignore")],
                );
                self.status = "Skip intro? (Y/n)".into();
                self.status_expires = None;
            }
            PlayerEvent::IntroEnded => {
                if self.skip_intro_end_ticks.take().is_some() {
                    self.status.clear();
                }
            }
            PlayerEvent::SkipIntroPlay => {
                self.skip_intro_end_ticks = None;
                self.status.clear();
            }
            PlayerEvent::MpvQuit => {
                self.next_up_item = None;
                self.skip_intro_end_ticks = None;
                self.status.clear();
                self.refresh_after_stop();
            }
            PlayerEvent::CommandRejected(reason) => {
                self.pending_remote_move_cursor = None;
                self.flash_status(reason);
            }
            PlayerEvent::RemoteDisconnected(reason) => {
                self.restore_local_mode(&reason);
                self.refresh_after_stop();
                return true;
            }
            PlayerEvent::QueueDesynced(reason) => {
                self.flash_status(reason);
            }
        }
        false
    }
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, Box<dyn std::error::Error>>
{
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let _ = crossterm::execute!(
        stdout,
        crossterm::event::PushKeyboardEnhancementFlags(
            crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    );
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(
    mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    crossterm::terminal::disable_raw_mode()?;
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::event::PopKeyboardEnhancementFlags
    );
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::ui_util::{fmt_duration, item_text_and_style};
    use super::*;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use mbv_core::api::TICKS_PER_SECOND;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use unicode_width::UnicodeWidthStr;

    pub(crate) fn make_item(name: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: "id".into(),
            name: name.into(),
            item_type: item_type.into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    pub(crate) fn make_session(device_name: &str, client: &str) -> mbv_core::api::SessionInfo {
        mbv_core::api::SessionInfo {
            id: "sess-1".into(),
            device_name: device_name.into(),
            client: client.into(),
            user_name: "user".into(),
            host: "127.0.0.1".into(),
            supported_commands: Vec::new(),
            now_playing: None,
            now_playing_item_id: None,
            position_s: 0,
            runtime_s: 0,
            is_paused: false,
            volume: 100,
            sub_index: -1,
            audio_index: 1,
            muted: false,
            media_info: mbv_core::api::SessionMediaInfo::default(),
        }
    }

    // ── fmt_duration ─────────────────────────────────────────────────────────

    #[test]
    fn fmt_duration_zero() {
        assert_eq!(fmt_duration(0), "0:00");
    }

    #[test]
    fn fmt_duration_seconds_only() {
        assert_eq!(fmt_duration(45), "0:45");
    }

    #[test]
    fn fmt_duration_minutes_and_seconds() {
        assert_eq!(fmt_duration(90), "1:30");
        assert_eq!(fmt_duration(3599), "59:59");
    }

    #[test]
    fn fmt_duration_hours() {
        assert_eq!(fmt_duration(3600), "1:00:00");
        assert_eq!(fmt_duration(3661), "1:01:01");
        assert_eq!(fmt_duration(7384), "2:03:04");
    }

    // ── item_text_and_style ──────────────────────────────────────────────────

    #[test]
    fn item_text_plain_unwatched_movie() {
        let item = make_item("Inception", "Movie");
        let (text, style) = item_text_and_style(&item, false);
        assert_eq!(text, "Inception");
        assert_eq!(style.fg, Some(palette::WHITE));
    }

    #[test]
    fn item_text_played_movie_uses_default_color() {
        let mut item = make_item("Inception", "Movie");
        item.played = true;
        let (_, style) = item_text_and_style(&item, false);
        assert_eq!(style.fg, Some(palette::WHITE));
    }

    #[test]
    fn item_text_in_progress_shows_duration() {
        let mut item = make_item("Inception", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200; // 2 hours
        item.playback_position_ticks = TICKS_PER_SECOND * 3600; // 1 hour in → 50%
        let (text, style) = item_text_and_style(&item, false);
        assert!(text.contains("2h00m"), "expected duration in: {text}");
        assert!(
            !text.contains("50%"),
            "pct should be in span, not text: {text}"
        );
        assert_eq!(style.fg, Some(palette::WHITE));
    }

    #[test]
    fn item_text_played_but_in_progress_shows_duration() {
        let mut item = make_item("Inception", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200;
        item.playback_position_ticks = TICKS_PER_SECOND * 3600;
        item.played = true;
        let (text, _) = item_text_and_style(&item, false);
        assert!(text.contains("2h00m"), "expected duration in: {text}");
        assert!(
            !text.contains("50%"),
            "pct should be in span, not text: {text}"
        );
    }

    #[test]
    fn item_text_includes_duration() {
        let mut item = make_item("Movie", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 5400; // 90 min
        let (text, _) = item_text_and_style(&item, false);
        assert!(text.contains("1h30m"), "expected duration in: {text}");
    }

    #[test]
    fn item_text_series_shows_unplayed_count_not_green() {
        let mut item = make_item("Breaking Bad", "Series");
        item.is_folder = true;
        item.unplayed_item_count = 5;
        let (text, style) = item_text_and_style(&item, false);
        assert!(text.contains("[5]"), "expected count in: {text}");
        assert_eq!(style.fg, Some(palette::WHITE));
    }

    #[test]
    fn item_text_nav_folder_is_white() {
        let mut item = make_item("Folder", "Folder");
        item.is_folder = true;
        let (_, style) = item_text_and_style(&item, false);
        assert_eq!(style.fg, Some(palette::WHITE));
    }

    #[test]
    fn item_text_selected_clears_color() {
        let item = make_item("X", "Movie");
        let (_, style) = item_text_and_style(&item, true);
        assert_eq!(style.fg, None);
    }

    #[test]
    fn queue_restore_uses_saved_cursor_when_last_played_is_missing() {
        let items = make_items(3);
        let cursor = super::actions::queue_restore_cursor(&items, 2, None, false);
        assert_eq!(cursor, 2);
    }

    // ── test helpers ─────────────────────────────────────────────────────────

    pub(crate) fn make_items(n: usize) -> Vec<MediaItem> {
        (0..n)
            .map(|i| {
                let mut item = make_item(&format!("Item {i}"), "Movie");
                item.id = format!("id{i}");
                item
            })
            .collect()
    }

    pub(crate) fn make_audio_items(n: usize) -> Vec<MediaItem> {
        (0..n)
            .map(|i| {
                let mut item = make_item(&format!("Track {i}"), "Audio");
                item.id = format!("id{i}");
                item.media_type = "Audio".into();
                item
            })
            .collect()
    }

    /// Minimal App stub for logic-only tests.
    pub(crate) fn make_app_stub() -> App {
        use mbv_core::player::{PlayerProxy, PlayerStatus};
        use std::sync::{Arc, Mutex};

        let status = Arc::new(Mutex::new(PlayerStatus {
            volume_max: 100,
            ..Default::default()
        }));

        let (_, player_rx) = std::sync::mpsc::channel();
        let (_, ws_rx) = std::sync::mpsc::channel();
        let (lib_tx, lib_rx) = std::sync::mpsc::channel();
        let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
        // No worker thread spawned here: `image_picker` is always `None` in
        // this stub, so no `ThreadProtocol` is ever built and nothing sends
        // on `resize_register_tx`/reads `resize_response_rx`.
        let (resize_register_tx, _resize_register_rx) = std::sync::mpsc::channel();
        let (_resize_response_tx, resize_response_rx) = std::sync::mpsc::channel();
        let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
        let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
        let (search_tx, search_rx) = std::sync::mpsc::channel::<Result<Vec<MediaItem>, String>>();

        let player = PlayerProxy::stub(status.clone());

        use crate::config::Config;
        use mbv_core::api::EmbyClient;
        let client = EmbyClient::new(Config::default());

        App {
            _test_state_dir_guard: crate::config::TestStateDirGuard::new_if_unset(),
            client: Arc::new(Mutex::new(client)),
            player,
            mpris: None,
            player_rx,
            ws_rx,
            tab_idx: 0,
            hidden_libraries: Vec::new(),
            hidden_latest: Vec::new(),
            music_levels: Vec::new(),
            player_tab: PlayerTab::default(),
            remote_player_tab: None,
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
                power_home_cursor: 0,
                power_home_scroll: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            layout: layout::AppLayout {
                power: layout::LayoutPower {
                    detail_page_h: 5,
                    ..Default::default()
                },
                ..Default::default()
            },
            terminal_width: 80,
            terminal_height: 24,

            home_panel_section_offset: 0,
            home_cards_section_offset: 0,
            home_loading: false,
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: std::time::Instant::now(),
            last_drag_seek: std::time::Instant::now(),
            last_click_pos: (u16::MAX, u16::MAX),
            confirm_remove_idx: None,
            pending_delete_idx: None,
            pending_queue_removal: None,
            confirm_clear_queue: false,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
            pending_remote_move_cursor: None,
            skip_intro_end_ticks: None,
            next_up_item: None,
            queue_view: 0,
            queue_group: true,
            power_focus: PowerFocus::default(),
            power_left_tab: 0,
            power_left_width: POWER_LEFT_WIDTH_DEFAULT,
            power_left_tab_pending: 0,
            power_queue_scroll: 0,
            power_queue_relocated: false,
            home_card_view: false,
            last_played_item_id: None,
            last_played_completed: false,
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            card_image_tx,
            card_image_rx,
            resize_register_tx,
            resize_response_rx,
            image_picker: None,
            show_help: false,
            show_settings: false,
            settings_cursor: 0,
            settings_scroll: 0,
            settings_save_at: None,
            confirm_logout: false,
            multiselect_popup: None,
            help_scroll: 0,
            system_notifications: false,
            notif_failed: false,
            notif_action_tx,
            notif_action_rx,
            context_menu: None,
            lib_tx,
            lib_rx,
            search: SearchSubsystem::new(search_tx, search_rx),
            force_clear: false,
            tab_scroll: 0,
            ui_volume: 100,
            pre_mute_volume: None,
            mute_on: false,
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_scroll: 0,
            sessions_loading: false,
            show_sessions: false,
            playlists: Vec::new(),
            playlists_cursor: 0,
            playlists_scroll: 0,
            playlists_loading: false,
            show_playlists: false,
            playlists_open: None,
            playlists_open_items: Vec::new(),
            playlists_open_cursor: 0,
            playlists_open_scroll: 0,
            playlists_open_loading: false,
            queue_source: crate::config::QueueSource::Unknown,
            queue_dirty: false,
            pending_queue_action: None,
            show_save_playlist_modal: false,
            use_nerd_fonts: false,
            indicator_style: Default::default(),
            panel_mode: Default::default(),
            ws_send_tx: None,
            last_keepalive: Instant::now(),
            last_capabilities: Instant::now(),
            sessions_tx,
            sessions_rx,
            connected_session_id: None,
            connected_session_state: None,
            last_session_poll: std::time::Instant::now(),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: std::time::Instant::now(),
            remote_api_pos_advanced_at: std::time::Instant::now() - Duration::from_secs(60),
            remote_seek_pending_until: std::time::Instant::now() - Duration::from_secs(1),
            runtime_zero_since: None,
            suspended_local: None,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
            last_nav_at: Instant::now() - Duration::from_secs(1),
            album_year_cache: std::collections::HashMap::new(),
            album_year_loading: std::collections::HashSet::new(),
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_loading: std::collections::HashSet::new(),
            pending_album_artist_fetches: std::collections::VecDeque::new(),
            album_artist_fetches_active: 0,
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            image_cache_size: 50,
            image_protocol: None,
            image_protocol_enabled: false,
            confirm_rescan: false,
            queue_scope: QueueScope::Local,
            stay_alive_ctrl: None,
            attached: true,
        }
    }

    // ── wants_terminal_render (#156: detached stay-alive must not touch
    // the terminal — Terminal::clear() blocks on a cursor-position DSR
    // query nobody answers once the terminal-client has detached) ────────

    #[test]
    fn wants_terminal_render_true_when_attached_and_due() {
        let mut app = make_app_stub();
        app.attached = true;
        let stale = Instant::now() - Duration::from_secs(10);
        assert!(app.wants_terminal_render(false, stale, Duration::from_secs(1)));
    }

    #[test]
    fn wants_terminal_render_false_when_detached_even_with_events_and_force_clear() {
        let mut app = make_app_stub();
        app.attached = false;
        app.force_clear = true;
        let stale = Instant::now() - Duration::from_secs(10);
        // had_events, force_clear, and an elapsed render_interval would all
        // independently demand a render while attached -- none of them may
        // override `attached == false`, or the run loop calls
        // Terminal::clear()/draw() with nobody left to answer the pty.
        assert!(!app.wants_terminal_render(true, stale, Duration::from_secs(1)));
    }

    #[test]
    fn wants_terminal_render_false_when_detached_and_idle() {
        let app = make_app_stub();
        let mut app = app;
        app.attached = false;
        let recent = Instant::now();
        assert!(!app.wants_terminal_render(false, recent, Duration::from_secs(1)));
    }

    #[test]
    fn try_quit_bare_mode_does_not_touch_attached() {
        let mut app = make_app_stub();
        app.attached = true;
        // No `stay_alive_ctrl` -> bare mode -> `attached` is irrelevant and
        // must stay untouched (it's never consulted outside stay-alive).
        let _ = app.try_quit();
        assert!(app.attached);
    }

    #[test]
    fn try_quit_stay_alive_detach_clears_attached_and_notifies_relay() {
        let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut app = make_app_stub();
        app.attached = true;
        app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));

        let quit_loop_should_exit = app.try_quit();

        assert!(
            !quit_loop_should_exit,
            "stay-alive `q` must detach, never quit the run loop"
        );
        assert!(
            !app.attached,
            "detach must clear `attached` so the run loop skips terminal I/O \
             until the next reattach (#156)"
        );

        // And it must have actually told the relay to detach, not just
        // flipped local state.
        use std::io::Read;
        relay_end.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 32];
        let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
        assert_eq!(&buf[..n], b"DETACH\n");
    }

    fn left_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.render(f)).unwrap();

        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    // ── transport_prev_next_available (issue #112) ─────────────────────────
    // Drives whether playback transport is currently available at the queue
    // boundaries. The header uses the `next` half directly, while the `P`/`N`
    // keys still reuse both halves.

    #[test]
    fn transport_prev_next_unavailable_when_player_inactive() {
        let app = make_app_stub();
        assert!(!app.player.status.lock().unwrap().active);
        assert_eq!(app.transport_prev_next_available(), (false, false));
    }

    #[test]
    fn transport_prev_next_both_available_mid_queue() {
        let app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 3;
            st.current_idx = 1;
        }
        assert_eq!(app.transport_prev_next_available(), (true, true));
    }

    #[test]
    fn transport_prev_unavailable_on_first_item() {
        let app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 3;
            st.current_idx = 0;
        }
        assert_eq!(app.transport_prev_next_available(), (false, true));
    }

    #[test]
    fn transport_next_unavailable_on_last_item() {
        let app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 3;
            st.current_idx = 2;
        }
        assert_eq!(app.transport_prev_next_available(), (true, false));
    }

    #[test]
    fn transport_prev_next_both_unavailable_on_single_item_queue() {
        let app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 1;
            st.current_idx = 0;
        }
        assert_eq!(app.transport_prev_next_available(), (false, false));
    }

    #[test]
    fn transport_prev_next_both_available_for_connected_remote_session_regardless_of_local_status()
    {
        // SessionInfo (see mbv_core::api::SessionInfo) exposes no
        // queue-position/length fields, so there's no boundary to check for a
        // connected remote session. Local status here is deliberately set to
        // "last item" to prove it's ignored while a session is connected.
        let mut app = make_app_stub();
        app.connected_session_id = Some("session-1".into());
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 3;
            st.current_idx = 2;
        }
        assert_eq!(app.transport_prev_next_available(), (true, true));
    }

    fn make_remote_app_stub(local_items: Vec<MediaItem>, remote_items: Vec<MediaItem>) -> App {
        use crate::config::Config;
        use mbv_core::api::EmbyClient;

        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.player_tab.items = local_items;
        app.player_tab.queue_cursor = 0;
        app
    }

    fn make_remote_app_stub_with_cmd_rx(
        local_items: Vec<MediaItem>,
        remote_items: Vec<MediaItem>,
    ) -> (App, std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>) {
        use crate::config::Config;
        use mbv_core::api::EmbyClient;

        let (remote, player_rx, cmd_rx) =
            mbv_core::remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.player_tab.items = local_items;
        app.player_tab.queue_cursor = 0;
        (app, cmd_rx)
    }

    fn make_local_daemon_app_stub(remote_items: Vec<MediaItem>) -> App {
        use crate::config::Config;
        use mbv_core::api::EmbyClient;

        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, true)
    }

    #[test]
    fn local_daemon_bootstrap_adopts_saved_local_queue_and_source() {
        let items = make_items(2);
        let bootstrap = bootstrap_local_daemon_queue(
            Vec::new(),
            0,
            crate::config::QueueSource::Unknown,
            Some(crate::config::QueueState {
                source: crate::config::QueueSource::Playlist {
                    id: Some("pl1".into()),
                    name: "Saved".into(),
                },
                items,
                cursor: 1,
                last_played_item_id: None,
                last_played_completed: false,
                positions: Default::default(),
            }),
        );

        assert_eq!(bootstrap.player_tab.items.len(), 2);
        assert_eq!(bootstrap.player_tab.queue_cursor, 1);
        assert!(matches!(
            bootstrap.queue_source,
            crate::config::QueueSource::Playlist { ref name, .. } if name == "Saved"
        ));
        assert!(matches!(
            bootstrap.adopt_queue,
            Some((_, 1, crate::config::QueueSource::Playlist { ref name, .. })) if name == "Saved"
        ));
    }

    #[test]
    fn failed_local_daemon_adoption_routes_through_remote_disconnected() {
        // #119 task 5: a swallowed `adopt_queue()` send-failure must not
        // leave the app silently sitting on optimistic queue state the
        // daemon never received — it routes through the same handling a
        // live `PlayerEvent::RemoteDisconnected` uses.
        let mut app = make_local_daemon_app_stub(Vec::new());
        assert_eq!(app.queue_scope, QueueScope::Local);

        app.handle_failed_local_daemon_adoption();

        assert!(app.remote_player_tab.is_none());
        assert_eq!(app.queue_scope, QueueScope::Local);
        assert!(app.status.contains("daemon connection lost"));
    }

    #[test]
    fn remote_app_starts_on_local_queue_when_remote_queue_is_empty() {
        let app = make_remote_app_stub(make_items(2), Vec::new());

        assert_eq!(app.queue_scope, QueueScope::Local);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
        let app = make_remote_app_stub(make_items(2), make_items(1));

        assert_eq!(app.queue_scope, QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    }

    #[test]
    fn local_daemon_bootstrap_carries_saved_positions_for_enrichment() {
        let items = make_items(2);
        let mut positions = std::collections::HashMap::new();
        positions.insert(items[0].id.clone(), 999);
        let bootstrap = bootstrap_local_daemon_queue(
            Vec::new(),
            0,
            crate::config::QueueSource::Unknown,
            Some(crate::config::QueueState {
                source: crate::config::QueueSource::Album,
                items,
                cursor: 0,
                last_played_item_id: None,
                last_played_completed: false,
                positions: positions.clone(),
            }),
        );

        assert_eq!(bootstrap.positions, positions);
    }

    #[test]
    fn local_daemon_bootstrap_has_no_positions_without_saved_state() {
        let bootstrap =
            bootstrap_local_daemon_queue(Vec::new(), 0, crate::config::QueueSource::Unknown, None);

        assert!(bootstrap.positions.is_empty());
    }

    #[test]
    fn local_daemon_bootstrap_uses_restore_cursor_and_carries_last_played_state() {
        let items = make_items(3);
        let bootstrap = bootstrap_local_daemon_queue(
            Vec::new(),
            0,
            crate::config::QueueSource::Unknown,
            Some(crate::config::QueueState {
                source: crate::config::QueueSource::Album,
                items: items.clone(),
                cursor: 0,
                last_played_item_id: Some(items[1].id.clone()),
                last_played_completed: true,
                positions: Default::default(),
            }),
        );

        assert_eq!(bootstrap.player_tab.queue_cursor, 2);
        assert_eq!(
            bootstrap.last_played_item_id.as_deref(),
            Some(items[1].id.as_str())
        );
        assert!(bootstrap.last_played_completed);
    }

    #[test]
    fn local_daemon_bootstrap_prefers_existing_daemon_queue_state() {
        let remote_items = make_items(2);
        let bootstrap = bootstrap_local_daemon_queue(
            remote_items.clone(),
            0,
            crate::config::QueueSource::Playlist {
                id: Some("daemon".into()),
                name: "Daemon Queue".into(),
            },
            Some(crate::config::QueueState {
                source: crate::config::QueueSource::Playlist {
                    id: Some("local".into()),
                    name: "Local Saved".into(),
                },
                items: make_items(1),
                cursor: 0,
                last_played_item_id: None,
                last_played_completed: false,
                positions: Default::default(),
            }),
        );

        assert_eq!(bootstrap.player_tab.items.len(), 2);
        assert_eq!(bootstrap.player_tab.items[0].id, remote_items[0].id);
        assert!(matches!(
            bootstrap.queue_source,
            crate::config::QueueSource::Playlist { ref name, .. } if name == "Daemon Queue"
        ));
        assert!(bootstrap.adopt_queue.is_none());
    }

    #[test]
    fn session_direct_endpoint_prefers_advertised_tcp_port() {
        let app = make_app_stub();
        let mut sess = make_session("remote-host", "mbv");
        sess.host = "192.168.1.20".into();
        sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];
        assert_eq!(
            app.session_direct_endpoint(&sess),
            Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
                "192.168.1.20:47788".parse().unwrap()
            ))
        );
    }

    #[test]
    fn session_direct_endpoint_rejects_non_mbv_without_local_fallback() {
        let app = make_app_stub();
        let sess = make_session("other-host", "Emby");
        assert_eq!(app.session_direct_endpoint(&sess), None);
    }

    #[test]
    fn session_direct_endpoint_falls_back_to_local_socket_for_same_host_session() {
        let app = make_app_stub();
        let device_name = app.client.lock().unwrap().device_name.clone();
        let sess = make_session(&device_name, "mbv");
        assert_eq!(
            app.session_direct_endpoint(&sess),
            Some(mbv_core::remote_player::DaemonEndpoint::Local)
        );
    }

    #[test]
    fn remote_position_extrapolation_does_not_round_up_partial_seconds() {
        assert_eq!(
            App::extrapolated_remote_position(10, Duration::from_millis(1600)),
            11
        );
        assert_eq!(
            App::extrapolated_remote_position(10, Duration::from_secs(2)),
            12
        );
    }

    #[test]
    fn feed_home_video_group_view_requires_homevideos_and_feed_config() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                loading: true,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        assert!(!app.is_feed_home_video_group_view(0));

        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];
        assert!(app.is_feed_home_video_group_view(0));
    }

    #[test]
    fn feed_home_video_group_view_stays_enabled_with_cached_groups() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-youtube".into(),
                    title: "YouTube".into(),
                    items: vec![folder.clone()],
                    total_count: 1,
                    cursor: 1,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: None,
                },
                BrowseLevel {
                    parent_id: "folder-a".into(),
                    title: "Channel A".into(),
                    items: vec![video.clone()],
                    total_count: 1,
                    cursor: 0,
                    item_types: Some("Video".into()),
                    unplayed_only: true,
                    sort_by: "DateCreated".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: Some(vec![video.clone()]),
                },
            ],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video],
                }],
                loading: false,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];
        assert!(app.is_feed_home_video_group_view(0));
    }

    #[test]
    fn fetch_home_preserves_feed_home_video_state() {
        let mut app = make_app_stub();
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library: library.clone(),
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video.clone()],
                }],
                loading: false,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.rebuild_library_tabs_from_views(&[library]);

        assert_eq!(app.libs.len(), 1);
        assert!(app.is_feed_home_video_group_view(0));
        let feed = app.libs[0].feed_home_video.as_ref().unwrap();
        assert_eq!(feed.groups.len(), 1);
        assert_eq!(feed.groups[0].items.len(), 1);
        assert_eq!(feed.groups[0].items[0].id, "video-a1");
    }

    #[test]
    fn feed_home_video_root_does_not_auto_push_before_folder_pagination_completes() {
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![],
                total_count: 0,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                loading: true,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        let mut folders = Vec::new();
        for idx in 0..100 {
            let mut folder = make_item(&format!("Channel {idx}"), "Folder");
            folder.id = format!("folder-{idx}");
            folder.is_folder = true;
            folders.push(folder);
        }

        app.handle_lib_event(LibEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            level: BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: folders,
                total_count: 101,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            },
        });

        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(app.libs[0].nav_stack[0].items.len(), 100);
        assert_eq!(app.libs[0].nav_stack[0].total_count, 101);
        // Pagination must keep going even though the root cursor (0) is nowhere
        // near the loaded edge -- the feed-home-video root isn't scrolled by the
        // user, so it has to paginate to completion on its own or aggregation
        // (and therefore the grouped view) would never be able to start.
        assert!(
            app.libs[0].nav_stack[0].loading,
            "expected the next folder page to be fetched automatically"
        );
    }

    #[test]
    fn select_feed_folder_group_pushes_video_level_for_selected_folder() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        let mut first = make_item("Channel A", "Folder");
        first.id = "folder-a".into();
        first.is_folder = true;
        let mut second = make_item("Channel B", "Folder");
        second.id = "folder-b".into();
        second.is_folder = true;
        let mut second_video = make_item("B1", "Movie");
        second_video.id = "video-b1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![first.clone(), second.clone()],
                total_count: 2,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![second_video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder: second.clone(),
                    items: vec![second_video.clone()],
                }],
                loading: false,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.select_feed_folder_group(0, 1);
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(1)
        );
        assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
        assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");
    }

    #[test]
    fn select_feed_folder_group_zero_pushes_all_videos_level() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video.clone()],
                }],
                loading: false,
                selected_group: 1,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.select_feed_folder_group(0, 0);
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(0)
        );
        assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
        assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-a1");
    }

    #[test]
    fn select_feed_folder_group_uses_client_side_all_items_cache() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        let mut first = make_item("Channel A", "Folder");
        first.id = "folder-a".into();
        first.is_folder = true;
        first.path = "/videos/a".into();
        let mut second = make_item("Channel B", "Folder");
        second.id = "folder-b".into();
        second.is_folder = true;
        second.path = "/videos/b".into();

        let mut a_video = make_item("A1", "Movie");
        a_video.id = "video-a1".into();
        a_video.path = "/videos/a/one.mp4".into();
        let mut b_video = make_item("B1", "Movie");
        b_video.id = "video-b1".into();
        b_video.path = "/videos/b/one.mp4".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![first.clone(), second.clone()],
                total_count: 2,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![a_video.clone(), b_video.clone()],
                groups: vec![
                    FeedHomeVideoGroup {
                        folder: first.clone(),
                        items: vec![a_video.clone()],
                    },
                    FeedHomeVideoGroup {
                        folder: second.clone(),
                        items: vec![b_video.clone()],
                    },
                ],
                loading: false,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.select_feed_folder_group(0, 2);
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(2)
        );
        assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
        assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");

        app.go_back();
        app.select_feed_folder_group(0, 0);
        assert_eq!(app.feed_home_video_selected_items(0).len(), 2);
    }

    #[test]
    fn select_feed_folder_group_updates_feed_state_when_detail_level_exists() {
        let mut app = make_app_stub();
        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        let mut first = make_item("Channel A", "Folder");
        first.id = "folder-a".into();
        first.is_folder = true;
        let mut second = make_item("Channel B", "Folder");
        second.id = "folder-b".into();
        second.is_folder = true;

        let mut a_video = make_item("A1", "Movie");
        a_video.id = "video-a1".into();
        let mut b_video = make_item("B1", "Movie");
        b_video.id = "video-b1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![first.clone(), second.clone()],
                total_count: 2,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![a_video.clone(), b_video.clone()],
                groups: vec![
                    FeedHomeVideoGroup {
                        folder: first,
                        items: vec![a_video],
                    },
                    FeedHomeVideoGroup {
                        folder: second,
                        items: vec![b_video.clone()],
                    },
                ],
                loading: false,
                selected_group: 1,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: Some(b_video.clone()),
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.select_feed_folder_group(0, 2);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(2)
        );
        assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
        assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");
    }

    #[test]
    fn go_back_keeps_feed_home_video_group_view_intact() {
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.tab_idx = 2;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video],
                }],
                loading: false,
                selected_group: 1,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.go_back();
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(1)
        );
    }

    #[test]
    fn feed_home_video_root_filters_groups_from_all_video_paths() {
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![],
                total_count: 0,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                loading: true,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        let mut empty = make_item("Empty Channel", "Folder");
        empty.id = "folder-empty".into();
        empty.is_folder = true;
        empty.path = "/videos/empty".into();

        let mut active = make_item("Active Channel", "Folder");
        active.id = "folder-active".into();
        active.is_folder = true;
        active.path = "/videos/active".into();

        app.handle_lib_event(LibEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            level: BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![empty, active.clone()],
                total_count: 2,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            },
        });

        assert_eq!(app.libs[0].nav_stack.len(), 1);

        let mut video = make_item("Episode 1", "Movie");
        video.path = "/videos/active/ep1.mp4".into();

        app.handle_lib_event(LibEvent::FeedHomeVideoAggregated {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder: active.clone(),
                items: vec![video],
            }],
        });

        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.groups.len()),
            Some(1)
        );
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .and_then(|state| state.groups.first())
                .map(|group| group.folder.id.as_str()),
            Some("folder-active")
        );
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.all_items.len()),
            Some(1)
        );
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        app.ensure_feed_home_video_group_level(0);
        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
        assert_eq!(
            app.feed_home_video_selected_items(0)[0].path,
            "/videos/active/ep1.mp4"
        );
    }

    #[test]
    fn ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_groups() {
        // A stale selected group from a prior aggregation run with more groups
        // must clamp to the groups that actually exist now.
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;

        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let video = make_item("A1", "Movie");

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video],
                }],
                loading: false,
                selected_group: 5,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.ensure_feed_home_video_group_level(0);

        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(
            app.libs[0]
                .feed_home_video
                .as_ref()
                .map(|state| state.selected_group),
            Some(1)
        );
    }

    #[test]
    fn refresh_lib_targets_power_feed_selection() {
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.tab_idx = 1;
        app.power_left_tab = 1;
        app.power_focus = PowerFocus::Left;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video],
                }],
                loading: false,
                selected_group: 1,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.refresh_lib();

        assert!(app.libs[0].nav_stack[0].loading);
        assert!(app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.loading)
            .unwrap_or(false));
    }

    #[test]
    fn podcast_library_detects_collection_type() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        assert!(app.is_podcast_library(0));
    }

    #[test]
    fn podcast_library_detects_name_when_collection_type_missing() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        assert!(app.is_podcast_library(0));
    }

    #[test]
    fn podcast_folder_context_menu_uses_play_labels_and_item_state() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        let mut show = make_item("Show A", "Folder");
        show.id = "show-a".into();
        show.is_folder = true;
        show.unplayed_item_count = 0;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-podcasts".into(),
                title: "Podcasts".into(),
                items: vec![show],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu");
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert!(labels.contains(&"Mark Unplayed"));
        assert!(!labels.contains(&"Mark Played"));
        assert!(!labels.contains(&"Mark Watched"));
        assert!(!labels.contains(&"Mark Unwatched"));
    }

    #[test]
    fn podcast_folder_context_menu_shows_mark_played_when_unplayed_items_remain() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        let mut show = make_item("Show A", "Folder");
        show.id = "show-a".into();
        show.is_folder = true;
        show.unplayed_item_count = 3;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-podcasts".into(),
                title: "Podcasts".into(),
                items: vec![show],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        app.tab_idx = app.lib_tab_offset();

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu");
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert!(labels.contains(&"Mark Played"));
        assert!(!labels.contains(&"Mark Unplayed"));
    }

    #[test]
    fn power_view_podcast_context_menu_uses_left_pane_library_context() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        let mut show = make_item("Show A", "Folder");
        show.id = "show-a".into();
        show.is_folder = true;
        show.unplayed_item_count = 0;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-podcasts".into(),
                title: "Podcasts".into(),
                items: vec![show],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu");
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert!(labels.contains(&"Mark Unplayed"));
        assert!(!labels.contains(&"Mark Watched"));
        assert!(!labels.contains(&"Mark Unwatched"));
    }

    #[test]
    fn power_view_podcast_context_menu_offers_mark_all_played_for_selected_show() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        let mut show = make_item("Show A", "Folder");
        show.id = "show-a".into();
        show.is_folder = true;

        let mut first = make_item("Episode 1", "Audio");
        first.id = "ep-1".into();
        first.media_type = "Audio".into();
        let mut second = make_item("Episode 2", "Audio");
        second.id = "ep-2".into();
        second.media_type = "Audio".into();
        second.played = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-podcasts".into(),
                title: "Podcasts".into(),
                items: vec![show.clone()],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![first.clone(), second.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder: show,
                    items: vec![first.clone(), second],
                }],
                loading: false,
                selected_group: 1,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu");
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert!(labels.contains(&"────────"));
        assert!(labels.contains(&"Mark All Played"));
        assert!(labels.contains(&"Mark All Unplayed"));
        let sep_idx = labels
            .iter()
            .position(|label| *label == "────────")
            .unwrap();
        let all_played_idx = labels
            .iter()
            .position(|label| *label == "Mark All Played")
            .unwrap();
        let all_unplayed_idx = labels
            .iter()
            .position(|label| *label == "Mark All Unplayed")
            .unwrap();
        assert!(sep_idx < all_played_idx);
        assert!(all_played_idx < all_unplayed_idx);
        assert_eq!(sep_idx, labels.len() - 3);
        assert_eq!(all_played_idx, labels.len() - 2);
        assert_eq!(all_unplayed_idx, labels.len() - 1);
        assert!(menu.entries.iter().any(|entry| {
            matches!(
                entry.action.as_ref(),
                Some(ContextAction::MarkItemsPlayed(ids)) if ids == &vec!["ep-1".to_string()]
            )
        }));
        assert!(menu.entries.iter().any(|entry| {
            matches!(
                entry.action.as_ref(),
                Some(ContextAction::MarkItemsUnplayed(ids)) if ids == &vec!["ep-2".to_string()]
            )
        }));
    }

    #[test]
    fn power_view_podcast_context_menu_mark_all_played_uses_all_pill_selection() {
        let mut app = make_app_stub();
        let mut library = make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.collection_type = "podcasts".into();
        library.is_folder = true;

        let mut first_show = make_item("Show A", "Folder");
        first_show.id = "show-a".into();
        first_show.is_folder = true;
        let mut second_show = make_item("Show B", "Folder");
        second_show.id = "show-b".into();
        second_show.is_folder = true;

        let mut first = make_item("Episode 1", "Audio");
        first.id = "ep-1".into();
        first.media_type = "Audio".into();
        let mut second = make_item("Episode 2", "Audio");
        second.id = "ep-2".into();
        second.media_type = "Audio".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-podcasts".into(),
                title: "Podcasts".into(),
                items: vec![first_show.clone(), second_show.clone()],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![first.clone(), second.clone()],
                groups: vec![
                    FeedHomeVideoGroup {
                        folder: first_show,
                        items: vec![first.clone()],
                    },
                    FeedHomeVideoGroup {
                        folder: second_show,
                        items: vec![second.clone()],
                    },
                ],
                loading: false,
                selected_group: 0,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu");
        let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
        assert_eq!(labels[labels.len() - 3], "────────");
        assert_eq!(labels[labels.len() - 2], "Mark All Played");
        assert_eq!(labels[labels.len() - 1], "Mark All Unplayed");
        assert!(menu.entries.iter().any(|entry| {
            matches!(
                entry.action.as_ref(),
                Some(ContextAction::MarkItemsPlayed(ids))
                    if ids == &vec!["ep-1".to_string(), "ep-2".to_string()]
            )
        }));
        assert!(menu.entries.iter().any(|entry| {
            matches!(
                entry.action.as_ref(),
                Some(ContextAction::MarkItemsUnplayed(ids)) if ids.is_empty()
            )
        }));
    }

    #[test]
    fn refreshed_does_not_overwrite_feed_root_with_video_items() {
        let mut app = make_app_stub();
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".into();
        library.collection_type = "homevideos".into();
        library.is_folder = true;
        let mut folder = make_item("Channel A", "Folder");
        folder.id = "folder-a".into();
        folder.is_folder = true;
        let mut video = make_item("A1", "Movie");
        video.id = "video-a1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![video.clone()],
                groups: vec![FeedHomeVideoGroup {
                    folder,
                    items: vec![video.clone()],
                }],
                loading: false,
                ..FeedHomeVideoState::default()
            }),
            power_detail_item: None,
            power_detail_scroll: 0,

            album_track_focus: None,
        });

        app.handle_lib_event(LibEvent::Refreshed {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            item_types: Some("Video".into()),
            unplayed_only: true,
            items: vec![video],
            total_count: 1,
        });

        assert_eq!(app.libs[0].nav_stack.len(), 1);
        assert_eq!(app.libs[0].nav_stack[0].item_types, None);
        assert_eq!(app.libs[0].nav_stack[0].items.len(), 1);
        assert!(app.libs[0].nav_stack[0].items[0].is_folder);
        assert!(app.is_feed_home_video_group_view(0));
    }

    #[test]
    fn stale_remote_queue_scope_falls_back_to_local_when_not_in_direct_remote_mode() {
        let mut app = make_app_stub();
        app.remote_player_tab = Some(PlayerTab::new(make_items(2), 1));
        app.queue_scope = QueueScope::Remote;

        assert_eq!(app.visible_queue_scope(), QueueScope::Local);

        app.set_queue_scope(QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert_eq!(app.queue_scope, QueueScope::Local);
    }

    #[test]
    fn queue_scope_resolution_matrix_without_remote_queue() {
        let mut app = make_app_stub();
        app.queue_scope = QueueScope::Local;

        assert!(!app.has_direct_remote_queue());
        assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert!(app.local_queue_metadata_applies(QueueScope::Local));
        assert!(app.local_queue_metadata_applies(QueueScope::Remote));
    }

    #[test]
    fn queue_scope_resolution_matrix_stale_remote_scope_without_direct_remote() {
        let mut app = make_app_stub();
        app.remote_player_tab = Some(PlayerTab::new(make_items(2), 0));
        app.queue_scope = QueueScope::Remote;

        assert!(!app.has_direct_remote_queue());
        assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert!(app.local_queue_metadata_applies(QueueScope::Local));
        assert!(app.local_queue_metadata_applies(QueueScope::Remote));
    }

    #[test]
    fn queue_scope_resolution_matrix_direct_remote_displaying_local() {
        let local_items = make_items(1);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items, remote_items);
        app.queue_scope = QueueScope::Local;

        assert!(app.has_direct_remote_queue());
        assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert!(app.local_queue_metadata_applies(QueueScope::Local));
        assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
    }

    #[test]
    fn queue_scope_resolution_matrix_direct_remote_displaying_remote() {
        let local_items = make_items(1);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items, remote_items);
        app.queue_scope = QueueScope::Remote;

        assert!(app.has_direct_remote_queue());
        assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
        assert!(app.local_queue_metadata_applies(QueueScope::Local));
        assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
    }

    #[test]
    fn non_power_queue_scope_switch_via_keyboard_local() {
        let local_items = make_items(1);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items, remote_items);
        app.tab_idx = 1;
        app.queue_view = 0;
        app.queue_scope = QueueScope::Remote;

        assert!(app.has_direct_remote_queue());
        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.queue_scope, QueueScope::Local);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn non_power_queue_scope_switch_via_keyboard_remote() {
        let local_items = make_items(1);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items, remote_items);
        app.tab_idx = 1;
        app.queue_view = 0;
        app.queue_scope = QueueScope::Local;

        assert!(app.has_direct_remote_queue());
        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.queue_scope, QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    }

    #[test]
    fn power_queue_renders_scope_pills_and_hitboxes_for_direct_remote() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.set_queue_scope(QueueScope::Local);

        let rendered = render_app_to_string(&mut app, 90, 28);

        assert!(
            rendered.contains(" Local ") && rendered.contains(" Remote "),
            "expected power queue scope pills in rendered output:\n{rendered}"
        );
        assert!(
            rendered.contains(" Queue "),
            "expected queue pill:\n{rendered}"
        );
        assert!(app.layout.power.queue_scope_local_area.width >= " Local ".width() as u16);
        assert!(app.layout.power.queue_scope_remote_area.width >= " Remote ".width() as u16);
    }

    #[test]
    fn power_queue_scope_switch_via_keyboard_works_from_queue_focus() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Queue;
        app.set_queue_scope(QueueScope::Local);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn power_left_focus_brackets_do_not_switch_queue_scope() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.set_queue_scope(QueueScope::Local);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn power_queue_scope_switch_via_click_uses_rendered_hitboxes() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.set_queue_scope(QueueScope::Local);
        let _ = render_app_to_string(&mut app, 90, 28);

        let remote = app.layout.power.queue_scope_remote_area;
        app.handle_mouse(left_down(remote.x, remote.y));
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

        let local = app.layout.power.queue_scope_local_area;
        app.handle_mouse(left_down(local.x, local.y));
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn non_power_queue_scope_switch_via_click_uses_rendered_hitboxes() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = 0;
        app.set_queue_scope(QueueScope::Local);
        let _ = render_app_to_string(&mut app, 90, 24);

        let remote = app.layout.queue.scope_remote_area;
        app.handle_mouse(left_down(remote.x, remote.y));
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

        let local = app.layout.queue.scope_local_area;
        app.handle_mouse(left_down(local.x, local.y));
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn non_power_remote_queue_row_click_updates_remote_cursor() {
        let mut app = make_remote_app_stub(make_items(2), make_items(3));
        app.tab_idx = 1;
        app.queue_view = 0;
        app.set_queue_scope(QueueScope::Remote);
        let _ = render_app_to_string(&mut app, 90, 24);

        let row = app.layout.queue.inner.y + 1;
        app.handle_mouse(left_down(app.layout.queue.inner.x, row));

        assert_eq!(app.player_tab.queue_cursor, 0);
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    }

    #[test]
    fn power_scope_keys_are_ignored_outside_queue_tab() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 0;
        app.queue_view = QUEUE_VIEW_POWER;
        app.set_queue_scope(QueueScope::Local);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn power_view_shift_resize_grows_from_queue_focus_and_persists_pref() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Queue;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

        assert!(!handled);
        assert_eq!(app.status, "Power view width: 45 cols");
        let prefs: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
        )
        .expect("prefs json");
        assert_eq!(prefs["power_left_width"].as_u64(), Some(45));
    }

    #[test]
    fn power_view_shift_resize_is_ignored_outside_power_view() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.queue_view = 0;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

        assert!(!handled);
        assert!(app.status.is_empty());
        assert!(!crate::config::prefs_path().exists());
    }

    #[test]
    fn power_view_shift_resize_is_blocked_by_help_overlay() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.show_help = true;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

        assert!(!handled);
        assert!(app.show_help);
        assert!(app.status.is_empty());
        assert!(!crate::config::prefs_path().exists());
    }

    #[test]
    fn power_view_shift_resize_clamps_and_reports_minimum_and_maximum() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
        assert!(!handled);
        assert_eq!(app.status, "Power view width already at minimum (40 cols)");
        assert!(!crate::config::prefs_path().exists());

        assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
        assert_eq!(app.status, "Power view width: 45 cols");

        assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
        assert_eq!(app.status, "Power view width: 48 cols");

        assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
        assert_eq!(app.status, "Power view width already at maximum (48 cols)");

        let prefs: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
        )
        .expect("prefs json");
        assert_eq!(prefs["power_left_width"].as_u64(), Some(48));
    }

    #[test]
    fn power_view_render_normalizes_saved_left_width_and_updates_layout() {
        let _guard = crate::config::TestStateDirGuard::new();
        let prefs = serde_json::json!({
            "playlist_view": QUEUE_VIEW_POWER,
            "tab_idx": 1,
            "power_left_width": 70,
        });
        std::fs::write(
            crate::config::prefs_path(),
            serde_json::to_string(&prefs).expect("prefs json"),
        )
        .expect("write prefs");

        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;

        let _ = render_app_to_string(&mut app, 70, 28);

        assert_eq!(app.layout.power.queue_area.width, 42);
        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
        )
        .expect("prefs json");
        assert_eq!(saved["power_left_width"].as_u64(), Some(42));
    }

    #[test]
    fn power_view_render_uses_resized_width_on_next_frame() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;

        let _ = render_app_to_string(&mut app, 100, 28);
        assert_eq!(app.layout.power.queue_area.width, 40);

        assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
        assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));

        let _ = render_app_to_string(&mut app, 100, 28);
        assert_eq!(app.layout.power.queue_area.width, 50);
    }

    #[test]
    fn non_power_queue_scope_switch_ignored_without_direct_remote() {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.queue_view = 0;
        app.queue_scope = QueueScope::Local;

        assert!(!app.has_direct_remote_queue());
        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.queue_scope, QueueScope::Local);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.queue_scope, QueueScope::Local);
    }

    #[test]
    fn local_daemon_queue_has_no_scope_affordance_or_remote_switch() {
        let mut app = make_local_daemon_app_stub(make_items(2));
        app.tab_idx = 1;
        app.queue_view = 0;
        let rendered = render_app_to_string(&mut app, 90, 24);

        assert!(!app.has_direct_remote_queue());
        assert_eq!(
            app.layout.queue.scope_local_area,
            ratatui::layout::Rect::default()
        );
        assert_eq!(
            app.layout.queue.scope_remote_area,
            ratatui::layout::Rect::default()
        );
        assert!(
            !rendered.contains(" Local ") && !rendered.contains(" Remote "),
            "local-daemon queue should not render split-scope pills:\n{rendered}"
        );

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn attached_session_only_queue_has_no_scope_affordance_or_remote_switch() {
        let mut app = make_app_stub();
        app.connected_session_id = Some("session-1".into());
        app.connected_session_state = Some(make_session("remote-host", "Emby"));
        app.tab_idx = 1;
        app.queue_view = 0;
        let rendered = render_app_to_string(&mut app, 90, 24);

        assert!(!app.has_direct_remote_queue());
        assert_eq!(
            app.layout.queue.scope_local_area,
            ratatui::layout::Rect::default()
        );
        assert_eq!(
            app.layout.queue.scope_remote_area,
            ratatui::layout::Rect::default()
        );
        assert!(
            !rendered.contains(" Local ") && !rendered.contains(" Remote "),
            "attached-session queue should not render split-scope pills:\n{rendered}"
        );

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(!handled);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    }

    #[test]
    fn status_bar_row_is_always_present_and_holds_the_control_pill() {
        let mut app = make_app_stub();
        app.tab_idx = 0; // Home tab, nothing playing — the row must still appear.

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains('\u{2261}'),
            "expected the control pill's playlist glyph (≡) on the final screen row:\n{rendered}"
        );
        // The pill must no longer render inside the tab row (first line).
        let first_line = rendered.lines().next().unwrap();
        assert!(
            !first_line.contains('\u{2261}'),
            "control pill must have moved off the tab row:\n{first_line}"
        );
        // TABBAR_LEFT_RESERVE shrinks from 10 (pill + gap) to 2 (small margin)
        // now that the pill no longer lives in the tab row -- the first tab
        // label should start within a couple columns of the left edge, not
        // leave a 10-column dead zone where the pill used to be.
        let first_non_space = first_line.find(|c: char| c != ' ').unwrap_or(0);
        assert!(
            first_non_space <= 3,
            "expected the tab row's first tab to start near the left edge (col <= 3), got col {first_non_space}:\n{first_line}"
        );
    }

    #[test]
    fn direct_remote_play_items_keeps_local_queue_intact() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let replacement = make_items(4);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items);
        app.queue_source = crate::config::QueueSource::Album;

        app.execute_pending_queue_action(PendingQueueAction::PlayItems {
            items: replacement.clone(),
            start_idx: 2,
            source: crate::config::QueueSource::Shuffle,
        });

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(app.player_tab.queue_cursor, 0);
        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            replacement
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
        assert!(matches!(
            app.queue_source,
            crate::config::QueueSource::Album
        ));
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    }

    #[test]
    fn direct_remote_track_changes_do_not_clobber_local_last_played() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items);
        app.last_played_item_id = Some(local_items[1].id.clone());
        app.last_played_completed = true;

        app.handle_player_event(PlayerEvent::TrackChanged(2));

        assert_eq!(
            app.last_played_item_id.as_deref(),
            Some(local_items[1].id.as_str())
        );
        assert!(app.last_played_completed);
    }

    #[test]
    fn command_rejected_flashes_the_daemon_supplied_reason() {
        let mut app = make_app_stub();

        app.handle_player_event(PlayerEvent::CommandRejected(
            "Daemon is running in audio-only mode; can't play video items".to_string(),
        ));

        assert_eq!(
            app.status,
            "Daemon is running in audio-only mode; can't play video items"
        );
    }

    #[test]
    fn stopped_progress_updates_the_queue_model_not_just_the_shadow() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(2);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let slot_id = app.player_tab.queue.slots()[0].slot_id;

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 600_000_000,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: None,
        });

        let slot = app.player_tab.queue.slot(slot_id).unwrap();
        assert_eq!(
            slot.item.playback_position_ticks, 600_000_000,
            "progress must be applied to the queue model, not only the display shadow"
        );
    }

    #[test]
    fn stopped_with_accepted_report_marks_pending_sync_and_clears_active_slot() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(1);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let slot_id = app.player_tab.queue.slots()[0].slot_id;
        app.handle_player_event(PlayerEvent::TrackChanged(0));
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
        }

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 600_000_000,
            played: false,
            consume: false,
            progress_report_accepted: true,
            error: None,
        });

        let slot = app.player_tab.queue.slot(slot_id).unwrap();
        assert_eq!(
            slot.progress_state
                .pending_sync
                .as_ref()
                .map(|progress| progress.position_ticks),
            Some(600_000_000)
        );
        assert_eq!(app.player_tab.queue.active_slot_id(), None);
    }

    #[test]
    fn stopped_consume_removes_the_right_slot_occurrence() {
        // Duplicate item ids: two occurrences of the same underlying item.
        // Stopping+consuming the second occurrence must remove that slot
        // specifically — never the first, which happens to share an id.
        let mut app = make_app_stub();
        let mut items = make_items(3);
        items[0].id = "dup".into();
        items[2].id = "dup".into();
        app.player_tab.items = items;
        app.player_tab.sync_queue_model_from_items_if_needed();
        let first_dup = app.player_tab.queue.slots()[0].slot_id;
        let second_dup = app.player_tab.queue.slots()[2].slot_id;
        app.client.lock().unwrap().config.consume_videos = true;

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 2,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
            error: None,
        });

        assert!(app.player_tab.queue.slot(first_dup).is_some());
        assert!(app.player_tab.queue.slot(second_dup).is_none());
    }

    #[test]
    fn stopped_delete_removes_the_active_now_playing_slot() {
        // The confirmed "remove now-playing item and stop playback" flow:
        // pending_delete_idx marks the active slot for removal, then a Stopped
        // event drives it. Now that TrackChanged populates the model's
        // active_slot_id in real playback, the gated remove_slot path would
        // refuse the active slot — the confirmed delete must bypass that gate.
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.player_tab.items = make_items(3);
        app.player_tab.sync_queue_model_from_items_if_needed();
        // TrackChanged(0) activates slot 0, mirroring real playback where the
        // model's active_slot_id becomes Some before the delete.
        app.handle_player_event(PlayerEvent::TrackChanged(0));
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        app.pending_delete_idx = Some(0);

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: None,
        });

        assert_eq!(
            app.player_tab.items.len(),
            2,
            "the confirmed delete must remove the active now-playing slot"
        );
        assert_eq!(
            app.queue_undo_stack.len(),
            1,
            "delete must push an undo entry"
        );
    }

    #[test]
    fn stopped_path_consumes_the_last_audio_item_in_the_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        // When the last item in the queue finishes, the player thread sends a
        // Stopped event (not TrackCompleted/TrackChanged) since there's no next
        // track to advance to. consume_audio must still remove it, mirroring how
        // consume_videos already works for a video's Stopped-path removal.
        let items = make_audio_items(1);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.client.lock().unwrap().config.consume_audio = true;

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: true,
            progress_report_accepted: false,
            error: None,
        });

        assert!(
            app.player_tab.items.is_empty(),
            "the last audio item should be consumed via the Stopped-path when consume_audio is on"
        );
    }

    #[test]
    fn stopped_path_does_not_consume_audio_when_consume_audio_is_off() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_audio_items(1);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.client.lock().unwrap().config.consume_audio = false;

        app.handle_player_event(PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: true,
            progress_report_accepted: false,
            error: None,
        });

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "consume_audio is off, so the item must stay in the queue"
        );
    }

    #[test]
    fn track_completed_progress_follows_slot_after_earlier_removal() {
        // queue: [a, b, c]; a is removed (indices shift: b now at 0, c at 1),
        // then a completion event for the player's post-removal index of b
        // (0) arrives. Progress must land on slot b regardless of the churn.
        let mut app = make_app_stub();
        app.player_tab.items = make_items(3);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let slot_b = app.player_tab.queue.slots()[1].slot_id;
        let slot_a = app.player_tab.queue.slots()[0].slot_id;
        assert!(matches!(
            app.player_tab.queue.remove_slot(slot_a),
            RemoveSlotResult::Removed(_)
        ));
        app.player_tab.sync_items_from_queue_model();

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 600_000_000,
            played: false,
            consume: false,
            progress_report_accepted: false,
        });

        let slot = app.player_tab.queue.slot(slot_b).unwrap();
        assert_eq!(slot.item.playback_position_ticks, 600_000_000);
    }

    #[test]
    fn track_completed_for_removed_slot_does_not_mutate_queue() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(2);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let ids_before: Vec<_> = app
            .player_tab
            .queue
            .slots()
            .iter()
            .map(|s| s.slot_id)
            .collect();

        // index 5 does not exist
        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 5,
            position_ticks: 600_000_000,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });

        let ids_after: Vec<_> = app
            .player_tab
            .queue
            .slots()
            .iter()
            .map(|s| s.slot_id)
            .collect();
        assert_eq!(ids_before, ids_after);
        assert!(app.pending_queue_removal.is_none());
    }

    #[test]
    fn track_changed_activates_the_current_slot() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(3);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let slot_b = app.player_tab.queue.slots()[1].slot_id;

        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.queue.active_slot_id(),
            Some(slot_b),
            "TrackChanged must set the model's active slot by identity, not just move the raw cursor"
        );
    }

    #[test]
    fn track_changed_activates_slot_and_consumes_deferred_slot() {
        // [a, b, c]; complete+consume a (deferred), then TrackChanged to b.
        let mut app = make_app_stub();
        app.player_tab.items = make_items(3);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let slot_b = app.player_tab.queue.slots()[1].slot_id;
        app.client.lock().unwrap().config.consume_videos = true;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        assert!(app.pending_queue_removal.is_some());

        app.handle_player_event(PlayerEvent::TrackChanged(1)); // player reports b at old idx 1

        // a was consumed; queue is [b, c]; b is active.
        assert_eq!(app.player_tab.queue.slots().len(), 2);
        assert_eq!(app.player_tab.queue.active_slot_id(), Some(slot_b));
        assert!(app.pending_queue_removal.is_none());
    }

    #[test]
    fn consuming_a_video_without_autosave_marks_queue_dirty() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        };
        app.client.lock().unwrap().config.consume_videos = true;
        app.client.lock().unwrap().config.save_playlist_on_consume = false;

        // First item finishes playing and is consumed while advancing to the next track.
        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "consumed item should be removed from the local queue"
        );
        assert!(
            app.queue_dirty,
            "consuming an item changes the saved playlist's contents; without \
             save_playlist_on_consume the queue must be marked dirty so the user is still \
             prompted to save before quitting/replacing the queue"
        );
    }

    #[test]
    fn consuming_a_video_resyncs_the_players_own_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        // The player thread (QueueSession) keeps its own separate copy of the
        // items list, independent of `player_tab.items`. If consume only shrinks
        // the app-side queue and never tells the player, the player's internal
        // index space permanently diverges from the displayed queue after the
        // first consume — any later index-based command (Enter on a queue row,
        // JumpTo, next natural advance) then lands on the wrong item.
        let items = make_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.client.lock().unwrap().config.consume_videos = true;
        let cmd_rx = app.player.spy_on_commands();

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert!(
            matches!(
                cmd_rx.try_recv(),
                Ok(crate::player::PlayerCommand::QueueRemove(0))
            ),
            "consuming idx=0 must tell the player to remove idx=0 from its own \
             internal queue, keeping it in sync with the app-side queue"
        );
    }

    #[test]
    fn consuming_a_video_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        };
        app.client.lock().unwrap().config.consume_videos = true;
        app.client.lock().unwrap().config.save_playlist_on_consume = true;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "consumed item should be removed from the local queue"
        );
        assert!(
            !app.queue_dirty,
            "with save_playlist_on_consume enabled, consuming from a saved playlist should \
             trigger an immediate re-save to Emby (mirroring the manual save-playlist flow), \
             so the queue is no longer considered dirty"
        );
    }

    #[test]
    fn consuming_a_video_on_direct_remote_queue_does_not_touch_local_queue_or_dirty_flag() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items);
        // The local queue happens to be a saved playlist with autosave-on-consume enabled —
        // the trap scenario: before the scope gate, consuming on the *remote* queue would
        // still fire save_playlist_to_emby() and push the unrelated, unmodified local
        // playlist to Emby.
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        };
        app.client.lock().unwrap().config.consume_videos = true;
        app.client.lock().unwrap().config.save_playlist_on_consume = true;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.remote_player_tab.as_ref().unwrap().items.len(),
            1,
            "consumed item should still be removed from the remote queue"
        );
        assert_eq!(
            app.player_tab.items.len(),
            local_items.len(),
            "consume on a direct-remote queue must not touch the unrelated local playlist"
        );
        assert!(
            !app.queue_dirty,
            "consume on a direct-remote queue must not mark the local queue dirty or trigger \
             an autosave of the local playlist — the change happened on the remote queue"
        );
    }

    #[test]
    fn consuming_an_audio_item_without_autosave_marks_queue_dirty() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_audio_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        };
        app.client.lock().unwrap().config.consume_audio = true;
        app.client
            .lock()
            .unwrap()
            .config
            .save_playlist_on_consume_audio = false;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "consumed audio item should be removed from the local queue"
        );
        assert!(
            app.queue_dirty,
            "consuming an audio item changes the saved playlist's contents; without \
             save_playlist_on_consume_audio the queue must be marked dirty so the user is \
             still prompted to save before quitting/replacing the queue"
        );
    }

    #[test]
    fn consuming_an_audio_item_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_audio_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        };
        app.client.lock().unwrap().config.consume_audio = true;
        app.client
            .lock()
            .unwrap()
            .config
            .save_playlist_on_consume_audio = true;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "consumed audio item should be removed from the local queue"
        );
        assert!(
            !app.queue_dirty,
            "with save_playlist_on_consume_audio enabled, consuming from a saved playlist \
             should trigger an immediate re-save to Emby, so the queue is no longer dirty"
        );
    }

    #[test]
    fn consume_videos_flag_does_not_consume_audio_items() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_audio_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.client.lock().unwrap().config.consume_videos = true;
        app.client.lock().unwrap().config.consume_audio = false;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: false,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            2,
            "consume_videos must not remove an audio item; consume_audio is off"
        );
    }

    #[test]
    fn consume_audio_flag_does_not_consume_video_items() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(2);
        let mut app = make_app_stub();
        app.player_tab.items = items;
        app.client.lock().unwrap().config.consume_audio = true;
        app.client.lock().unwrap().config.consume_videos = false;

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 0,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        app.handle_player_event(PlayerEvent::TrackChanged(1));

        assert_eq!(
            app.player_tab.items.len(),
            2,
            "consume_audio must not remove a video item; consume_videos is off"
        );
    }

    #[test]
    fn alt_q_enqueues_from_home_view() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.tab_idx = 0;
        app.home.section = 0;
        app.home.continue_items = make_items(1);
        app.home.continue_cursor = 0;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

        assert!(!handled);
        assert_eq!(app.player_tab.items.len(), 1);
        assert_eq!(app.player_tab.items[0].id, "id0");
    }

    #[test]
    fn alt_q_appends_to_direct_remote_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items, remote_items.clone());
        app.tab_idx = 0;
        app.queue_scope = QueueScope::Remote;
        app.home.section = 0;
        app.home.continue_items = make_items(1);
        app.home.continue_cursor = 0;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

        assert!(!handled);
        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            remote_items
                .iter()
                .map(|i| i.id.as_str())
                .chain(std::iter::once("id0"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn clearing_local_queue_in_direct_remote_mode_leaves_remote_queue_intact() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items, remote_items.clone());
        app.set_queue_scope(QueueScope::Local);
        app.queue_source = crate::config::QueueSource::Album;
        app.queue_dirty = true;

        app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

        assert!(app.player_tab.items.is_empty());
        assert_eq!(app.player_tab.queue_cursor, 0);
        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            remote_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            app.queue_source,
            crate::config::QueueSource::Unknown
        ));
        assert!(!app.queue_dirty);
    }

    #[test]
    fn clearing_remote_queue_in_direct_remote_mode_leaves_local_queue_metadata_intact() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items);
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("playlist-1".into()),
            name: "Saved".into(),
        };
        app.queue_dirty = true;

        app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

        assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            app.queue_source,
            crate::config::QueueSource::Playlist { .. }
        ));
        assert!(app.queue_dirty);
    }

    #[test]
    fn removing_from_local_queue_in_direct_remote_mode_does_not_touch_remote_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(3);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        app.set_queue_scope(QueueScope::Local);

        app.remove_from_queue(1);

        assert_eq!(app.player_tab.items.len(), 2);
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![local_items[0].id.as_str(), local_items[2].id.as_str()]
        );
        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            remote_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(app.queue_dirty);
        assert_eq!(app.remote_queue_undo_stack.len(), 0);
    }

    #[test]
    fn removing_from_remote_queue_in_direct_remote_mode_does_not_touch_local_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());

        app.remove_from_queue(1);

        assert_eq!(app.remote_player_tab.as_ref().unwrap().items.len(), 2);
        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![remote_items[0].id.as_str(), remote_items[2].id.as_str()]
        );
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(!app.queue_dirty);
        assert_eq!(app.queue_undo_stack.len(), 0);
        assert_eq!(app.remote_queue_undo_stack.len(), 1);
    }

    #[test]
    fn clearing_remote_queue_does_not_prompt_to_save_local_playlist() {
        let mut app = make_remote_app_stub(make_items(2), make_items(3));
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("playlist-1".into()),
            name: "Saved".into(),
        };
        app.queue_dirty = true;

        app.replace_queue_or_prompt(PendingQueueAction::ClearQueue);

        assert!(!app.show_save_playlist_modal);
        assert!(app.pending_queue_action.is_none());
        assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
        assert!(app.queue_dirty);
    }

    #[test]
    fn removing_from_inactive_remote_queue_is_rejected() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items, remote_items.clone());
        app.player.status.lock().unwrap().active = false;

        app.remove_from_queue(1);

        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            remote_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(app.status, "Remote queue can only be edited while active");
    }

    #[test]
    fn context_menu_remove_targets_displayed_remote_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        app.tab_idx = 1;
        app.queue_view = 0;
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

        app.open_context_menu();

        let action = app
            .context_menu
            .as_ref()
            .expect("context menu")
            .entries
            .iter()
            .find_map(|entry| match entry.action.as_ref() {
                Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
                _ => None,
            })
            .expect("remove from queue action");
        assert_eq!(action, 2);

        app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

        let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
        assert_eq!(
            item_ids(&app.remote_player_tab.as_ref().unwrap().items),
            vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
        );
        assert_eq!(app.remote_queue_undo_stack.len(), 1);
    }

    #[test]
    fn stale_context_menu_remove_remote_queue_index_is_ignored() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        app.tab_idx = 1;
        app.queue_view = 0;
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

        app.open_context_menu();

        let action = app
            .context_menu
            .as_ref()
            .expect("context menu")
            .entries
            .iter()
            .find_map(|entry| match entry.action.as_ref() {
                Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
                _ => None,
            })
            .expect("remove from queue action");
        app.remote_player_tab.as_mut().unwrap().items.truncate(2);

        app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

        let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
        assert_eq!(
            item_ids(&app.remote_player_tab.as_ref().unwrap().items),
            vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
        assert!(app.remote_queue_undo_stack.is_empty());
    }

    #[test]
    fn move_queue_item_up_swaps_items_and_cursor_follows() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 1;

        app.move_queue_item_up();

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                items[1].id.as_str(),
                items[0].id.as_str(),
                items[2].id.as_str()
            ]
        );
        assert_eq!(app.player_tab.queue_cursor, 0);
        assert_eq!(app.queue_undo_stack.len(), 1);
    }

    #[test]
    fn move_queue_item_down_swaps_items_and_cursor_follows() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 1;

        app.move_queue_item_down();

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                items[0].id.as_str(),
                items[2].id.as_str(),
                items[1].id.as_str()
            ]
        );
        assert_eq!(app.player_tab.queue_cursor, 2);
        assert_eq!(app.queue_undo_stack.len(), 1);
    }

    #[test]
    fn move_queue_item_up_is_noop_at_start_of_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 0;

        app.move_queue_item_up();

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(app.player_tab.queue_cursor, 0);
        assert!(app.queue_undo_stack.is_empty());
    }

    #[test]
    fn move_queue_item_down_is_noop_at_end_of_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 2;

        app.move_queue_item_down();

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(app.player_tab.queue_cursor, 2);
        assert!(app.queue_undo_stack.is_empty());
    }

    #[test]
    fn undo_reverses_a_move_and_cursor_follows_back() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 1;

        app.move_queue_item_up();
        assert_eq!(app.player_tab.queue_cursor, 0);

        app.undo_last_queue_edit(QueueScope::Local);

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(app.player_tab.queue_cursor, 1);
        assert!(app.queue_undo_stack.is_empty());
    }

    #[test]
    fn undo_of_move_does_not_disturb_prior_removal_undo_history() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 0;

        // A removal, then a move -- undoing once should only reverse the move.
        app.remove_from_queue(0);
        app.player_tab.queue_cursor = 0;
        app.move_queue_item_down();
        assert_eq!(app.queue_undo_stack.len(), 2);

        app.undo_last_queue_edit(QueueScope::Local);

        assert_eq!(app.queue_undo_stack.len(), 1);
        assert!(matches!(
            app.queue_undo_stack.last(),
            Some(UndoEntry::Remove(0, _))
        ));
    }

    #[test]
    fn undo_of_move_is_refused_if_the_moved_item_is_no_longer_at_to() {
        let _guard = crate::config::TestStateDirGuard::new();
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 0;

        app.move_queue_item_down(); // items[0] now sits at index 1
        assert_eq!(app.queue_undo_stack.len(), 1);

        // Something untracked by this undo stack happens to the queue
        // afterwards (e.g. a natural consume) removing the item that's now
        // at index 1, so the undo entry's `to` position no longer holds the
        // item that was actually moved.
        app.player_tab.items.remove(1);

        app.undo_last_queue_edit(QueueScope::Local);

        // Refused rather than blindly swapping whatever now sits at 0/1.
        assert_eq!(app.status, "Can't undo move: queue changed since then");
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![items[1].id.as_str(), items[2].id.as_str()]
        );
    }

    #[test]
    fn undo_of_move_is_refused_when_duplicate_id_masks_changed_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut items = make_items(3);
        items[0].id = "duplicate".into();
        items[0].name = "First duplicate".into();
        items[0].playlist_item_id = "slot-a".into();
        items[1].id = "duplicate".into();
        items[1].name = "Second duplicate".into();
        items[1].playlist_item_id = "slot-b".into();
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 0;

        app.move_queue_item_down(); // First duplicate now sits at index 1.
        assert_eq!(app.queue_undo_stack.len(), 1);

        app.player_tab.items.remove(1);
        app.player_tab.items.insert(1, items[1].clone());

        app.undo_last_queue_edit(QueueScope::Local);

        assert_eq!(app.status, "Can't undo move: queue changed since then");
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Second duplicate", "Second duplicate", "Item 2"]
        );
    }

    #[test]
    fn resolve_slot_at_maps_index_to_slot_and_rejects_out_of_range() {
        let tab = PlayerTab::new(make_items(3), 0);
        let s0 = tab.queue.slots()[0].slot_id;
        let s2 = tab.queue.slots()[2].slot_id;
        assert_eq!(tab.resolve_slot_at(0), Some(s0));
        assert_eq!(tab.resolve_slot_at(2), Some(s2));
        assert_eq!(tab.resolve_slot_at(3), None);
    }

    #[test]
    fn queue_edit_preserves_updated_item_fields_after_shadow_model_was_built() {
        let mut app = make_app_stub();
        app.player_tab.set_items(make_items(2), 0);
        let _slot_id = app.player_tab.slot_id_at(0).unwrap();

        app.player_tab.items[0].playback_position_ticks = 42;
        app.player_tab.items[0].played = true;

        app.player_tab.append_item(make_item("new", "Movie"));

        assert_eq!(app.player_tab.items[0].playback_position_ticks, 42);
        assert!(app.player_tab.items[0].played);
    }

    #[test]
    fn move_queue_item_for_remote_scope_sends_move_command_and_preserves_local_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(3);
        let remote_items = make_items(3);
        let (mut app, cmd_rx) =
            make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

        app.move_queue_item_up();

        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                remote_items[1].id.as_str(),
                remote_items[0].id.as_str(),
                remote_items[2].id.as_str()
            ]
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(!app.queue_dirty);
        assert_eq!(app.queue_undo_stack.len(), 0);
        assert_eq!(app.remote_queue_undo_stack.len(), 1);
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(mbv_core::ctrl::CtrlCmd::PlayerCmd(
                mbv_core::ctrl::WireCommand::QueueMove(1, 0)
            ))
        ));
    }

    #[test]
    fn move_queue_item_for_inactive_remote_scope_is_rejected() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(3);
        let remote_items = make_items(3);
        let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;
        app.player.status.lock().unwrap().active = false;

        app.move_queue_item_up();

        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            remote_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
        assert_eq!(app.status, "Remote queue can only be edited while active");
        assert!(cmd_rx.try_recv().is_err());
    }

    #[test]
    fn remote_queue_update_reconciles_remote_queue_without_touching_local_queue() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        let updated_remote = vec![
            remote_items[2].clone(),
            remote_items[0].clone(),
            remote_items[1].clone(),
        ];

        app.handle_player_event(PlayerEvent::QueueUpdated {
            items: updated_remote.clone(),
            cursor: 2,
            source: crate::config::QueueSource::Remote,
        });

        assert_eq!(
            app.remote_player_tab
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            updated_remote
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn remote_queue_update_after_move_keeps_cursor_on_moved_item() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let (mut app, _cmd_rx) =
            make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

        app.move_queue_item_up();

        app.handle_player_event(PlayerEvent::QueueUpdated {
            items: vec![
                remote_items[1].clone(),
                remote_items[0].clone(),
                remote_items[2].clone(),
            ],
            cursor: 1,
            source: crate::config::QueueSource::Remote,
        });

        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn remote_queue_update_after_move_tracks_duplicate_item_by_position() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let mut remote_items = make_items(3);
        remote_items[1].id = remote_items[0].id.clone();
        let (mut app, _cmd_rx) =
            make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
        app.set_queue_scope(QueueScope::Remote);
        app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

        app.move_queue_item_down();

        app.handle_player_event(PlayerEvent::QueueUpdated {
            items: vec![
                remote_items[0].clone(),
                remote_items[2].clone(),
                remote_items[1].clone(),
            ],
            cursor: 0,
            source: crate::config::QueueSource::Remote,
        });

        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            local_items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn moving_now_playing_item_keeps_cursor_on_it() {
        let _guard = crate::config::TestStateDirGuard::new();
        // `PlayerProxy::stub` (used by `make_app_stub`) has no live cmd channel to
        // assert against, so this only covers the app-side item/cursor bookkeeping;
        // `player::tests` covers the mpv-side PlaylistMove handling directly.
        let items = make_items(3);
        let mut app = make_app_stub();
        app.player_tab.items = items.clone();
        app.player_tab.queue_cursor = 1;
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 1;
        }

        app.move_queue_item_down();

        assert_eq!(
            app.player_tab
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                items[0].id.as_str(),
                items[2].id.as_str(),
                items[1].id.as_str()
            ]
        );
        assert_eq!(app.player_tab.queue_cursor, 2);
    }

    #[test]
    fn remote_slot_state_is_off_for_local_only_app() {
        let app = make_app_stub();
        assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
        assert!(!app.can_disconnect_remote());
        assert_eq!(
            app.sessions_overlay_footer(),
            "[↵]conn [r]refresh [Esc]close"
        );
    }

    #[test]
    fn remote_slot_state_is_attached_session_when_connected_to_remote_session() {
        let mut app = make_app_stub();
        app.connected_session_id = Some("session-1".into());

        assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
        assert!(app.can_disconnect_remote());
        assert_eq!(
            app.sessions_overlay_footer(),
            "[↵]conn [d]disc [r]refresh [Esc]close"
        );
    }

    #[test]
    fn remote_slot_state_is_direct_remote_for_network_daemon_mode() {
        let app = make_remote_app_stub(make_items(2), make_items(3));

        assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
        assert!(app.can_disconnect_remote());
        assert_eq!(
            app.sessions_overlay_footer(),
            "[↵]conn [d]disc [r]refresh [Esc]close"
        );
    }

    #[test]
    fn direct_remote_connect_keeps_local_scope_when_remote_queue_is_empty() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(2);
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
        let sess = make_session("remote-host", "mbv");

        app.switch_to_direct_remote(&sess, remote, remote_rx);

        assert_eq!(app.queue_scope, QueueScope::Local);
        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
        assert_eq!(app.player_tab.items.len(), 2);
    }

    #[test]
    fn direct_remote_connect_switches_to_remote_scope_when_remote_queue_has_items() {
        let mut app = make_app_stub();
        app.player_tab.items = make_items(2);
        let remote_items = make_items(1);
        let (remote, remote_rx) =
            mbv_core::remote_player::RemotePlayer::stub(remote_items.clone(), 0);
        let sess = make_session("remote-host", "mbv");

        app.switch_to_direct_remote(&sess, remote, remote_rx);

        assert_eq!(app.queue_scope, QueueScope::Remote);
        assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
        assert_eq!(
            app.remote_player_tab.as_ref().unwrap().items[0].id,
            remote_items[0].id
        );
        assert_eq!(app.player_tab.items.len(), 2);
    }

    #[test]
    fn switch_to_direct_remote_rebinds_mpris_to_the_new_remote_status() {
        // #175: before `switch_to_direct_remote` called `mpris::rebind`,
        // MPRIS stayed wired to whatever `PlayerStatus` was live when the
        // D-Bus service was first registered (almost always the initial
        // local `Player`'s), so local desktop MPRIS never picked up a
        // remote daemon's playback after a mid-session "Direct Remote"
        // takeover -- exactly the bug this issue reports. This drives the
        // real `App` method (not just `mpris::rebind` in isolation) to
        // prove the wiring at the call site is actually in place.
        let mut app = make_app_stub();
        let local_status = app.player.status.clone();
        app.mpris = Some(crate::mpris::test_handle(
            local_status.clone(),
            |_| {},
            None,
        ));

        let remote_items = make_items(1);
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let remote_status = remote.status.clone();
        let sess = make_session("remote-host", "mbv");

        app.switch_to_direct_remote(&sess, remote, remote_rx);

        let handle = app.mpris.as_ref().expect("mpris handle still present");
        let bound_status = crate::mpris::test_status(handle);
        assert!(
            Arc::ptr_eq(&bound_status, &remote_status),
            "switch_to_direct_remote must rebind MPRIS to the new remote's status"
        );
        assert!(!Arc::ptr_eq(&bound_status, &local_status));
    }

    #[test]
    fn restore_local_mode_rebinds_mpris_back_to_the_suspended_local_status() {
        // #175 follow-through: after a Direct Remote takeover ends (however
        // it ends -- disconnect, user action, etc.), MPRIS must follow
        // playback back to the restored local `Player`, not stay wired to
        // the now-defunct remote session.
        let mut app = make_app_stub();
        let local_status = app.player.status.clone();
        app.mpris = Some(crate::mpris::test_handle(
            local_status.clone(),
            |_| {},
            None,
        ));

        let remote_items = make_items(1);
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let remote_status = remote.status.clone();
        let sess = make_session("remote-host", "mbv");
        app.switch_to_direct_remote(&sess, remote, remote_rx);

        app.restore_local_mode("test: ending direct remote session");

        let handle = app.mpris.as_ref().expect("mpris handle still present");
        let bound_status = crate::mpris::test_status(handle);
        assert!(
            Arc::ptr_eq(&bound_status, &local_status),
            "restore_local_mode must rebind MPRIS back to the restored local status"
        );
        assert!(!Arc::ptr_eq(&bound_status, &remote_status));
    }

    #[test]
    fn remote_slot_state_is_local_daemon_for_thin_client_mode() {
        let app = make_local_daemon_app_stub(make_items(3));

        assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
        assert!(!app.can_disconnect_remote());
        assert_eq!(
            app.sessions_overlay_footer(),
            "[↵]conn [r]refresh [Esc]close"
        );
    }

    #[test]
    fn attached_session_state_wins_over_local_daemon_indicator() {
        let mut app = make_local_daemon_app_stub(make_items(3));
        app.connected_session_id = Some("session-1".into());

        assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
        assert!(app.can_disconnect_remote());
    }

    #[test]
    fn disconnect_remote_does_not_exit_local_daemon_mode() {
        let mut app = make_local_daemon_app_stub(make_items(3));

        app.disconnect_remote();

        assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
        assert!(app.player.is_remote());
        assert!(!app.can_disconnect_remote());
        assert_eq!(app.status, "Local daemon mode stays connected");
    }

    #[test]
    fn disconnect_remote_clears_attached_remote_session() {
        let mut app = make_app_stub();
        app.connected_session_id = Some("session-1".into());
        app.connected_session_state = Some(make_session("remote-host", "Emby"));
        app.session_miss_count = 2;
        app.remote_pos_s = 120;

        app.disconnect_remote();

        assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
        assert!(app.connected_session_id.is_none());
        assert!(app.connected_session_state.is_none());
        assert_eq!(app.session_miss_count, 0);
        assert_eq!(app.remote_pos_s, 0);
        assert_eq!(app.status, "Disconnected from remote session");
    }

    #[test]
    fn displayed_queue_playback_state_stays_active_for_local_daemon_queue() {
        let app = make_local_daemon_app_stub(make_items(3));
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 2;
            status.position_ticks = 42;
            status.runtime_ticks = 84;
            status.paused = true;
        }

        assert_eq!(
            app.displayed_queue_playback_state(),
            PlaybackState {
                active: true,
                active_idx: 2,
                position_ticks: 42,
                runtime_ticks: 84,
                paused: true,
            }
        );
    }

    #[test]
    fn local_daemon_consume_adjusts_active_idx_after_removal_shift() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_local_daemon_app_stub(make_items(4));
        app.client.lock().unwrap().config.consume_videos = true;
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 1;
        }

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 1,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        {
            let mut status = app.player.status.lock().unwrap();
            // Thin-client path: the remote player updates status.current_idx
            // from the daemon's TrackChanged event before App handles the
            // pending consume removal, so App must correct the shifted index.
            status.current_idx = 2;
        }
        app.handle_player_event(PlayerEvent::TrackChanged(2));

        assert_eq!(app.player_tab.queue_cursor, 1);
        assert_eq!(
            app.displayed_queue_playback_state().active_idx,
            1,
            "after removing the completed item, the active index must shift to \
             the now-playing item's new slot instead of following the stale \
             pre-removal numeric index"
        );
    }

    #[test]
    fn direct_remote_consume_adjusts_active_idx_after_removal_shift() {
        let _guard = crate::config::TestStateDirGuard::new();
        let local_items = make_items(2);
        let remote_items = make_items(4);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        app.client.lock().unwrap().config.consume_videos = true;
        app.set_queue_scope(QueueScope::Remote);
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 1;
        }

        app.handle_player_event(PlayerEvent::TrackCompleted {
            idx: 1,
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        });
        {
            let mut status = app.player.status.lock().unwrap();
            // Network direct-remote path receives the same raw pre-removal
            // TrackChanged index from the daemon as the same thin-client
            // control path covered above.
            status.current_idx = 2;
        }
        app.handle_player_event(PlayerEvent::TrackChanged(2));

        let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(&app.player_tab.items).unwrap(),
            serde_json::to_value(&local_items).unwrap()
        );
        assert_eq!(app.player_tab.queue_cursor, 0);
        assert_eq!(
            item_ids(&app.remote_player_tab.as_ref().unwrap().items),
            vec![
                remote_items[0].id.clone(),
                remote_items[2].id.clone(),
                remote_items[3].id.clone(),
            ]
        );
        assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
        assert_eq!(
            app.displayed_queue_playback_state().active_idx,
            1,
            "after removing the completed remote item, the active index must \
             shift to the now-playing item's new remote-queue slot"
        );
    }

    #[test]
    fn displayed_queue_playback_state_is_inactive_for_non_playback_scope() {
        let mut app = make_remote_app_stub(make_items(2), make_items(3));
        app.connected_session_state = Some(make_session("remote-host", "Emby"));
        app.connected_session_state
            .as_mut()
            .unwrap()
            .now_playing_item_id = Some("id1".into());
        app.set_queue_scope(QueueScope::Local);

        assert_eq!(app.visible_queue_scope(), QueueScope::Local);
        assert_eq!(
            app.displayed_queue_playback_state(),
            PlaybackState::default()
        );
    }

    // ── home_section_len_cur ─────────────────────────────────────────────────

    #[test]
    fn home_section_len_cur_section_zero_uses_continue_items() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(5);
        app.home.continue_cursor = 3;
        assert_eq!(app.home_section_len_cur(0), (5, 3));
    }

    #[test]
    fn home_section_len_cur_section_one_uses_latest() {
        let mut app = make_app_stub();
        app.home.latest = vec![("Latest Movies".into(), "lib1".into(), make_items(7), 2)];
        assert_eq!(app.home_section_len_cur(1), (7, 2));
    }

    #[test]
    fn home_section_len_cur_out_of_bounds_returns_zero() {
        let app = make_app_stub();
        assert_eq!(app.home_section_len_cur(99), (0, 0));
    }

    // ── set_home_cursor ──────────────────────────────────────────────────────

    #[test]
    fn set_home_cursor_out_of_bounds_section_is_noop() {
        let mut app = make_app_stub();
        app.set_home_cursor(99, 3); // should not panic
    }

    // ── move_home_cursor ─────────────────────────────────────────────────────

    #[test]
    fn move_home_cursor_forward_within_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(5);
        app.home.continue_cursor = 0;
        app.move_home_cursor(1);
        assert_eq!(app.home.continue_cursor, 1);
        assert_eq!(app.home.section, 0);
    }

    #[test]
    fn move_home_cursor_forward_stops_at_end_with_adjacent_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(3);
        app.home.continue_cursor = 2; // at end of section 0
        app.home.latest = vec![("T".into(), "lib".into(), make_items(4), 0)];
        app.move_home_cursor(1);
        // stays at end, does not advance to section 1 or wrap
        assert_eq!(app.home.section, 0);
        assert_eq!(app.home.continue_cursor, 2);
    }

    #[test]
    fn move_home_cursor_forward_stops_at_end_of_last_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(3);
        app.home.continue_cursor = 2;
        app.move_home_cursor(1);
        assert_eq!(app.home.section, 0);
        assert_eq!(app.home.continue_cursor, 2);
    }

    #[test]
    fn move_home_cursor_backward_within_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(5);
        app.home.continue_cursor = 3;
        app.move_home_cursor(-1);
        assert_eq!(app.home.continue_cursor, 2);
        assert_eq!(app.home.section, 0);
    }

    #[test]
    fn move_home_cursor_backward_stops_at_start_with_prior_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(3);
        app.home.latest = vec![("T".into(), "lib".into(), make_items(4), 0)];
        app.home.section = 1;
        app.home.latest[0].3 = 0; // at start of section 1
        app.move_home_cursor(-1);
        // stays at start, does not go back to section 0 or wrap
        assert_eq!(app.home.section, 1);
        assert_eq!(app.home.latest[0].3, 0);
    }

    #[test]
    fn move_home_cursor_backward_stops_at_start_of_first_section() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(3);
        app.home.continue_cursor = 0;
        app.move_home_cursor(-1);
        assert_eq!(app.home.section, 0);
        assert_eq!(app.home.continue_cursor, 0);
    }

    // ── ensure_home_section_visible ──────────────────────────────────────────

    fn sections(n: usize) -> Vec<(String, String, Vec<MediaItem>, usize)> {
        (0..n)
            .map(|i| (format!("Sec {i}"), format!("lib{i}"), make_items(3), 0))
            .collect()
    }

    #[test]
    fn ensure_visible_all_sections_fit_no_scrolling_needed() {
        let mut app = make_app_stub();
        // terminal_height=24, chrome=3, panel_h=21; HOME_MIN_SECTION_H=6; 3*6=18<=21 => all visible
        app.terminal_height = 24;
        app.home.latest = sections(2); // 3 total sections
        app.home.section = 2;
        app.home_panel_section_offset = 0;
        app.ensure_home_section_visible();
        assert_eq!(app.home_panel_section_offset, 0);
    }

    #[test]
    fn ensure_visible_scrolls_offset_down_when_section_below_window() {
        let mut app = make_app_stub();
        // panel_h = 24-3 = 21; visible_rows = 21/6 = 3
        // 8 Latest => n_rows = 1 + (8+1)/2 = 5; max_offset = 2
        // section 8 (Latest[7]) => sec_row = 1 + 7/2 = 4
        // 4 >= 0 + 3 => offset = 4 + 1 - 3 = 2
        app.terminal_height = 24;
        app.home.latest = sections(8);
        app.home.section = 8; // last section, row 4
        app.home_panel_section_offset = 0;
        app.ensure_home_section_visible();
        assert_eq!(app.home_panel_section_offset, 2);
    }

    #[test]
    fn ensure_visible_scrolls_offset_up_when_section_above_window() {
        let mut app = make_app_stub();
        app.terminal_height = 24;
        app.home.latest = sections(4); // 5 total sections
        app.home.section = 0;
        app.home_panel_section_offset = 3; // selected is above window
        app.ensure_home_section_visible();
        assert_eq!(app.home_panel_section_offset, 0);
    }

    #[test]
    fn ensure_visible_clamps_offset_to_max() {
        let mut app = make_app_stub();
        // panel_h = 24 - 3 = 21; visible = 3; 3 total sections => max_offset = 0
        app.terminal_height = 24;
        app.home.latest = sections(2); // 3 total sections, all fit
        app.home.section = 1;
        app.home_panel_section_offset = 10; // way too high
        app.ensure_home_section_visible();
        assert_eq!(app.home_panel_section_offset, 0);
    }

    // ── cursor preservation during home refresh ──────────────────────────────

    #[test]
    fn home_refresh_preserves_cursor_by_lib_id() {
        // Simulate what init_home does: old_cursors keyed by lib_id.
        let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
            (
                "Latest Movies".into(),
                "lib-movies".into(),
                make_items(10),
                7,
            ),
            ("Latest TV".into(), "lib-tv".into(), make_items(5), 3),
        ];
        let old_cursors: std::collections::HashMap<String, usize> = old_latest
            .iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        // New fetch returns same libs but with fresh items.
        let new_items_movies = make_items(12);
        let new_items_tv = make_items(4);

        let cursor_movies = old_cursors
            .get("lib-movies")
            .copied()
            .unwrap_or(0)
            .min(new_items_movies.len().saturating_sub(1));
        let cursor_tv = old_cursors
            .get("lib-tv")
            .copied()
            .unwrap_or(0)
            .min(new_items_tv.len().saturating_sub(1));

        assert_eq!(cursor_movies, 7, "cursor preserved when within bounds");
        assert_eq!(cursor_tv, 3, "cursor preserved when within bounds");
    }

    #[test]
    fn home_refresh_clamps_cursor_when_new_list_is_shorter() {
        let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![(
            "Latest Movies".into(),
            "lib-movies".into(),
            make_items(10),
            9,
        )];
        let old_cursors: std::collections::HashMap<String, usize> = old_latest
            .iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        let new_items = make_items(5); // shorter than before
        let cursor = old_cursors
            .get("lib-movies")
            .copied()
            .unwrap_or(0)
            .min(new_items.len().saturating_sub(1));

        assert_eq!(cursor, 4, "cursor clamped to new last index");
    }

    #[test]
    fn home_refresh_cursor_defaults_zero_for_new_library() {
        let old_cursors: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let new_items = make_items(8);
        let cursor = old_cursors
            .get("brand-new-lib")
            .copied()
            .unwrap_or(0)
            .min(new_items.len().saturating_sub(1));
        assert_eq!(cursor, 0);
    }

    #[test]
    fn home_section_clamped_after_refresh_removes_sections() {
        let mut app = make_app_stub();
        app.home.latest = sections(4); // 5 total
        app.home.section = 4;

        // Simulate refresh that returns fewer sections.
        app.home.latest = sections(1); // now only 2 total
        let n = 1 + app.home.latest.len();
        if app.home.section >= n {
            app.home.section = n.saturating_sub(1);
        }
        assert_eq!(app.home.section, 1);
    }

    // ── status_bar (Task 2: session/connection label + unsaved marker) ───────

    #[test]
    fn status_bar_shows_direct_remote_label_next_to_pill() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 0;
        app.set_queue_scope(QueueScope::Remote);

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("REMOTE"),
            "expected a REMOTE label on the status bar for DirectRemote state:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_has_no_session_label_when_remote_slot_is_off() {
        let mut app = make_app_stub();
        app.tab_idx = 0;

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            !last_line.contains("REMOTE") && !last_line.contains("ATTACHED") && !last_line.contains("DAEMON"),
            "expected no session label when nothing is connected:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_shows_unsaved_marker_on_any_tab_when_queue_is_dirty() {
        let mut app = make_app_stub();
        app.tab_idx = 0; // Home tab, not the Queue tab -- unsaved state must still show.
        app.queue_dirty = true;

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("UNSAVED"),
            "expected an UNSAVED marker regardless of the active tab when the queue is dirty:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_drops_alive_before_unsaved_when_left_segment_overflows() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 0;
        app.set_queue_scope(QueueScope::Remote); // -> " REMOTE" label (7 cols)
        let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
        app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end)); // -> " ALIVE" (6 cols)
        app.queue_dirty = true; // -> " UNSAVED" (8 cols)

        // pill (9) + REMOTE (7) + ALIVE (6) + UNSAVED (8) = 30 cols; a 28-col
        // terminal leaves only 19 cols for the label -- enough for REMOTE +
        // UNSAVED (15) but not all three (21), so ALIVE must drop first.
        let rendered = render_app_to_string(&mut app, 28, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("REMOTE") && last_line.contains("UNSAVED"),
            "expected REMOTE and UNSAVED to survive the overflow:\n{last_line}"
        );
        assert!(
            !last_line.contains("ALIVE"),
            "expected ALIVE to be the first thing dropped on overflow:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_keeps_only_unsaved_when_left_segment_severely_overflows() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 0;
        app.set_queue_scope(QueueScope::Remote);
        let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
        app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));
        app.queue_dirty = true;

        // Only 11 cols available for the label (20 - 9) -- not enough for
        // REMOTE + UNSAVED (15), so REMOTE must drop too; UNSAVED (8) still fits
        // and is never dropped.
        let rendered = render_app_to_string(&mut app, 20, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("UNSAVED"),
            "UNSAVED must be protected even under severe overflow:\n{last_line}"
        );
        assert!(
            !last_line.contains("REMOTE") && !last_line.contains("ALIVE"),
            "expected REMOTE and ALIVE both dropped before UNSAVED is touched:\n{last_line}"
        );
    }
}
