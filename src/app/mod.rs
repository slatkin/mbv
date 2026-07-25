mod action;
mod actions;
mod bootstrap;
mod context_menu_actions;
mod feed_actions;
pub(crate) mod images;
mod input;
mod input_context_menu;
mod input_mouse;
mod input_resolver;
pub(crate) mod layout;
mod library_browse_actions;
mod library_route;
mod music_actions;
pub(crate) mod palette;
mod queue_actions;
mod queue_scope;
pub mod render;
mod resize;
mod search;
mod session_connect;
mod settings;
pub(crate) mod stay_alive;
mod types_browse;
mod types_context_menu;
mod types_events;
mod types_feed;
mod types_library_tab;
mod types_playback;
mod types_player_tab;
mod types_settings;
pub(crate) mod ui_util;

use self::bootstrap::bootstrap_local_daemon_queue;
use self::resize::{spawn_resize_worker, ResizeRegisterTx, ResizeResponseRx};
use self::search::SearchSubsystem;
use self::types_browse::{
    restore_library_position, AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel,
    LibSearch, SeriesDetail,
};
use self::types_context_menu::{
    ContextAction, ContextMenu, ContextMenuEntry, LibraryRoutePopup, LibraryRouteStage,
    MultiSelectKind, MultiSelectPopup,
};
use self::types_events::{LibEvent, SessionEvent};
use self::types_feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, SavePlaylistDialog, SavePlaylistStage,
};
use self::types_library_tab::LibraryTab;
use self::types_playback::{
    ArtistHeaderSelection, HomePane, LocalPlaybackTarget, PendingQueueAction, PlaybackState,
    PlaybackTarget, QueueScope, QueueScopeResolution, RemotePlaybackTarget, RemoteSlotState,
    SuspendedLocalSession, UndoEntry,
};
use self::types_player_tab::PlayerTab;
use self::types_settings::{PanelFocus, SettingKey, SETTING_SECTIONS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
// Set only by SIGHUP or stdin POLLHUP (terminal vanished). Never set by q/SIGTERM.
// The watchdog's forced exit arms only on this flag so clean q-quits are never raced.
static TERMINAL_GONE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
type DirectConnectFn = fn(
    &mbv_core::remote_player::DaemonEndpoint,
    &str,
) -> Result<
    (
        mbv_core::remote_player::RemotePlayer,
        mpsc::Receiver<PlayerEvent>,
    ),
    String,
>;

#[cfg(test)]
static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);
#[cfg(test)]
static DIRECT_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Separate from DIRECT_CONNECT_OVERRIDE above (Sessions-panel "Direct
// Remote" upgrade, keyed off a discovered SessionInfo): this is issue
// #222's lazy daemon-route connect primitive, targeting a statically
// configured DaemonEndpoint with no session discovery. Kept as its own
// override/lock pair so the two connect paths -- and the App state they
// eventually drive (`connected_session_id`/`direct_remote_label` vs. a
// future #223 `active_route`) -- stay independently testable and are
// never conflated, per #223's explicit "must not be conflated" rule.
#[cfg(test)]
static DAEMON_ROUTE_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);
#[cfg(test)]
static DAEMON_ROUTE_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

// Test seam for live-session-list lookups, mirroring
// DAEMON_ROUTE_CONNECT_OVERRIDE/_TEST_LOCK above: lets tests inject a fake
// session list without a real network call. Shared by
// `try_auto_reconnect`'s `DirectSession` lookup (#236) and the F2
// "Library Routes" device picker (`enter_device_stage`, #256).
#[cfg(test)]
type SessionsLoadFn =
    fn(&mbv_core::api::EmbyClient) -> Result<Vec<mbv_core::api::SessionInfo>, String>;
#[cfg(test)]
static SESSIONS_LOAD_OVERRIDE: Mutex<Option<SessionsLoadFn>> = Mutex::new(None);
#[cfg(test)]
static SESSIONS_LOAD_TEST_LOCK: Mutex<()> = Mutex::new(());

