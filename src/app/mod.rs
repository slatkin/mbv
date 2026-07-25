mod action;
mod actions;
mod app_state_misc;
mod bootstrap;
mod construct;
mod context_menu_actions;
mod feed_actions;
pub(crate) mod images;
mod input;
mod input_context_menu;
mod input_mouse;
mod input_resolver;
pub(crate) mod layout;
mod library_browse_actions;
mod library_position_state;
mod library_route;
mod music_actions;
pub(crate) mod palette;
mod player_event;
mod queue_actions;
mod queue_scope;
mod remote_slot_state;
pub mod render;
mod resize;
mod run_loop_events;
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
use mbv_core::playback_queue::QueueSlotId;
#[cfg(test)]
use mbv_core::playback_queue::RemoveSlotResult;
use mbv_core::player::{PlayerCommand, PlayerEvent, PlayerProxy};
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
