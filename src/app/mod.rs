mod actions;
pub(crate) mod images;
mod input;
pub(crate) mod palette;
pub mod render;
mod settings;
pub(crate) mod ui_util;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
// Set only by SIGHUP or stdin POLLHUP (terminal vanished). Never set by q/SIGTERM.
// The watchdog's forced exit arms only on this flag so clean q-quits are never raced.
static TERMINAL_GONE: AtomicBool = AtomicBool::new(false);

pub(super) const PLAYLIST_VIEW_POWER: u8 = 1;
pub(super) const PLAYLIST_VIEW_COUNT: u8 = 2;
/// Width reserved on the right of the tab bar for the volume badge (+ gap/arrow).
pub(super) const TABBAR_RIGHT_RESERVE: u16 = 17;
/// Width reserved on the left of the tab bar for the control pill (`  m ⇌ ≡  ` + gap).
pub(super) const TABBAR_LEFT_RESERVE: u16 = 10;

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
fn start_quit_watchdog(quit_handle: Option<crate::player::QuitHandle>) {
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
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};

use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::api::{parse_mbv_direct_tcp_port, EmbyClient, MediaItem};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerProxy};
use crate::ws::WsEvent;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogPane {
    Sources,
    Log,
}

#[derive(Clone)]
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
    RemoveFromPlaylist(usize),
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
    items: Vec<crate::api::MediaItem>,
    results: Vec<usize>, // indices into items, sorted by score desc
    cursor: usize,       // position within results
    scroll: usize,       // viewport scroll offset for the results list
    loading: bool,       // true while full-library fetch is in flight
}

struct HomeSearch {
    query: String,
    last_query: String, // query string that produced current results
    results: Vec<crate::api::MediaItem>,
    cursor: usize,
    loading: bool,
    scroll: usize,
    type_filter: usize,  // 0 = All; 1..N index into sorted type list
    input_focused: bool, // true = typing into box; false = browsing results
}

impl HomeSearch {
    fn type_sort_key(t: &str) -> u8 {
        match t {
            "Movie" => 0,
            "Series" => 1,
            "Episode" => 2,
            "Audio" => 3,
            "MusicAlbum" => 4,
            "MusicArtist" => 5,
            _ => 6,
        }
    }

    pub(super) fn available_types(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut types: Vec<&str> = self
            .results
            .iter()
            .filter_map(|r| {
                let t = r.item_type.as_str();
                if seen.insert(t) {
                    Some(t)
                } else {
                    None
                }
            })
            .collect();
        types.sort_by_key(|t| Self::type_sort_key(t));
        types
    }

    pub(super) fn filtered_results(&self) -> Vec<&crate::api::MediaItem> {
        let types = self.available_types();
        let filter = if self.type_filter == 0 {
            None
        } else {
            types.get(self.type_filter - 1).copied()
        };
        self.results
            .iter()
            .filter(|r| filter.is_none_or(|t| r.item_type == t))
            .collect()
    }

    pub(super) fn filtered_count(&self) -> usize {
        self.filtered_results().len()
    }
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
    QueueRestored {
        items: Vec<MediaItem>,
        source: crate::config::QueueSource,
        last_played_item_id: Option<String>,
        last_played_completed: bool,
    },
    Error(String),
}

enum SessionEvent {
    Loaded(Vec<crate::api::SessionInfo>),
    ItemRefreshed(String, Box<crate::api::MediaItem>), // (item_id, fresh)
    Error(String),
}