pub(super) const POWER_LEFT_WIDTH_DEFAULT: u16 = 40;
pub(super) const POWER_LEFT_WIDTH_STEP: u16 = 5;
/// Width reserved on the right of the tab bar for the volume badge (+ gap/arrow).
pub(super) const TABBAR_RIGHT_RESERVE: u16 = 17;
/// Left margin for the tab row. The control pill used to live here (hence
/// the old, larger reservation); it now renders in the status bar (see
/// `render_status_bar`) and the tabs are left-aligned flush with the left
/// edge instead.
pub(super) const TABBAR_LEFT_RESERVE: u16 = 0;

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
fn start_quit_watchdog(quit_handle: Option<mbv_core::player::QuitHandle>, quit_timeout: Duration) {
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
                    h.stop_for_shutdown(quit_timeout);
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

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};

use mbv_core::api::{EmbyClient, MediaItem};
use mbv_core::playback_queue::{QueueSlotId, RemoveSlotResult};
use mbv_core::player::{Player, PlayerCommand, PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;
use ratatui_image::picker::Picker;

const PAGE_SIZE: usize = 100;
const PREFETCH_AHEAD: usize = 25;

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
    home: HomePane,
    libs: Vec<LibraryTab>,
    player_tab: PlayerTab,
    remote_player_tab: Option<PlayerTab>,
    status: String,
    status_expires: Option<Instant>,
    /// `true` only for instances built via `App::new_remote` (the
    /// `--connect-daemon` / local-daemon-auto-detect thin-client launch
    /// path). Those instances never populate `active_route` or
    /// `connected_session_state` (those are set by runtime library-route
    /// switches / session attaches that only apply to `App::new` instances),
    /// so `teardown`'s auto-reconnect persistence (#236) must skip this flag
    /// entirely rather than compute (and save) a bogus `None` record that
    /// would wipe out a real record saved by a different `App::new` session.
    launched_as_remote: bool,
    hidden_libraries: Vec<String>,
    hidden_latest: Vec<String>,
    /// `Config.library_routes` at startup (#256). Values are resolved
    /// `tcp://host:port` endpoints, read directly with no live-session
    /// lookup -- see `mbv_core::config::resolve_library_route`.
    library_routes: std::collections::HashMap<String, String>,
    music_levels: Vec<String>,
    album_indexes: std::collections::HashMap<String, AlbumIndexState>,
    // Per-frame layout geometry from last render, used for mouse hit-testing.
    // See src/app/layout.rs for the grouping rationale.
    layout: layout::AppLayout,
    terminal_width: u16,
    terminal_height: u16,

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
    last_space_press: Option<Instant>,
    last_esc_press: Option<Instant>,
    confirm_remove_idx: Option<usize>, // playlist index pending removal confirmation
    pending_delete_idx: Option<usize>, // deferred removal of now-playing item after Stopped event
    pending_queue_removal: Option<(QueueSlotId, bool)>, // deferred removal (slot, is_audio) after TrackChanged index-shifts
    confirm_clear_queue: bool,
    queue_undo_stack: Vec<UndoEntry>,
    remote_queue_undo_stack: Vec<UndoEntry>,
    pending_remote_move_cursor: Option<usize>,
    skip_intro_end_ticks: Option<i64>,
    next_up_item: Option<MediaItem>,
    // Power-view UI scalars. NOTE: this is NOT the whole of Power's state -- Power also
    // reuses shared self.libs.
    panel_focus: PanelFocus,
    library_tab: usize, // 0 = Home/CW, 1..=libs.len() = library index
    queue_column_width: u16,
    queue_column_collapsed: bool,
    library_tab_pending: usize, // restored from prefs; applied once libs have loaded
    queue_scroll: usize,
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
    library_routes_popup: Option<LibraryRoutePopup>,
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
    ws_send_tx: Option<mbv_core::ws::WsSender>,
    last_keepalive: Instant,
    last_capabilities: Instant,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
    connected_session_id: Option<String>,
    connected_session_state: Option<mbv_core::api::SessionInfo>,
    direct_remote_connected: bool,
    direct_remote_label: Option<String>,
    last_session_poll: Instant,
    session_miss_count: u8, // consecutive polls that didn't find the connected session
    remote_pos_s: i64,      // monotonic position estimate for the connected remote
    remote_pos_at: Instant, // when remote_pos_s was last anchored
    remote_api_pos_advanced_at: Instant, // last time the API position actually moved forward
    remote_seek_pending_until: Instant, // suppress poll pos-reconcile after a seek
    runtime_zero_since: Option<Instant>, // when runtime_s first became 0 for the current item (fast-poll cap)
    suspended_local: Option<SuspendedLocalSession>,
    /// The library route currently driving playback, if any (#223):
    /// `Some(name)` holds the lowercased library name whose configured
    /// daemon is the active player target. `None` means local playback,
    /// or a Sessions-panel direct remote (`connected_session_id` /
    /// `direct_remote_label`) -- a separate concept, never conflated with
    /// this one. Fixed for the life of the current queue: a *new* queue
    /// re-evaluates it (see `apply_route_for_playback`), but enqueuing
    /// into the existing queue must match it or be rejected (see
    /// `enqueue_route_conflict`).
    active_route: Option<String>,
    /// Per-item cache of ancestor-lookup library-route resolution for
    /// cross-library aggregate views (Continue Watching/Next Up,
    /// Favorites), keyed by item id. `Some(name)` = resolved to that
    /// library (lowercased); `None` = resolved, no owning library route.
    /// Avoids a repeat `get_ancestors` round-trip for the same item
    /// within a session (#223). Each entry also carries the `Instant` it
    /// was cached at, so a mid-session library reorganization on the
    /// Emby server self-heals after `LIBRARY_ROUTE_CACHE_TTL` instead of
    /// requiring an app restart (#223, post-grilling revision item 5).
    library_route_cache: std::collections::HashMap<String, (Option<String>, Instant)>,
    force_clear: bool,
    tab_scroll: usize,
    ui_volume: u8,
    pre_mute_volume: Option<u8>,
    mute_on: bool,
    last_scroll_at: Instant,
    last_nav_at: Instant,
    last_power_library_nav_at: Instant,
    /// Set when the terminal reports FocusGained; used to swallow the
    /// single click that merely refocused the window. `None` until the
    /// first focus event is ever seen (terminals that never report focus
    /// never suppress).
    refocus_at: Option<Instant>,
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
    /// TV series detail cache for inline rendering in Power View.
    /// When a Series is selected, we proactively fetch seasons and episodes
    /// so the inline detail pane can render without drilling in.
    series_detail_cache: std::collections::HashMap<String, SeriesDetail>,
    series_detail_loading: std::collections::HashSet<String>,
    save_playlist_dialog: Option<SavePlaylistDialog>,
    image_protocol: Option<String>,
    image_protocol_enabled: bool,
    confirm_rescan: bool,
    pending_rescan_lib_idx: Option<usize>,
    library_position_state: crate::config::LibraryPositionState,
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
    library_routes: std::collections::HashMap<String, String>,
    hidden_latest: Vec<String>,
    music_levels: Vec<String>,
    use_nerd_fonts: bool,
    indicator_style: render::indicators::IndicatorStyle,
    image_cache_size: usize,
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

const SESSIONS_PANEL_W: u16 = 40;
const HELP_PANEL_W: u16 = 40;
const SETTINGS_PANEL_W: u16 = 40;
const PLAYLISTS_PANEL_W: u16 = 40;
impl App {
    pub(super) fn queue_column_width_max_for_terminal(terminal_width: u16) -> u16 {
        POWER_LEFT_WIDTH_DEFAULT.max(terminal_width.saturating_mul(3) / 5)
    }

    pub(super) fn normalize_queue_column_width(width: u16, terminal_width: u16) -> u16 {
        width.clamp(
            POWER_LEFT_WIDTH_DEFAULT,
            Self::queue_column_width_max_for_terminal(terminal_width),
        )
    }

    pub(super) fn clamp_queue_column_width(&mut self) -> bool {
        let normalized =
            Self::normalize_queue_column_width(self.queue_column_width, self.terminal_width);
        if normalized == self.queue_column_width {
            return false;
        }
        self.queue_column_width = normalized;
        true
    }

    /// Record that the terminal just regained focus, arming the
    /// refocus-click suppression window (see `handle_mouse`).
    pub(super) fn note_focus_gained(&mut self) {
        self.refocus_at = Some(Instant::now());
    }

    /// Clear any pending refocus suppression -- the window shouldn't
    /// outlive the focus session that armed it.
    pub(super) fn note_focus_lost(&mut self) {
        self.refocus_at = None;
    }

    /// Save the current position of `lib_idx` (#361 collapsed the old
    /// Default/Power scope split -- there is one view and one saved
    /// position per library now).
    fn save_default_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let library_id = lib.library.id.clone();
        let position = lib.library_position_snapshot();
        self.library_position_state
            .libraries
            .insert(library_id, position);
        crate::config::save_library_position_state(&self.library_position_state);
    }

    /// Whether `lib_idx` is the library currently visible in the left
    /// panel -- used to decide whether a manual refresh/rescan should clear
    /// its saved position (see `refresh_lib`/`trigger_lib_rescan`).
    fn active_library_position_scope_for(&self, lib_idx: usize) -> Option<()> {
        (self.library_tab == lib_idx + 1).then_some(())
    }

    fn saved_library_position(&self, lib_idx: usize) -> Option<crate::config::LibraryPosition> {
        let library_id = self.libs.get(lib_idx)?.library.id.as_str();
        self.library_position_state
            .libraries
            .get(library_id)
            .cloned()
    }

    fn replace_saved_library_position(
        &mut self,
        lib_idx: usize,
        position: crate::config::LibraryPosition,
    ) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        self.library_position_state
            .libraries
            .insert(lib.library.id.clone(), position);
        crate::config::save_library_position_state(&self.library_position_state);
    }

    fn set_panel_focus(&mut self, focus: PanelFocus) {
        if self.panel_focus == focus {
            return;
        }
        if matches!(focus, PanelFocus::Queue) {
            self.focus_power_queue_initial_item();
        }
        self.panel_focus = focus;
        self.save_prefs();
    }

    fn focus_power_queue_initial_item(&mut self) {
        let playback = self.displayed_queue_playback_state();
        let queue = self.displayed_queue_mut();
        if playback.active && playback.active_idx < queue.items.len() {
            queue.queue_cursor = playback.active_idx;
        } else if queue.queue_cursor >= queue.items.len() && !queue.items.is_empty() {
            queue.queue_cursor = 0;
        }
    }

    fn activate_library_position(&mut self, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let current = self
            .libs
            .get(lib_idx)
            .filter(|lib| !lib.nav_stack.is_empty())
            .map(|lib| lib.library_position_snapshot());
        let saved = self.saved_library_position(lib_idx);
        if current.as_ref() == saved.as_ref() {
            if current.is_none() {
                self.ensure_lib_loaded_for(lib_idx);
            } else if self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx) {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if lib.feed_home_video.is_none() {
                        lib.feed_home_video = Some(FeedHomeVideoState {
                            loading: true,
                            ..FeedHomeVideoState::default()
                        });
                    }
                }
                self.maybe_refresh_feed_groups_after_refresh(lib_idx);
            }
            return;
        }
        match saved {
            Some(position) if !position.levels.is_empty() => {
                let root = &position.levels[0];
                let restore_feed_view =
                    self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx);
                let placeholder = BrowseLevel {
                    parent_id: root.parent_id.clone(),
                    title: root.title.clone(),
                    items: Vec::new(),
                    total_count: 0,
                    cursor: 0,
                    item_types: root.item_types.clone(),
                    unplayed_only: root.unplayed_only,
                    sort_by: root.sort_by.clone(),
                    sort_order: root.sort_order.clone(),
                    loading: true,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                };
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if restore_feed_view {
                        lib.feed_home_video
                            .get_or_insert_with(FeedHomeVideoState::default)
                            .loading = true;
                    }
                    lib.apply_library_position(position.clone(), vec![placeholder]);
                }
                self.spawn_restore_library_position(lib_idx, position);
            }
            _ => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.apply_library_position(
                        crate::config::LibraryPosition::default(),
                        Vec::new(),
                    );
                }
                self.ensure_lib_loaded_for(lib_idx);
            }
        }
    }

    fn clear_saved_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if self
            .library_position_state
            .libraries
            .remove(&lib.library.id)
            .is_none()
        {
            return;
        }
        crate::config::save_library_position_state(&self.library_position_state);
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

    fn has_sessions_panel_connection(&self) -> bool {
        self.connected_session_id.is_some()
            || self.connected_session_state.is_some()
            || self.direct_remote_connected
    }

    fn can_disconnect_remote(&self) -> bool {
        self.has_sessions_panel_connection()
    }

    fn disconnect_remote(&mut self) {
        if self.connected_session_id.is_some() || self.connected_session_state.is_some() {
            self.connected_session_id = None;
            self.connected_session_state = None;
            self.session_miss_count = 0;
            self.remote_pos_s = 0;
            self.flash_status("Disconnected from remote session".to_string());
        } else if self.direct_remote_connected {
            self.restore_local_mode("Disconnected from direct remote session");
        } else {
            self.flash_status("No session connected".to_string());
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
            library_position_state: crate::config::load_library_position_state(),
            hidden_libraries: init.hidden_libraries,
            library_routes: init.library_routes,
            hidden_latest: init.hidden_latest,
            music_levels: init.music_levels,
            album_indexes: std::collections::HashMap::new(),
            use_nerd_fonts: init.use_nerd_fonts,
            indicator_style: init.indicator_style,
            image_cache_size: init.image_cache_size,
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
                home_cursor: 0,
                home_scroll: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            layout: layout::AppLayout::default(),
            terminal_width: 80,
            terminal_height: 24,

            home_loading: true,
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_click_pos: (u16::MAX, u16::MAX),
            last_space_press: None,
            last_esc_press: None,
            confirm_remove_idx: None,
            pending_delete_idx: None,
            pending_queue_removal: None,
            confirm_clear_queue: false,
            queue_undo_stack: Vec::new(),
            remote_queue_undo_stack: Vec::new(),
            pending_remote_move_cursor: None,
            skip_intro_end_ticks: None,
            next_up_item: None,
            // #361: read the new prefs key, falling back to the pre-#361 one
            // for one release. `power_focus`/`power_left_tab`/`power_left_width`
            // on disk are renamed to `panel_focus`/`library_tab`/`queue_column_width`;
            // this fallback can be deleted a release after that lands.
            panel_focus: PanelFocus::from_pref(
                prefs["panel_focus"]
                    .as_str()
                    .or_else(|| prefs["power_focus"].as_str()),
            ),
            library_tab: 0,
            queue_column_width: prefs["queue_column_width"]
                .as_u64()
                .or_else(|| prefs["power_left_width"].as_u64())
                .map(|v| (v as u16).max(POWER_LEFT_WIDTH_DEFAULT))
                .unwrap_or(POWER_LEFT_WIDTH_DEFAULT),
            queue_column_collapsed: false,
            library_tab_pending: prefs["library_tab"]
                .as_u64()
                .or_else(|| prefs["power_left_tab"].as_u64())
                .unwrap_or(0) as usize,
            queue_scroll: 0,
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
            library_routes_popup: None,
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
            last_keepalive: Instant::now(),
            last_capabilities: Instant::now(),
            connected_session_id: None,
            connected_session_state: None,
            direct_remote_connected: false,
            direct_remote_label: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            remote_api_pos_advanced_at: Instant::now() - Duration::from_secs(60),
            remote_seek_pending_until: Instant::now() - Duration::from_secs(1),
            runtime_zero_since: None,
            suspended_local: None,
            active_route: None,
            library_route_cache: std::collections::HashMap::new(),
            force_clear: false,
            tab_scroll: 0,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
            last_nav_at: Instant::now() - Duration::from_secs(1),
            last_power_library_nav_at: Instant::now() - Duration::from_secs(1),
            refocus_at: None,
            album_artist_cache: std::collections::HashMap::new(),
            album_artist_loading: std::collections::HashSet::new(),
            pending_album_artist_fetches: std::collections::VecDeque::new(),
            album_artist_fetches_active: 0,
            album_tracks_cache: std::collections::HashMap::new(),
            album_tracks_loading: std::collections::HashSet::new(),
            series_detail_cache: std::collections::HashMap::new(),
            series_detail_loading: std::collections::HashSet::new(),
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            confirm_rescan: false,
            pending_rescan_lib_idx: None,
            queue_scope: init.initial_queue_scope,
            stay_alive_ctrl: init.stay_alive_ctrl,
            attached: true,
            launched_as_remote: false,
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
        let library_routes = client.config.library_routes.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let system_notifications = client.config.system_notifications;
        let image_protocol = ui_config.image_protocol.clone();
        let image_protocol_enabled = image_protocol.is_some();
        let image_cache_size = ui_config.image_cache_size;
        let use_nerd_fonts = ui_config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            ui_config.indicator_style.parse().unwrap_or_default();
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
            library_routes,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
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
        app.try_auto_reconnect();
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
        let library_routes = client.config.library_routes.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let always_play_next = client.config.always_play_next;
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
            library_routes,
            hidden_latest,
            music_levels,
            use_nerd_fonts,
            indicator_style,
            image_cache_size,
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
        app.launched_as_remote = true;
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

    /// How often the run loop should repaint while otherwise idle (no key
    /// events, no completed fetches to react to). Fast (150 ms) whenever
    /// something is visibly in motion -- active local/remote playback, or a
    /// card image fetch in flight -- so states that only resolve with the
    /// passage of time (like a loading placeholder swapping in once its box
    /// should be reserved) actually get painted instead of being skipped
    /// between "just started" and "just finished" with nothing in between.
    /// Falls back to a slow 1 s cadence when nothing is changing, to avoid
    /// spinning the terminal for no reason.
    fn render_interval(&self) -> Duration {
        let playback = self.effective_playback_state();
        if playback.active
            || self.connected_session_state.is_some()
            || !self.card_image_loading.is_empty()
        {
            Duration::from_millis(150)
        } else {
            Duration::from_secs(1)
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode.
        self.image_picker = Some(self.build_image_picker());

        // Don't clobber a still-live flash message (e.g. try_auto_reconnect's
        // outcome, set during App::new) -- only show "Loading..." if there's
        // no pending flash, mirroring the render loop's own expiry check.
        let has_live_flash = self.status_expires.is_some_and(|t| t > Instant::now());
        if !has_live_flash {
            self.status = "Loading...".into();
        }
        self.home_loading = true;
        terminal.draw(|f| self.render(f))?;

        {
            let c = self.client.lock().unwrap();
            c.register_capabilities();
        }

        match self.fetch_home() {
            Ok(()) => {
                let has_live_flash = self.status_expires.is_some_and(|t| t > Instant::now());
                if !has_live_flash {
                    self.status.clear();
                }
            }
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
        let quit_timeout =
            Duration::from_secs(self.client.lock().unwrap().config.quit_timeout_secs);
        start_quit_watchdog(self.player.quit_handle(), quit_timeout);

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

            had_events |= self.drain_notif_actions();

            while let Ok(ev) = self.lib_rx.try_recv() {
                had_events = true;
                self.handle_lib_event(ev);
            }

            had_events |= self.drain_search_results();

            had_events |= self.drain_session_events();

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
                match ev {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        if self.handle_key(key) {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse);
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
                    Event::FocusGained => self.note_focus_gained(),
                    Event::FocusLost => self.note_focus_lost(),
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
                    crossterm::event::EnableMouseCapture,
                    crossterm::event::EnableFocusChange
                );
                self.force_clear = true;
                log::info!(target: "stay_alive", "reattach-refresh: capabilities re-detected, images invalidated");
            }

            self.sync_volume_from_player();

            // See `render_interval`'s doc comment for the fast/slow cadence rules.
            let render_interval = self.render_interval();
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

        self.teardown(quit_timeout);
        let _ = restore_terminal(terminal); // ignore errors — terminal may be gone (SIGHUP)
        Ok(())
    }

    /// Drain and act on notification-originated actions (skip-intro, next-up,
    /// clear-queue confirmation, notif-failure flag). Extracted from `run()`'s
    /// loop body; returns whether any action was received so the caller can
    /// fold that into its own `had_events` for render scheduling.
    fn drain_notif_actions(&mut self) -> bool {
        let mut produced = false;
        while let Ok(action) = self.notif_action_rx.try_recv() {
            produced = true;
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
        produced
    }

    /// Drain the search-results channel and surface any errors as a flash
    /// message. Extracted from `run()`'s loop body; returns whether any
    /// results were received so the caller can fold that into `had_events`.
    fn drain_search_results(&mut self) -> bool {
        let search_outcome = self.search.drain_results();
        let produced = search_outcome.received > 0;
        if produced {
            for error in search_outcome.errors {
                self.flash_status_high(format!("Search error: {error}"));
            }
        }
        produced
    }

    /// Drain the sessions-poll channel, dispatching each event to
    /// `handle_session_event`. Extracted from `run()`'s loop body; returns
    /// whether any event was received so the caller can fold that into
    /// `had_events`.
    fn drain_session_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(ev) = self.sessions_rx.try_recv() {
            produced = true;
            self.handle_session_event(ev);
        }
        produced
    }

    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    fn handle_session_event(&mut self, ev: SessionEvent) {
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
                                    if let Ok(mut items) =
                                        client.get_items_by_ids(std::slice::from_ref(&prev_id))
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
                        let api_active = self.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
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
                            if let Some(new_idx) = s.now_playing_item_id.as_ref().and_then(|id| {
                                self.player_tab.items.iter().position(|it| &it.id == id)
                            }) {
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
                            let since = self.runtime_zero_since.get_or_insert_with(Instant::now);
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
                if let Some(slot) = self.player_tab.items.iter_mut().find(|i| i.id == item_id) {
                    *slot = *fresh;
                }
            }
            SessionEvent::Error(e) => {
                self.sessions_loading = false;
                self.flash_status_high(format!("Sessions error: {e}"));
            }
        }
    }

    /// Shared local-player teardown sequence for both the signal-triggered
    /// quit-watchdog path (SIGHUP/SIGTERM) and the normal in-app quit-key
    /// path (both now break out of `run()`'s event loop the same way) —
    /// these two used to diverge, one bounded and one not, which is #202:
    /// an unbounded join on a hung `report_stopped` call during shutdown
    /// could hold the single-instance flock indefinitely. `quit_timeout`
    /// bounds every blocking step below; the player thread's own nested
    /// bounded calls (`ProgressGuard::stop_and_join`,
    /// `SessionReporter::report_stopped_for_shutdown`) each derive their
    /// own budget from the same value via `Player::stop_for_shutdown` —
    /// see the `outer_bound` comment below for why the outer join needs
    /// real headroom over those, not an identical `Duration`.
    ///
    /// Extracted from `run()`'s tail so it's callable directly against a
    /// stubbed `App` in tests without a real tty — `run()` itself remains
    /// untested end-to-end (unchanged status quo, not a regression; it has
    /// never had test coverage since it unconditionally calls
    /// `enable_raw_mode()`).
    fn teardown(&mut self, quit_timeout: Duration) {
        // #236: persist whichever remote connection (if any) is active
        // right now, before anything below or in the caller's cleanup
        // path clears `active_route` / direct-session identity -- so the
        // next launch's `App::new` can restore it. Mutually exclusive by
        // construction (library routing and Sessions-panel direct-remote
        // are two independent ways to end up thin-client; #223's
        // `restore_local_mode` and `connect_to_session` never let both be
        // set at once). Gated on `auto_reconnect` so the file is
        // never written (or read) at all when the feature is off. Also
        // gated on `!launched_as_remote`: `App::new_remote` instances never
        // populate `active_route`/`connected_session_state` (those are set
        // only by `App::new`'s runtime library-route-switch / session-attach
        // mechanisms), so running this block for them would always compute
        // `None` and wipe out a real record saved by a different `App::new`
        // session (per ADR 0010, `new_remote`'s path is unaffected by #236).
        if self.launched_as_remote {
            log::info!(target: "auto_reconnect", "teardown persistence skipped: launched as remote");
        } else if !self.client.lock().unwrap().config.auto_reconnect {
            log::info!(target: "auto_reconnect", "teardown persistence skipped: auto-reconnect disabled");
        } else {
            let last = if let Some(library) = self.active_route.clone() {
                log::info!(target: "auto_reconnect", "teardown decision=save-library-route library={library:?}");
                Some(mbv_core::config::LastRemoteConnection::LibraryRoute { library })
            } else if let Some(sess) = self.connected_session_state.as_ref() {
                log::info!(target: "auto_reconnect", "teardown decision=save-direct-session device={:?}", sess.device_name);
                Some(mbv_core::config::LastRemoteConnection::DirectSession {
                    device_name: sess.device_name.clone(),
                })
            } else {
                log::info!(target: "auto_reconnect", "teardown decision={}", if self.direct_remote_label.is_some() { "save-direct-session" } else { "clear" });
                self.direct_remote_label.as_ref().map(|device_name| {
                    mbv_core::config::LastRemoteConnection::DirectSession {
                        device_name: device_name.clone(),
                    }
                })
            };
            match mbv_core::config::save_last_remote_connection(last.as_ref()) {
                Ok(()) => log::info!(target: "auto_reconnect", "state persistence succeeded"),
                Err(e) => log::warn!(target: "auto_reconnect", "state persistence failed: {e}"),
            }
        }
        let quit_requested = QUIT_REQUESTED.load(Ordering::Relaxed);
        // Leave the daemon's player running when the TUI disconnects; only stop
        // and join the player when we own it locally. Both signal-triggered and
        // in-app quit paths share the same bounded local teardown.
        let (was_playing, current_idx, position_ticks, last_valid_pos) = {
            let st = self.player.status.lock().unwrap();
            (
                st.active,
                st.current_idx,
                st.position_ticks,
                st.last_valid_pos,
            )
        };
        log::info!(target: "player", "quit: requested={quit_requested} was_playing={was_playing} idx={current_idx} position_ticks={position_ticks} last_valid_pos={last_valid_pos} timeout={}s", quit_timeout.as_secs());
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
        if !self.player.is_remote() {
            self.player.stop_for_shutdown(quit_timeout);
            // The two nested bounded calls inside the player thread's own
            // shutdown path (on_shutdown, run sequentially) do NOT share an
            // identical budget: PlaybackSession::progress_join_budget gives
            // ProgressGuard::stop_and_join only quit_timeout/2 (it's a
            // secondary, non-network-critical join), while
            // report_stopped_for_shutdown keeps the full quit_timeout as its
            // own budget (the session-terminating call, worth protecting
            // most — see progress_join_budget's doc comment). Worst case the
            // two together take quit_timeout/2 + quit_timeout =
            // 1.5*quit_timeout, so the outer bound below is that plus a 3s
            // cushion — a real, explicit margin for the remaining
            // bookkeeping and fixed overhead (thread-spawn cost, contended
            // locks, drop cleanup; mark_played retry is fire-and-forget on a
            // detached thread and the PlayerEvent::Stopped send is a cheap
            // channel op), not just "the same Duration racing every layer of
            // the timeout composition" as an earlier version of this
            // function did.
            let outer_bound = quit_timeout + quit_timeout / 2 + Duration::from_secs(3);
            let started = Instant::now();
            self.player.join_or_timeout(outer_bound);
            let elapsed = started.elapsed();
            log::info!(target: "player", "quit: player join finished in {}ms (bound={}ms)",
                elapsed.as_millis(), outer_bound.as_millis());
        }
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
    crossterm::execute!(stdout, crossterm::event::EnableFocusChange)?;
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
    crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableFocusChange)?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "tests_ui_util.rs"]
mod tests_ui_util;

#[cfg(test)]
#[path = "tests_library_position.rs"]
mod tests_library_position;

#[cfg(test)]
#[path = "tests_lifecycle.rs"]
mod tests_lifecycle;

#[cfg(test)]
#[path = "tests_session_connect.rs"]
mod tests_session_connect;

#[cfg(test)]
#[path = "tests_feed_podcast.rs"]
mod tests_feed_podcast;

#[cfg(test)]
#[path = "tests_queue_scope.rs"]
mod tests_queue_scope;

#[cfg(test)]
#[path = "tests_queue_consume.rs"]
mod tests_queue_consume;

#[cfg(test)]
#[path = "tests_queue_mutation.rs"]
mod tests_queue_mutation;

#[cfg(test)]
#[path = "tests_route_state.rs"]
mod tests_route_state;

#[cfg(test)]
#[path = "tests_status_bar.rs"]
mod tests_status_bar;