#[derive(Default)]
struct PlayerTab {
    items: Vec<MediaItem>,
    playlist_cursor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueScope {
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemoteSlotState {
    Off,
    AttachedSession,
    DirectRemote,
    LocalDaemon,
}

/// Geometry of one power-home section card in the two-column grid, computed at render
/// time and reused by keyboard navigation (column jumps).
#[derive(Clone, Default)]
pub(crate) struct PowerHomeSectionMeta {
    pub flat_start: usize, // first flat item index in this section
    pub len: usize,        // number of items (0 for the empty Keep Watching card)
    pub row: usize,        // grid row
    pub col: usize,        // grid column
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
}

struct SuspendedLocalSession {
    player: PlayerProxy,
    player_rx: mpsc::Receiver<PlayerEvent>,
    ws_rx: mpsc::Receiver<WsEvent>,
    ws_send_tx: Option<crate::ws::WsSender>,
}

pub struct App {
    client: Arc<Mutex<EmbyClient>>,
    player: PlayerProxy,
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
    log_scroll: usize,
    log_pane: LogPane,        // which pane has focus
    log_source_cursor: usize, // selected row in sources pane
    log_disabled_sources: std::collections::HashSet<String>,
    // Layout rects from last render, used for mouse hit-testing
    playlist_rect: Rect,
    home_rect: Rect,
    layout_playlist_inner: Rect,
    layout_section_areas: Vec<Rect>,
    layout_tabs_area: Rect,
    terminal_width: u16,
    terminal_height: u16,
    layout_lib_scroll: Vec<usize>,
    layout_lib_row_heights: Vec<Vec<u16>>, // per lib_idx: height of each visible row, from scroll
    layout_home_scrolls: Vec<usize>,
    layout_home_scrollbar: Rect,

    home_panel_section_offset: usize,
    home_cards_section_offset: usize,
    layout_home_card_strips: Vec<(usize, Rect)>,
    layout_lib_table_area: Vec<Rect>,
    layout_breadcrumbs: Vec<(u16, u16, u16, usize)>, // (x_start, x_end, row, target nav_stack len)
    layout_power_breadcrumbs: Vec<(u16, u16, u16, usize)>, // same format, for power-view header crumbs
    layout_power_selector_tabs: Vec<(Rect, usize)>,        // selector pill rect → target index
    mouse_col: u16,
    mouse_row: u16,
    last_click_time: Instant,
    last_click_pos: (u16, u16),
    last_drag_seek: Instant,
    layout_seekbar_area: Rect,
    layout_button_area: Rect,
    layout_tracks_area: Rect,
    layout_vol_area: Rect,
    layout_sub_area: Rect,
    layout_audio_area: Rect,
    layout_ind_au: Rect,
    layout_ind_sub: Rect,
    layout_ind_rc: Rect,
    layout_ind_mu: Rect,
    layout_ind_pb: Rect,
    confirm_remove_idx: Option<usize>, // playlist index pending removal confirmation
    pending_delete_idx: Option<usize>, // deferred removal of now-playing item after Stopped event
    pending_queue_removal: Option<usize>, // deferred removal after TrackChanged index-shifts
    confirm_clear_playlist: bool,
    playlist_undo_stack: Vec<(usize, MediaItem)>,
    remote_playlist_undo_stack: Vec<(usize, MediaItem)>,
    skip_intro_end_ticks: Option<i64>,
    next_up_item: Option<MediaItem>,
    playlist_view: u8,
    playlist_group: bool, // list view: group audio by album / episodes by series
    playlist_row_map: Vec<Option<usize>>, // list view visual row → item index (None = header/spacer)
    power_focus: PowerFocus,
    power_left_tab: usize, // 0 = Home/CW, 1..=libs.len() = library index
    power_left_tab_pending: usize, // restored from prefs; applied once libs have loaded
    power_left_area: Rect, // rendered area of the left panel (for mouse click / page calc)
    power_queue_area: Rect,
    power_cursor_screen_y: Option<u16>, // screen Y of focused item in library panel (set by renderer)
    power_queue_cursor_screen_y: Option<u16>, // screen Y of focused item in queue panel (set by renderer)
    power_inline_image_rect: Option<Rect>, // bounding rect of inline poster image in detail/episode views
    power_queue_scope_local_area: Rect,
    power_queue_scope_remote_area: Rect,
    power_queue_scroll: usize,
    power_queue_row_map: Vec<Option<usize>>, // visual row → item index (None = album header)
    power_left_row_map: Vec<Option<usize>>,  // visual row → item index for library letter groups
    power_home_hitmap: Vec<(Rect, usize)>,   // home-tab grid: item row rect → flat index (mouse)
    power_home_layout: Vec<PowerHomeSectionMeta>, // home-tab grid: per-section geometry (nav)
    power_left_sorted_indices: Vec<usize>,   // full sorted display order when letter-groups active
    power_detail_max_scroll: usize,          // max valid scroll (set each render frame)
    power_detail_page_h: usize,              // visible overview line count (set each render frame)
    // Whether the power-view queue is currently relocated to the bottom of the
    // right column (low-height layout). Sticky with hysteresis so a small,
    // transient change in the card image's rendered height (e.g. switching
    // from a season poster to an episode thumbnail while browsing seasons)
    // doesn't flip the whole right-panel layout and cause a visible reflow.
    power_queue_relocated: bool,
    home_card_view: bool,
    last_played_item_id: Option<String>,
    last_played_completed: bool,
    layout_carousel_slots: [(Option<usize>, Rect); 3],
    layout_carousel_left_arrow: Option<Rect>,
    layout_carousel_right_arrow: Option<Rect>,
    layout_carousel_up_arrow: Option<Rect>,
    layout_carousel_down_arrow: Option<Rect>,
    card_image_states: std::collections::HashMap<String, Option<StatefulProtocol>>,
    image_lru: std::collections::VecDeque<String>,
    image_cache_size: usize,
    card_image_loading: std::collections::HashSet<String>,
    last_card_height: u16,
    pending_image_fetches: std::collections::VecDeque<images::ImageFetchReq>,
    image_fetches_active: usize,
    card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    image_picker: Option<Picker>,
    context_menu: Option<ContextMenu>,
    context_menu_rect: Option<Rect>,
    show_help: bool,
    show_settings: bool,
    settings_cursor: usize,
    settings_scroll: usize,
    settings_save_at: Option<Instant>,
    confirm_logout: bool,
    multiselect_popup: Option<MultiSelectPopup>,
    layout_settings_area: Rect,
    settings_line_of_cursor: Vec<usize>,
    help_scroll: u16,
    show_log_tab: bool,
    system_notifications: bool,
    notif_failed: bool,
    notif_action_tx: mpsc::Sender<String>,
    notif_action_rx: mpsc::Receiver<String>,
    lib_tx: mpsc::Sender<LibEvent>,
    lib_rx: mpsc::Receiver<LibEvent>,
    home_search: Option<HomeSearch>,
    search_tx: mpsc::Sender<Result<Vec<MediaItem>, String>>,
    search_rx: mpsc::Receiver<Result<Vec<MediaItem>, String>>,
    sessions: Vec<crate::api::SessionInfo>,
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
    queue_restored: bool,
    /// True from `spawn_restore_queue_state` until the `QueueRestored` event
    /// is processed. Prevents `save_queue_state` from overwriting the on-disk
    /// state with an empty queue while the restore is still in-flight.
    queue_restore_pending: bool,
    queue_dirty: bool,
    pending_queue_action: Option<PendingQueueAction>,
    show_save_playlist_modal: bool,
    use_nerd_fonts: bool,
    indicator_style: render::indicators::IndicatorStyle,
    panel_mode: crate::config::PanelMode,
    ws_send_tx: Option<crate::ws::WsSender>,
    last_keepalive: Instant,
    last_capabilities: Instant,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
    connected_session_id: Option<String>,
    connected_session_state: Option<crate::api::SessionInfo>,
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
    layout_tabbar_vol_area: Rect,
    last_scroll_at: Instant,
    last_nav_at: Instant,
    album_year_cache: std::collections::HashMap<String, u32>,
    album_year_loading: std::collections::HashSet<String>,
    save_playlist_dialog: Option<SavePlaylistDialog>,
    image_protocol_enabled: bool,
    confirm_rescan: bool,
    queue_scope: QueueScope,
}

struct AppInit {
    client: std::sync::Arc<std::sync::Mutex<EmbyClient>>,
    player: crate::player::PlayerProxy,
    player_rx: std::sync::mpsc::Receiver<crate::player::PlayerEvent>,
    ws_rx: std::sync::mpsc::Receiver<WsEvent>,
    ws_send_tx: Option<crate::ws::WsSender>,
    player_tab: PlayerTab,
    remote_player_tab: Option<PlayerTab>,
    show_log_tab: bool,
    system_notifications: bool,
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
}

enum PendingQueueAction {
    PlayItems {
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
    },
    ClearQueue,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PowerFocus {
    Queue, // left panel (queue list below the card)
    #[default]
    Left, // right panel (library browser); driven by power_left_tab
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingKey {
    DaemonModeOnExit,
    StartOnQueue,
    AlwaysPlayNext,
    ConsumeVideos,
    SavePlaylistOnConsume,
    AlwaysSkipIntro,
    ShowLogTab,
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
            SettingKey::DaemonModeOnExit,
            SettingKey::AlwaysSkipIntro,
            SettingKey::ShowLogTab,
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
            SettingKey::SavePlaylistOnConsume,
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

    fn build(init: AppInit) -> Self {
        let prefs = Self::load_prefs();
        let has_remote_queue = init.remote_player_tab.is_some();
        App {
            client: init.client,
            player: init.player,
            player_rx: init.player_rx,
            ws_rx: init.ws_rx,
            ws_send_tx: init.ws_send_tx,
            player_tab: init.player_tab,
            remote_player_tab: init.remote_player_tab,
            show_log_tab: init.show_log_tab,
            system_notifications: init.system_notifications,
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
            home_search: None,
            search_tx: init.search_tx,
            search_rx: init.search_rx,
            sessions_tx: init.sessions_tx,
            sessions_rx: init.sessions_rx,
            card_image_tx: init.card_image_tx,
            card_image_rx: init.card_image_rx,
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
            log_scroll: 0,
            log_pane: LogPane::Log,
            log_source_cursor: 0,
            log_disabled_sources: std::collections::HashSet::new(),
            playlist_rect: Rect::default(),
            home_rect: Rect::default(),
            layout_playlist_inner: Rect::default(),
            layout_section_areas: Vec::new(),
            layout_tabs_area: Rect::default(),
            terminal_width: 80,
            terminal_height: 24,
            layout_lib_scroll: Vec::new(),
            layout_lib_row_heights: Vec::new(),
            layout_home_scrolls: Vec::new(),
            layout_home_scrollbar: Rect::default(),

            home_panel_section_offset: 0,
            home_cards_section_offset: 0,
            layout_home_card_strips: Vec::new(),
            layout_lib_table_area: Vec::new(),
            layout_breadcrumbs: Vec::new(),
            layout_power_breadcrumbs: Vec::new(),
            layout_power_selector_tabs: Vec::new(),
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_click_pos: (u16::MAX, u16::MAX),
            layout_seekbar_area: Rect::default(),
            layout_button_area: Rect::default(),
            layout_tracks_area: Rect::default(),
            layout_vol_area: Rect::default(),
            layout_sub_area: Rect::default(),
            layout_audio_area: Rect::default(),
            layout_ind_au: Rect::default(),
            layout_ind_sub: Rect::default(),
            layout_ind_rc: Rect::default(),
            layout_ind_mu: Rect::default(),
            layout_ind_pb: Rect::default(),
            confirm_remove_idx: None,
            pending_delete_idx: None,
            pending_queue_removal: None,
            confirm_clear_playlist: false,
            playlist_undo_stack: Vec::new(),
            remote_playlist_undo_stack: Vec::new(),
            skip_intro_end_ticks: None,
            next_up_item: None,
            playlist_view: prefs["playlist_view"].as_u64().unwrap_or(0).min(1) as u8,
            playlist_group: true,
            playlist_row_map: Vec::new(),
            power_focus: PowerFocus::default(),
            power_left_tab: 0,
            power_left_tab_pending: prefs["power_left_tab"].as_u64().unwrap_or(0) as usize,
            power_left_area: Rect::default(),
            power_queue_area: Rect::default(),
            power_cursor_screen_y: None,
            power_queue_cursor_screen_y: None,
            power_inline_image_rect: None,
            power_queue_scope_local_area: Rect::default(),
            power_queue_scope_remote_area: Rect::default(),
            power_queue_scroll: 0,
            power_queue_row_map: Vec::new(),
            power_left_row_map: Vec::new(),
            power_home_hitmap: Vec::new(),
            power_home_layout: Vec::new(),
            power_left_sorted_indices: Vec::new(),
            power_detail_max_scroll: 0,
            power_detail_page_h: 5,
            power_queue_relocated: false,
            home_card_view: false,
            ui_volume: prefs["ui_volume"].as_u64().unwrap_or(100).min(200) as u8,
            pre_mute_volume: prefs["pre_mute_volume"].as_u64().map(|v| v as u8),
            mute_on: prefs["mute_on"].as_bool().unwrap_or(false),
            layout_tabbar_vol_area: Rect::default(),
            last_played_item_id: None,
            last_played_completed: false,
            layout_carousel_slots: [(None, Rect::default()); 3],
            layout_carousel_left_arrow: None,
            layout_carousel_right_arrow: None,
            layout_carousel_up_arrow: None,
            layout_carousel_down_arrow: None,
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
            layout_settings_area: Rect::default(),
            settings_line_of_cursor: Vec::new(),
            help_scroll: 0,
            notif_failed: false,
            context_menu: None,
            context_menu_rect: None,
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
            queue_restored: false,
            queue_restore_pending: false,
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
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            confirm_rescan: false,
            queue_scope: if has_remote_queue {
                QueueScope::Remote
            } else {
                QueueScope::Local
            },
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
        let server_url = client.config.server_url.clone();
        let token = client.token.clone();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let show_log_tab = client.config.show_log_tab;
        let system_notifications = client.config.system_notifications;
        let image_protocol_enabled = client.config.image_protocol.is_some();
        let image_cache_size = client.config.image_cache_size;
        let use_nerd_fonts = client.config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            client.config.indicator_style.parse().unwrap_or_default();
        let start_on_queue = client.config.start_on_queue;
        let always_play_next = client.config.always_play_next;
        let always_skip_intro = client.config.always_skip_intro;
        crate::config::evict_old_image_cache();
        let ws_url = client.ws_url();
        let ws_send_tx = crate::ws::start(ws_url, ws_tx);
        let ws_send_tx_app = ws_send_tx.clone();
        // Prefer local config; fall back to Emby server prefs only on first run (all empty).
        let subtitle_prefs = if client.config.subtitle_mode.is_empty()
            && client.config.subtitle_lang.is_empty()
            && client.config.audio_lang.is_empty()
        {
            client.get_user_subtitle_prefs().unwrap_or_default()
        } else {
            crate::player::SubtitlePrefs {
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
        crate::mpris::start(player_status, move |cmd| {
            if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                let _ = tx.send(cmd);
            }
        });
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
        Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: Some(ws_send_tx_app),
            player_tab: PlayerTab {
                items: Vec::new(),
                playlist_cursor: 0,
            },
            remote_player_tab: None,
            show_log_tab,
            system_notifications,
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
        })
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
        remote: crate::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
        is_local_daemon: bool,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<crate::ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) =
            mpsc::channel::<(String, Option<image::DynamicImage>)>();
        let (notif_action_tx, notif_action_rx) = mpsc::channel::<String>();
        let (search_tx, search_rx) = mpsc::channel::<Result<Vec<MediaItem>, String>>();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let always_play_next = client.config.always_play_next;
        let start_on_queue = client.config.start_on_queue;
        let image_protocol_enabled = client.config.image_protocol.is_some();
        let image_cache_size = client.config.image_cache_size;
        let use_nerd_fonts = client.config.use_nerd_fonts;
        let indicator_style: render::indicators::IndicatorStyle =
            client.config.indicator_style.parse().unwrap_or_default();
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
        let initial_tab = PlayerTab {
            items: remote.items.lock().unwrap().clone(),
            playlist_cursor: remote.status.lock().unwrap().current_idx,
        };
        let player = PlayerProxy::remote(remote, always_play_next);
        let (player_tab, remote_player_tab) = if is_local_daemon {
            // Local daemon: one unified queue, exactly like plain local
            // playback — no separate remote_player_tab, no scope pill.
            (initial_tab, None)
        } else {
            // Remote/network daemon: keep a separate remote queue so the
            // user can browse locally while the daemon plays elsewhere.
            (PlayerTab::default(), Some(initial_tab))
        };
        Self::build(AppInit {
            client: client_arc,
            player,
            player_rx,
            ws_rx,
            ws_send_tx: None,
            player_tab,
            remote_player_tab,
            show_log_tab: false,
            system_notifications: false,
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
        })
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

    fn undo_stack_for_scope_mut(&mut self, scope: QueueScope) -> &mut Vec<(usize, MediaItem)> {
        match scope {
            QueueScope::Local => &mut self.playlist_undo_stack,
            QueueScope::Remote => &mut self.remote_playlist_undo_stack,
        }
    }

    fn queue_scope_has_local_metadata(&self, scope: QueueScope) -> bool {
        scope == QueueScope::Local || !self.has_direct_remote_queue()
    }

    fn queue_scope_is_playback(&self, scope: QueueScope) -> bool {
        scope == self.playback_queue_scope()
    }

    fn action_queue_scope(&self, action: &PendingQueueAction) -> QueueScope {
        match action {
            PendingQueueAction::PlayItems { .. } => self.playback_queue_scope(),
            PendingQueueAction::ClearQueue => self.displayed_queue_scope(),
            PendingQueueAction::Quit => QueueScope::Local,
        }
    }

    fn action_touches_local_queue(&self, action: &PendingQueueAction) -> bool {
        matches!(action, PendingQueueAction::Quit)
            || self.queue_scope_has_local_metadata(self.action_queue_scope(action))
    }

    fn clear_local_queue_metadata(&mut self) {
        self.queue_source = crate::config::QueueSource::Unknown;
        self.queue_restored = false;
        self.queue_dirty = false;
        self.playlist_undo_stack.clear();
    }

    fn persist_local_queue_state_if_needed(&self, scope: QueueScope) {
        if self.queue_scope_has_local_metadata(scope) {
            self.save_queue_state();
        }
    }

    fn replace_direct_remote_queue(&mut self, items: Vec<MediaItem>, cursor: usize) {
        let cursor = cursor.min(items.len().saturating_sub(1));
        self.player.send_command(crate::player::PlayerCommand::ReplacePlaylist {
            items: items.clone(),
            start_idx: cursor,
        });
        if let Some(queue) = self.remote_player_tab.as_mut() {
            queue.items = items;
            queue.playlist_cursor = cursor;
        }
    }

    fn sync_direct_remote_queue_after_edit(&mut self, scope: QueueScope) {
        if scope == QueueScope::Remote && self.has_direct_remote_queue() {
            let (items, cursor) = {
                let queue = self
                    .remote_player_tab
                    .as_ref()
                    .expect("direct remote queue requires remote queue");
                (queue.items.clone(), queue.playlist_cursor)
            };
            self.replace_direct_remote_queue(items, cursor);
        }
    }

    fn playback_queue_scope(&self) -> QueueScope {
        if self.has_direct_remote_queue() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    fn replace_playback_queue(&mut self, items: Vec<MediaItem>, cursor: usize) {
        let cursor = cursor.min(items.len().saturating_sub(1));
        match self.playback_queue_scope() {
            QueueScope::Local => {
                self.player_tab.items = items;
                self.player_tab.playlist_cursor = cursor;
            }
            QueueScope::Remote => {
                let queue = self
                    .remote_player_tab
                    .as_mut()
                    .expect("direct remote playback queue requires remote queue");
                queue.items = items;
                queue.playlist_cursor = cursor;
            }
        }
    }

    fn displayed_queue_scope(&self) -> QueueScope {
        if self.has_direct_remote_queue() && self.queue_scope == QueueScope::Remote {
            QueueScope::Remote
        } else {
            QueueScope::Local
        }
    }

    fn displayed_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.displayed_queue_scope())
    }

    fn displayed_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.displayed_queue_scope())
    }

    fn playback_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.playback_queue_scope())
    }

    fn playback_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.playback_queue_scope())
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
        sess: &crate::api::SessionInfo,
    ) -> Option<crate::remote_player::DaemonEndpoint> {
        if !sess.client.eq_ignore_ascii_case("mbv") {
            return None;
        }
        if let Some(port) = parse_mbv_direct_tcp_port(&sess.supported_commands) {
            if let Ok(ip) = sess.host.parse::<std::net::Ipv4Addr>() {
                return Some(crate::remote_player::DaemonEndpoint::Tcp(
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
            .then_some(crate::remote_player::DaemonEndpoint::Local)
    }

    fn switch_to_direct_remote(
        &mut self,
        sess: &crate::api::SessionInfo,
        remote: crate::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
    ) {
        let initial_items = remote.items.lock().unwrap().clone();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.client.lock().unwrap().config.always_play_next;

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

        self.remote_player_tab = Some(PlayerTab {
            items: initial_items,
            playlist_cursor: initial_cursor,
        });
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
        self.set_queue_scope(QueueScope::Remote);
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

    fn connect_to_session(&mut self, sess: &crate::api::SessionInfo) {
        if !self.player.is_remote() {
            if let Some(endpoint) = self.session_direct_endpoint(sess) {
                let auth_token = self.client.lock().unwrap().token.clone();
                match crate::remote_player::RemotePlayer::connect_endpoint(&endpoint, &auth_token) {
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

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode.
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.client.lock().unwrap().config.image_protocol.clone();
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
        self.image_picker = Some(picker);

        self.status = "Loading...".into();
        terminal.draw(|f| self.render(f))?;

        {
            let c = self.client.lock().unwrap();
            c.register_capabilities();
        }

        match self.fetch_home() {
            Ok(()) => self.status.clear(),
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
        self.spawn_restore_queue_state();
        terminal.draw(|f| self.render(f))?;

        install_signal_handlers();
        start_quit_watchdog(self.player.quit_handle());

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
                            let secs = end_ticks as f64 / crate::api::TICKS_PER_SECOND as f64;
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
                                self.playback_queue_mut().playlist_cursor = idx;
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
                        if self.confirm_clear_playlist {
                            self.confirm_clear_playlist = false;
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

            while let Ok(result) = self.search_rx.try_recv() {
                had_events = true;
                if let Some(ref mut hs) = self.home_search {
                    hs.loading = false;
                    hs.cursor = 0;
                    hs.scroll = 0;
                    hs.type_filter = 0;
                    match result {
                        Ok(items) => {
                            hs.results = items;
                        }
                        Err(e) => {
                            hs.results = Vec::new();
                            self.flash_status_high(format!("Search error: {e}"));
                        }
                    }
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
                                        self.player_tab.playlist_cursor = new_idx;
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
                // Image was decoded off-thread; just build the render protocol.
                let state: Option<StatefulProtocol> = img_opt.and_then(|dyn_img| {
                    self.image_picker
                        .as_ref()
                        .map(|p| p.new_resize_protocol(dyn_img))
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

            while let Ok(ev) = self.ws_rx.try_recv() {
                had_events = true;
                self.handle_ws_event(ev);
            }

            if let Some(at) = self.settings_save_at {
                if Instant::now() >= at {
                    let cfg = self.client.lock().unwrap().config.clone();
                    crate::config::save_config_settings(&cfg);
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
                            && self.home_rect.contains((mouse.column, mouse.row).into());
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
                    _ => {}
                }
            }

            self.sync_volume_from_player();

            // Animate the now-playing spinner at ~150 ms whenever a local player
            // is active or a remote session is connected; fall back to 1 s when
            // fully idle. Remote queue views need the fast cadence even if the
            // active item match is temporarily unavailable.
            let render_interval = {
                let (active, ..) = self.effective_playback_state();
                if active || self.connected_session_state.is_some() {
                    Duration::from_millis(150)
                } else {
                    Duration::from_secs(1)
                }
            };
            if had_events || self.force_clear || last_render.elapsed() >= render_interval {
                if self.force_clear {
                    self.force_clear = false;
                    terminal.clear()?;
                }
                terminal.draw(|f| self.render(f))?;
                last_render = Instant::now();
            }
        }

        // Signal quit (SIGHUP/SIGTERM — terminal closed or process termination).
        // Stop player and join its thread so the mpv window closes and
        // report_stopped completes before we exit. The player thread closes
        // the window before making the HTTP call (see SingleSession/PlaylistSession
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
            self.save_queue_state();
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
        // position_ticks (transiently 0 when PlaylistSession advances to the next track).
        if was_playing && !self.has_direct_remote_queue() {
            if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                if last_valid_pos > 0 && !item.is_audio() {
                    item.playback_position_ticks = last_valid_pos;
                }
                self.last_played_item_id = Some(item.id.clone());
            }
        }
        self.save_queue_state();
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
                error,
            } => {
                log::info!(target: "player", "Stopped event: idx={idx} position_ticks={}s played={played} error={error:?}",
                    position_ticks / crate::api::TICKS_PER_SECOND);
                if self.player.is_remote_disconnected() {
                    self.next_up_item = None;
                    self.skip_intro_end_ticks = None;
                    self.restore_local_mode("Daemon disconnected — returned to local mode");
                    self.refresh_after_stop();
                    return true;
                }
                let is_delete = self.pending_delete_idx.take() == Some(idx);
                let preserve_local_state = !self.has_direct_remote_queue();
                if let Some(item) = self.playback_queue_mut().items.get_mut(idx) {
                    if !is_delete {
                        if played {
                            item.playback_position_ticks = 0;
                            item.played = true;
                            log::info!(target: "player", "Stopped: marked played, position reset to 0");
                        } else if position_ticks > 0 && !item.is_audio() {
                            item.playback_position_ticks = position_ticks;
                            log::info!(target: "player", "Stopped: saved position={}s", position_ticks / crate::api::TICKS_PER_SECOND);
                        } else {
                            log::info!(target: "player", "Stopped: position not saved (position_ticks={} is_audio={})", position_ticks, item.is_audio());
                        }
                    }
                    if preserve_local_state {
                        self.last_played_item_id = Some(item.id.clone());
                        self.last_played_completed = played;
                    }
                }
                self.next_up_item = None;
                self.skip_intro_end_ticks = None;
                self.status.clear();
                if is_delete {
                    let allow_undo = !self.player.is_remote();
                    let item = {
                        let queue = self.playback_queue_mut();
                        let item = queue.items.remove(idx);
                        queue.playlist_cursor = if queue.items.is_empty() {
                            0
                        } else {
                            idx.min(queue.items.len() - 1)
                        };
                        item
                    };
                    if allow_undo {
                        self.playlist_undo_stack.push((idx, item));
                    }
                } else {
                    let is_video = self
                        .playback_queue()
                        .items
                        .get(idx)
                        .is_some_and(|i| i.is_video());
                    if played && is_video && self.client.lock().unwrap().config.consume_videos {
                        let queue = self.playback_queue_mut();
                        if idx < queue.items.len() {
                            queue.items.remove(idx);
                        }
                        if queue.items.is_empty() {
                            queue.playlist_cursor = 0;
                        } else {
                            queue.playlist_cursor = queue
                                .playlist_cursor
                                .min(queue.items.len().saturating_sub(1));
                        }
                    }
                }
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
            } => {
                if let Some(item) = self.playback_queue_mut().items.get_mut(idx) {
                    if played {
                        item.playback_position_ticks = 0;
                        item.played = true;
                    } else if position_ticks >= 300_000_000 && !item.is_audio() {
                        // Only update local position for meaningful progress (≥ 30 s).
                        // Startup noise from mpv (< 30 s) keeps the previous value intact.
                        item.playback_position_ticks = position_ticks;
                    }
                }
                let is_video = self
                    .playback_queue()
                    .items
                    .get(idx)
                    .is_some_and(|i| i.is_video());
                if consume && is_video && self.client.lock().unwrap().config.consume_videos {
                    self.pending_queue_removal = Some(idx);
                }
            }
            PlayerEvent::TrackChanged(idx) => {
                self.skip_intro_end_ticks = None;
                self.next_up_item = None;
                if self.status.starts_with("Next up:") {
                    self.status.clear();
                }
                let adjusted = if let Some(remove_idx) = self.pending_queue_removal.take() {
                    let queue = self.playback_queue_mut();
                    if remove_idx < queue.items.len() {
                        queue.items.remove(remove_idx);
                    }
                    if remove_idx < idx {
                        idx - 1
                    } else {
                        idx
                    }
                } else {
                    idx
                };
                self.playback_queue_mut().playlist_cursor = adjusted;
                if !self.has_direct_remote_queue() {
                    if let Some(item) = self.playback_queue().items.get(adjusted) {
                        self.last_played_item_id = Some(item.id.clone());
                    }
                }
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
            }
            PlayerEvent::PlaylistNextUp { next_idx } => {
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
                // Series episodes now use play_playlist; this only fires for movies
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
                        self.playback_queue_mut().playlist_cursor = idx;
                        self.flash_status(label);
                    } else {
                        log::warn!(target: "app", "next-up: item not in queue, cannot jump");
                    }
                } else {
                    log::warn!(target: "app", "next-up: NextUpPlay fired but next_up_item is None");
                }
            }
            PlayerEvent::QueueUpdated { items, cursor } => {
                let queue = self.playback_queue_mut();
                queue.items = items;
                queue.playlist_cursor = cursor;
            }
            PlayerEvent::IntroStarted { intro_end_ticks } => {
                self.skip_intro_end_ticks = Some(intro_end_ticks);
                let playing_title = self
                    .playback_queue()
                    .items
                    .get(self.playback_queue().playlist_cursor)
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
    use crate::api::TICKS_PER_SECOND;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    fn make_session(device_name: &str, client: &str) -> crate::api::SessionInfo {
        crate::api::SessionInfo {
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
            media_info: crate::api::SessionMediaInfo::default(),
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

    /// Minimal App stub for logic-only tests.
    pub(crate) fn make_app_stub() -> App {
        use crate::player::{PlayerProxy, PlayerStatus};
        use std::sync::{Arc, Mutex};

        let status = Arc::new(Mutex::new(PlayerStatus {
            position_ticks: 0,
            last_valid_pos: 0,
            runtime_ticks: 0,
            paused: false,
            volume: 100,
            volume_max: 100,
            current_idx: 0,
            active: false,
            title: String::new(),
            audio_tracks: Vec::new(),
            sub_tracks: Vec::new(),
            sub_track_stream_indexes: Vec::new(),
            audio_id: 0,
            audio_lang: String::new(),
            sub_id: 0,
            sub_lang: String::new(),
            muted: false,
            video_height: 0,
            audio_codec: String::new(),
            video_is_image: false,
        }));

        let (_, player_rx) = std::sync::mpsc::channel();
        let (_, ws_rx) = std::sync::mpsc::channel();
        let (lib_tx, lib_rx) = std::sync::mpsc::channel();
        let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
        let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
        let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
        let (search_tx, search_rx) = std::sync::mpsc::channel::<Result<Vec<MediaItem>, String>>();

        let player = PlayerProxy::stub(status.clone());

        use crate::api::EmbyClient;
        use crate::config::Config;
        let client = EmbyClient::new(Config::default());

        App {
            client: Arc::new(Mutex::new(client)),
            player,
            player_rx,
            ws_rx,
            tab_idx: 0,
            hidden_libraries: Vec::new(),
            hidden_latest: Vec::new(),
            music_levels: Vec::new(),
            player_tab: PlayerTab {
                items: Vec::new(),
                playlist_cursor: 0,
            },
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
            log_scroll: 0,
            log_pane: LogPane::Log,
            log_source_cursor: 0,
            log_disabled_sources: std::collections::HashSet::new(),
            playlist_rect: ratatui::layout::Rect::default(),
            home_rect: ratatui::layout::Rect::default(),
            layout_playlist_inner: ratatui::layout::Rect::default(),
            layout_section_areas: Vec::new(),
            layout_tabs_area: ratatui::layout::Rect::default(),
            terminal_width: 80,
            terminal_height: 24,
            layout_lib_scroll: Vec::new(),
            layout_lib_row_heights: Vec::new(),
            layout_home_scrolls: Vec::new(),
            layout_home_scrollbar: Rect::default(),

            home_panel_section_offset: 0,
            home_cards_section_offset: 0,
            layout_home_card_strips: Vec::new(),
            layout_lib_table_area: Vec::new(),
            layout_breadcrumbs: Vec::new(),
            layout_power_breadcrumbs: Vec::new(),
            layout_power_selector_tabs: Vec::new(),
            mouse_col: 0,
            mouse_row: 0,
            last_click_time: std::time::Instant::now(),
            last_drag_seek: std::time::Instant::now(),
            last_click_pos: (u16::MAX, u16::MAX),
            layout_seekbar_area: ratatui::layout::Rect::default(),
            layout_button_area: ratatui::layout::Rect::default(),
            layout_tracks_area: ratatui::layout::Rect::default(),
            layout_vol_area: ratatui::layout::Rect::default(),
            layout_sub_area: ratatui::layout::Rect::default(),
            layout_audio_area: ratatui::layout::Rect::default(),
            layout_ind_au: Rect::default(),
            layout_ind_sub: Rect::default(),
            layout_ind_rc: Rect::default(),
            layout_ind_mu: Rect::default(),
            layout_ind_pb: Rect::default(),
            confirm_remove_idx: None,
            pending_delete_idx: None,
            pending_queue_removal: None,
            confirm_clear_playlist: false,
            playlist_undo_stack: Vec::new(),
            remote_playlist_undo_stack: Vec::new(),
            skip_intro_end_ticks: None,
            next_up_item: None,
            playlist_view: 0,
            playlist_group: true,
            playlist_row_map: Vec::new(),
            power_focus: PowerFocus::default(),
            power_left_tab: 0,
            power_left_tab_pending: 0,
            power_left_area: Rect::default(),
            power_queue_area: Rect::default(),
            power_cursor_screen_y: None,
            power_queue_cursor_screen_y: None,
            power_inline_image_rect: None,
            power_queue_scope_local_area: Rect::default(),
            power_queue_scope_remote_area: Rect::default(),
            power_queue_scroll: 0,
            power_queue_row_map: Vec::new(),
            power_left_row_map: Vec::new(),
            power_home_hitmap: Vec::new(),
            power_home_layout: Vec::new(),
            power_left_sorted_indices: Vec::new(),
            power_detail_max_scroll: 0,
            power_detail_page_h: 5,
            power_queue_relocated: false,
            home_card_view: false,
            last_played_item_id: None,
            last_played_completed: false,
            layout_carousel_slots: [(None, ratatui::layout::Rect::default()); 3],
            layout_carousel_left_arrow: None,
            layout_carousel_right_arrow: None,
            layout_carousel_up_arrow: None,
            layout_carousel_down_arrow: None,
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            card_image_tx,
            card_image_rx,
            image_picker: None,
            show_help: false,
            show_settings: false,
            settings_cursor: 0,
            settings_scroll: 0,
            settings_save_at: None,
            confirm_logout: false,
            multiselect_popup: None,
            layout_settings_area: Rect::default(),
            settings_line_of_cursor: Vec::new(),
            help_scroll: 0,
            show_log_tab: false,
            system_notifications: false,
            notif_failed: false,
            notif_action_tx,
            notif_action_rx,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            home_search: None,
            search_tx,
            search_rx,
            force_clear: false,
            tab_scroll: 0,
            ui_volume: 100,
            pre_mute_volume: None,
            mute_on: false,
            layout_tabbar_vol_area: Rect::default(),
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
            queue_restored: false,
            queue_restore_pending: false,
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
            save_playlist_dialog: None,
            image_lru: std::collections::VecDeque::new(),
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            image_cache_size: 50,
            image_protocol_enabled: false,
            confirm_rescan: false,
            queue_scope: QueueScope::Local,
        }
    }

    fn make_remote_app_stub(local_items: Vec<MediaItem>, remote_items: Vec<MediaItem>) -> App {
        use crate::api::EmbyClient;
        use crate::config::Config;

        let (remote, player_rx) = crate::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.player_tab.items = local_items;
        app.player_tab.playlist_cursor = 0;
        app
    }

    fn make_local_daemon_app_stub(remote_items: Vec<MediaItem>) -> App {
        use crate::api::EmbyClient;
        use crate::config::Config;

        let (remote, player_rx) = crate::remote_player::RemotePlayer::stub(remote_items, 0);
        App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, true)
    }

    #[test]
    fn session_direct_endpoint_prefers_advertised_tcp_port() {
        let app = make_app_stub();
        let mut sess = make_session("remote-host", "mbv");
        sess.host = "192.168.1.20".into();
        sess.supported_commands = vec![crate::api::mbv_direct_tcp_port_command(47788)];
        assert_eq!(
            app.session_direct_endpoint(&sess),
            Some(crate::remote_player::DaemonEndpoint::Tcp(
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
            Some(crate::remote_player::DaemonEndpoint::Local)
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        });
        app.tab_idx = 1;
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        });
        app.tab_idx = 1;
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        });
        app.tab_idx = 1;
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.playlist_view = PLAYLIST_VIEW_POWER;
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
        app.remote_player_tab = Some(PlayerTab {
            items: make_items(2),
            playlist_cursor: 1,
        });
        app.queue_scope = QueueScope::Remote;

        assert_eq!(app.displayed_queue_scope(), QueueScope::Local);

        app.set_queue_scope(QueueScope::Remote);
        assert_eq!(app.displayed_queue_scope(), QueueScope::Local);
        assert_eq!(app.queue_scope, QueueScope::Local);
    }

    #[test]
    fn direct_remote_play_items_keeps_local_queue_intact() {
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
        assert_eq!(app.player_tab.playlist_cursor, 0);
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
        assert_eq!(app.remote_player_tab.as_ref().unwrap().playlist_cursor, 2);
        assert!(matches!(
            app.queue_source,
            crate::config::QueueSource::Album
        ));
        assert_eq!(app.displayed_queue_scope(), QueueScope::Remote);
    }

    #[test]
    fn direct_remote_track_changes_do_not_clobber_local_last_played() {
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
    fn alt_q_enqueues_from_home_view() {
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
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items, remote_items.clone());
        app.set_queue_scope(QueueScope::Local);
        app.queue_source = crate::config::QueueSource::Album;
        app.queue_dirty = true;

        app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

        assert!(app.player_tab.items.is_empty());
        assert_eq!(app.player_tab.playlist_cursor, 0);
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
        assert!(matches!(app.queue_source, crate::config::QueueSource::Unknown));
        assert!(!app.queue_dirty);
    }

    #[test]
    fn clearing_remote_queue_in_direct_remote_mode_leaves_local_queue_metadata_intact() {
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
        let local_items = make_items(3);
        let remote_items = make_items(2);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
        app.set_queue_scope(QueueScope::Local);

        app.remove_from_playlist(1);

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
        assert_eq!(app.remote_playlist_undo_stack.len(), 0);
    }

    #[test]
    fn removing_from_remote_queue_in_direct_remote_mode_does_not_touch_local_queue() {
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());

        app.remove_from_playlist(1);

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
        assert_eq!(app.playlist_undo_stack.len(), 0);
        assert_eq!(app.remote_playlist_undo_stack.len(), 1);
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
        let local_items = make_items(2);
        let remote_items = make_items(3);
        let mut app = make_remote_app_stub(local_items, remote_items.clone());
        app.player.status.lock().unwrap().active = false;

        app.remove_from_playlist(1);

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
}
