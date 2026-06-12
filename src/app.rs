use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rand::seq::SliceRandom;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs},
};
use textwrap::wrap;

use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use crate::applog::{AppLog, Level};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerProxy};
use crate::ws::WsEvent;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogPane { Sources, Log }

#[derive(Clone)]
enum ContextAction {
    Play,
    PlayFolder(String),
    ShuffleFolder(String),
    Enqueue,
    EnqueueFolder(MediaItem),
    MarkPlayed(String),
    MarkUnplayed(String),
    RemoveFromContinueWatching,
    RemoveFromPlaylist(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MultiSelectKind { HiddenLibraries, HiddenLatest }

struct MultiSelectPopup {
    kind: MultiSelectKind,
    items: Vec<(String, String, bool)>, // (name_lower, display_name, is_hidden)
    cursor: usize,
}

struct ContextMenu {
    x: u16,
    y: u16,
    items: Vec<&'static str>,
    actions: Vec<ContextAction>,
    cursor: usize,
}

struct LibSearch {
    query: String,
    items: Vec<crate::api::MediaItem>,
    results: Vec<usize>,               // indices into items, sorted by score desc
    cursor: usize,                     // position within results
}

struct BrowseLevel {
    parent_id: String,
    title: String,
    items: Vec<MediaItem>,
    cursor: usize,
    item_types: Option<String>,
    unplayed_only: bool,
    loading: bool,
}

enum LibEvent {
    Loaded { lib_idx: usize, parent_id: String, level: BrowseLevel },
    Refreshed { lib_idx: usize, parent_id: String, items: Vec<MediaItem> },
    Error(String),
}

enum SessionEvent {
    Loaded(Vec<crate::api::SessionInfo>),
    Error(String),
}

mod palette {
    use ratatui::style::Color;
    pub const BASE:          Color = Color::Rgb(26,  26,  26);   // near-black, for text on colored bg
    pub const OVERLAY:       Color = Color::Rgb(63,  63,  63);   // gray, unfocused borders
    pub const MUTED:         Color = Color::Rgb(108, 108, 108);  // dim text, icons
    pub const SUBTLE:        Color = Color::Rgb(158, 158, 158);  // secondary text
    pub const TEXT:          Color = Color::Rgb(230, 230, 230);  // primary text
    pub const WHITE:         Color = Color::Rgb(230, 230, 230);  // near-white (#e6e6e6)
    pub const YELLOW:        Color = Color::Rgb(250, 220, 70);   // yellow — in-progress, paused
    pub const PINE:          Color = Color::Rgb(61,  139, 55);   // dark green — folders, watched
    pub const FOAM:          Color = Color::Rgb(0,   164, 220);  // emby blue — now-playing item
    pub const IRIS:          Color = Color::Rgb(82,  181, 75);   // emby green — active tab, focused
    pub const IRIS_DIM:      Color = Color::Rgb(83,  133, 80);   // seekbar downloaded-unplayed: IRIS@50% over #555555
    pub const FOCUSED:       Color = Color::Rgb(83,  83,  83);   // focused item bg (#535353)
    pub const RED:           Color = Color::Rgb(220, 60,  60);   // loud volume
}

struct PlayerTab {
    items: Vec<MediaItem>,
    playlist_cursor: usize,
}

struct HomePane {
    continue_items: Vec<MediaItem>,
    continue_cursor: usize,
    latest: Vec<(String, String, Vec<MediaItem>, usize)>, // (title, lib_id, items, cursor)
    section: usize, // 0=continue, 1..=latest
}

struct LibraryTab {
    library: MediaItem,
    nav_stack: Vec<BrowseLevel>,
    search: Option<LibSearch>,
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
    status: String,
    status_expires: Option<Instant>,
    hidden_libraries: Vec<String>,
    hidden_latest: Vec<String>,
    music_levels: Vec<String>,
    log: AppLog,
    log_scroll: usize,
    log_pane: LogPane,        // which pane has focus
    log_source_cursor: usize, // selected row in sources pane
    log_disabled_sources: std::collections::HashSet<&'static str>,
    // Layout rects from last render, used for mouse hit-testing
    playlist_rect: Rect,
    home_rect: Rect,
    layout_playlist_inner: Rect,
    layout_section_areas: Vec<Rect>,
    layout_tabs_area: Rect,
    terminal_width: u16,
    terminal_height: u16,
    layout_lib_scroll: usize,
    layout_lib_row_heights: Vec<u16>, // height of each visible row, from scroll
    layout_home_scrolls: Vec<usize>,
    layout_home_scrollbar: Rect,
    home_panel_section_offset: usize,
    layout_lib_table_area: Rect,
    layout_breadcrumbs: Vec<(u16, u16, u16, usize)>, // (x_start, x_end, row, target nav_stack len)
    last_click_time: Instant,
    last_click_pos: (u16, u16),
    last_drag_seek: Instant,
    layout_seekbar_area: Rect,
    layout_button_area: Rect,
    layout_tracks_area: Rect,
    layout_vol_area: Rect,
    layout_sub_area: Rect,
    layout_sub_indicator_area: Rect,
    layout_audio_indicator_area: Rect,
    layout_audio_area: Rect,
    layout_sessions_btn_area: Rect,
    confirm_remove_idx: Option<usize>,     // playlist index pending removal confirmation
    pending_queue_removal: Option<usize>,  // deferred removal after TrackChanged index-shifts
    confirm_clear_playlist: bool,
    skip_intro_end_ticks: Option<i64>,
    next_up_item: Option<MediaItem>,
    playlist_view: u8,
    home_card_view: bool,
    last_played_item_id: Option<String>,
    layout_carousel_slots: [(Option<usize>, Rect); 3],
    layout_carousel_left_arrow: Option<Rect>,
    layout_carousel_right_arrow: Option<Rect>,
    layout_carousel_up_arrow: Option<Rect>,
    layout_carousel_down_arrow: Option<Rect>,
    last_carousel_click_slot: Option<usize>,
    last_carousel_click_time: Instant,
    card_image_states: std::collections::HashMap<String, Option<StatefulProtocol>>,
    card_image_loading: std::collections::HashSet<String>,
    card_image_tx: mpsc::Sender<(String, Option<Vec<u8>>)>,
    card_image_rx: mpsc::Receiver<(String, Option<Vec<u8>>)>,
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
    help_scroll: u16,
    show_log_tab: bool,
    lib_tx: mpsc::Sender<LibEvent>,
    lib_rx: mpsc::Receiver<LibEvent>,
    sessions: Vec<crate::api::SessionInfo>,
    sessions_cursor: usize,
    sessions_loading: bool,
    show_sessions: bool,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
    connected_session_id: Option<String>,
    connected_session_state: Option<crate::api::SessionInfo>,
    last_session_poll: Instant,
    remote_pos_s: i64,      // monotonic position estimate for the connected remote
    remote_pos_at: Instant, // when remote_pos_s was last anchored
    force_clear: bool,
    tab_scroll: usize,
    ui_volume: u8,
    pre_mute_volume: Option<u8>,
    layout_tabbar_vol_area: Rect,
    last_scroll_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingKey {
    DaemonModeOnExit,
    StartOnQueue,
    AlwaysPlayNext,
    ConsumeVideos,
    AlwaysSkipIntro,
    ShowLogTab,
    ImageProtocol,
    HiddenLibraries,
    HiddenLatest,
    ShowAudioWindow,
    UseMpvConfig,
    NoScripts,
    ShowSysTrayIcon,
    LogOut,
}

// Sections rendered as IRIS blocks in a 2×2 grid.
// LogOut is rendered separately as a plain line below the grid.
static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
    ("[general]", &[SettingKey::DaemonModeOnExit, SettingKey::AlwaysSkipIntro, SettingKey::ShowLogTab, SettingKey::ImageProtocol, SettingKey::HiddenLibraries, SettingKey::HiddenLatest]),
    ("[queue]",   &[SettingKey::StartOnQueue, SettingKey::AlwaysPlayNext, SettingKey::ConsumeVideos]),
    ("[mpv]",       &[SettingKey::ShowAudioWindow, SettingKey::UseMpvConfig, SettingKey::NoScripts]),
    ("[daemon]",    &[SettingKey::ShowSysTrayIcon]),
    ("[actions]",   &[SettingKey::LogOut]),
];

const SESSIONS_PANEL_W: u16 = 42;
const HELP_PANEL_W:     u16 = 58;
const SETTINGS_PANEL_W: u16 = 56;

fn setting_label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::DaemonModeOnExit  => "Daemon mode on exit",
        SettingKey::StartOnQueue      => "Start on queue",
        SettingKey::AlwaysPlayNext       => "Always play next",
        SettingKey::ConsumeVideos  => "Consume videos",
        SettingKey::AlwaysSkipIntro      => "Always skip intro",
        SettingKey::ShowLogTab        => "Show log tab",
        SettingKey::ImageProtocol     => "Image protocol",
        SettingKey::HiddenLibraries   => "Hidden libraries",
        SettingKey::HiddenLatest      => "Hidden latest",
        SettingKey::ShowAudioWindow   => "Show audio window",
        SettingKey::UseMpvConfig      => "Use mpv config",
        SettingKey::NoScripts         => "No scripts",
        SettingKey::ShowSysTrayIcon   => "Show systray icon",
        SettingKey::LogOut            => "Log out",
    }
}

fn setting_value(key: SettingKey, cfg: &crate::config::Config) -> String {
    match key {
        SettingKey::DaemonModeOnExit  => bool_val(cfg.daemon_mode_on_exit),
        SettingKey::StartOnQueue      => bool_val(cfg.start_on_queue),
        SettingKey::AlwaysPlayNext       => bool_val(cfg.always_play_next),
        SettingKey::ConsumeVideos  => bool_val(cfg.consume_videos),
        SettingKey::AlwaysSkipIntro      => bool_val(cfg.always_skip_intro),
        SettingKey::ShowLogTab        => bool_val(cfg.show_log_tab),
        SettingKey::ImageProtocol     => cfg.image_protocol.clone().unwrap_or_else(|| "none".into()),
        SettingKey::HiddenLibraries   => fmt_hidden_list(&cfg.hidden_libraries),
        SettingKey::HiddenLatest      => fmt_hidden_list(&cfg.hidden_latest),
        SettingKey::ShowAudioWindow   => bool_val(cfg.show_audio_window),
        SettingKey::UseMpvConfig      => bool_val(cfg.use_mpv_config),
        SettingKey::NoScripts         => bool_val(cfg.no_scripts),
        SettingKey::ShowSysTrayIcon   => bool_val(cfg.show_systray_icon),
        SettingKey::LogOut            => String::new(),
    }
}

fn fmt_hidden_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} hidden"),
    }
}

fn bool_val(v: bool) -> String { if v { "on".into() } else { "off".into() } }

fn settings_total_rows() -> usize {
    SETTING_SECTIONS.iter().map(|(_, keys)| keys.len()).sum()
}

fn settings_cursor_to_key(cursor: usize) -> SettingKey {
    let mut idx = 0;
    for &(_, keys) in SETTING_SECTIONS {
        for &key in keys {
            if idx == cursor { return key; }
            idx += 1;
        }
    }
    SettingKey::LogOut
}

const HOME_MIN_SECTION_H: u16 = 6; // 2 border rows + 4 content rows

impl App {
    pub fn new(client: EmbyClient) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (ws_tx, ws_rx) = mpsc::channel();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) = mpsc::channel::<(String, Option<Vec<u8>>)>();
        let server_url = client.config.server_url.clone();
        let token = client.token.clone();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let show_log_tab = client.config.show_log_tab;
        let start_on_queue = client.config.start_on_queue;
        let ws_url = client.ws_url();
        let log = AppLog::new(if show_log_tab { 5000 } else { 0 });
        let ws_send_tx = crate::ws::start(ws_url, ws_tx, log.clone());
        let always_play_next = client.config.always_play_next;
        let always_skip_intro = client.config.always_skip_intro;
        let subs_off = Self::load_subs_off();
        let raw_player = Player::new(server_url, token, client.config.show_audio_window, client.config.use_mpv_config, client.config.no_scripts, always_play_next, always_skip_intro, subs_off, player_tx, Some(ws_send_tx));
        let player_status = raw_player.status.clone();
        let player_cmd_tx = raw_player.cmd_tx.clone();
        crate::mpris::start(player_status, move |cmd| {
            if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                let _ = tx.send(cmd);
            }
        });
        let player = PlayerProxy::local(raw_player, always_play_next);
        let mut client = client;
        client.probe_chapter_api(&log);
        App {
            client: Arc::new(Mutex::new(client)),
            player,
            player_rx,
            ws_rx,
            tab_idx: if start_on_queue { 1 } else { 0 },
            hidden_libraries,
            hidden_latest,
            music_levels,
            player_tab: PlayerTab { items: Vec::new(), playlist_cursor: 0 },
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            log,
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
            layout_lib_scroll: 0,
            layout_lib_row_heights: Vec::new(),
            layout_home_scrolls: Vec::new(),
            layout_home_scrollbar: Rect::default(),
            home_panel_section_offset: 0,
            layout_lib_table_area: Rect::default(),
            layout_breadcrumbs: Vec::new(),
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_click_pos: (u16::MAX, u16::MAX),
            layout_seekbar_area: Rect::default(),
            layout_button_area: Rect::default(),
            layout_tracks_area: Rect::default(),
            layout_vol_area: Rect::default(),
            layout_sub_area: Rect::default(),
            layout_sub_indicator_area: Rect::default(),
            layout_audio_indicator_area: Rect::default(),
            layout_audio_area: Rect::default(),
            layout_sessions_btn_area: Rect::default(),
            confirm_remove_idx: None,
            pending_queue_removal: None,
            confirm_clear_playlist: false,
            skip_intro_end_ticks: None,
            next_up_item: None,
            playlist_view: Self::load_playlist_view(),
            home_card_view: Self::load_home_card_view(),
            ui_volume: Self::load_ui_volume(),
            pre_mute_volume: None,
            layout_tabbar_vol_area: Rect::default(),
            last_played_item_id: None,
            layout_carousel_slots: [(None, Rect::default()); 3],
            layout_carousel_left_arrow: None,
            layout_carousel_right_arrow: None,
            layout_carousel_up_arrow: None,
            layout_carousel_down_arrow: None,
            last_carousel_click_slot: None,
            last_carousel_click_time: Instant::now() - Duration::from_secs(1),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
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
            help_scroll: 0,
            show_log_tab,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_loading: false,
            show_sessions: false,
            sessions_tx,
            sessions_rx,
            connected_session_id: None,
            connected_session_state: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            force_clear: false,
            tab_scroll: 0,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
        }
    }

    pub fn new_remote(
        client: EmbyClient,
        remote: crate::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<crate::ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel::<SessionEvent>();
        let (card_image_tx, card_image_rx) = mpsc::channel::<(String, Option<Vec<u8>>)>();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let hidden_latest = client.config.hidden_latest.clone();
        let music_levels = client.config.music_levels.clone();
        let always_play_next = client.config.always_play_next;
        let log = AppLog::new(0);
        let mut client = client;
        client.probe_chapter_api(&log);
        let initial_items = remote.items.lock().unwrap().clone();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let player = PlayerProxy::remote(remote, always_play_next);
        App {
            client: Arc::new(Mutex::new(client)),
            player,
            player_rx,
            ws_rx,
            tab_idx: 0,
            hidden_libraries,
            hidden_latest,
            music_levels,
            player_tab: PlayerTab {
                items: initial_items,
                playlist_cursor: initial_cursor,
            },
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            log,
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
            layout_lib_scroll: 0,
            layout_lib_row_heights: Vec::new(),
            layout_home_scrolls: Vec::new(),
            layout_home_scrollbar: Rect::default(),
            home_panel_section_offset: 0,
            layout_lib_table_area: Rect::default(),
            layout_breadcrumbs: Vec::new(),
            last_click_time: Instant::now(),
            last_drag_seek: Instant::now() - Duration::from_secs(1),
            last_click_pos: (u16::MAX, u16::MAX),
            layout_seekbar_area: Rect::default(),
            layout_button_area: Rect::default(),
            layout_tracks_area: Rect::default(),
            layout_vol_area: Rect::default(),
            layout_sub_area: Rect::default(),
            layout_sub_indicator_area: Rect::default(),
            layout_audio_indicator_area: Rect::default(),
            layout_audio_area: Rect::default(),
            layout_sessions_btn_area: Rect::default(),
            confirm_remove_idx: None,
            pending_queue_removal: None,
            confirm_clear_playlist: false,
            skip_intro_end_ticks: None,
            next_up_item: None,
            playlist_view: Self::load_playlist_view(),
            home_card_view: Self::load_home_card_view(),
            ui_volume: Self::load_ui_volume(),
            pre_mute_volume: None,
            layout_tabbar_vol_area: Rect::default(),
            last_played_item_id: None,
            layout_carousel_slots: [(None, Rect::default()); 3],
            layout_carousel_left_arrow: None,
            layout_carousel_right_arrow: None,
            layout_carousel_up_arrow: None,
            layout_carousel_down_arrow: None,
            last_carousel_click_slot: None,
            last_carousel_click_time: Instant::now() - Duration::from_secs(1),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
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
            help_scroll: 0,
            show_log_tab: false,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_loading: false,
            show_sessions: false,
            sessions_tx,
            sessions_rx,
            connected_session_id: None,
            connected_session_state: None,
            last_session_poll: Instant::now() - Duration::from_secs(60),
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            force_clear: false,
            tab_scroll: 0,
            last_scroll_at: Instant::now() - Duration::from_secs(1),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode.
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.client.lock().unwrap().config.image_protocol.clone();
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let proto = protocol_override.as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "sixel"      => Some(ProtocolType::Sixel),
                "kitty"      => Some(ProtocolType::Kitty),
                "iterm2"     => Some(ProtocolType::Iterm2),
                "halfblocks" => Some(ProtocolType::Halfblocks),
                _            => None, // "auto" or unknown: use picker's detected protocol
            });
        if let Some(proto) = proto {
            picker.set_protocol_type(proto);
        }
        self.image_picker = Some(picker);

        self.status = "Loading...".into();
        terminal.draw(|f| self.render(f))?;

        {
            let c = self.client.lock().unwrap();
            c.register_capabilities(&self.log);
        }

        match self.fetch_home() {
            Ok(()) => self.status.clear(),
            Err(e) => self.status = format!("Error: {e}"),
        }
        self.restore_playlist();
        terminal.draw(|f| self.render(f))?;

        'outer: loop {
            if let Ok(ev) = self.player_rx.try_recv() {
                match ev {
                    PlayerEvent::Stopped { idx, position_ticks, played } => {
                        if self.player.is_remote_disconnected() {
                            self.next_up_item = None;
                            self.skip_intro_end_ticks = None;
                            self.status = "Daemon disconnected — playback stopped".into();
                            self.refresh_after_stop();
                            continue;
                        }
                        if let Some(item) = self.player_tab.items.get_mut(idx) {
                            if played {
                                item.playback_position_ticks = 0;
                                item.played = true;
                            } else if position_ticks > 0 && !item.is_audio() {
                                item.playback_position_ticks = position_ticks;
                            }
                            self.last_played_item_id = Some(item.id.clone());
                        }
                        self.next_up_item = None;
                        self.skip_intro_end_ticks = None;
                        self.status.clear();
                        let is_video = self.player_tab.items.get(idx).map_or(false, |i| i.is_video());
                        if played && is_video && self.client.lock().unwrap().config.consume_videos {
                            if idx < self.player_tab.items.len() {
                                self.player_tab.items.remove(idx);
                                self.player_tab.playlist_cursor = self.player_tab.playlist_cursor
                                    .min(self.player_tab.items.len().saturating_sub(1));
                            }
                        }
                        self.refresh_after_stop();
                    }
                    PlayerEvent::TrackCompleted { idx, position_ticks, played } => {
                        if let Some(item) = self.player_tab.items.get_mut(idx) {
                            if played {
                                item.playback_position_ticks = 0;
                                item.played = true;
                            } else if position_ticks >= 300_000_000 && !item.is_audio() {
                                // Only update local position for meaningful progress (≥ 30 s).
                                // Startup noise from mpv (< 30 s) keeps the previous value intact.
                                item.playback_position_ticks = position_ticks;
                            }
                        }
                        let is_video = self.player_tab.items.get(idx).map_or(false, |i| i.is_video());
                        let runtime  = self.player_tab.items.get(idx).map_or(0, |i| i.runtime_ticks);
                        let consumed_enough = played
                            || runtime == 0
                            || position_ticks * 20 >= runtime;
                        if is_video && consumed_enough && self.client.lock().unwrap().config.consume_videos {
                            self.pending_queue_removal = Some(idx);
                        }
                    }
                    PlayerEvent::TrackChanged(idx) => {
                        self.skip_intro_end_ticks = None;
                        let adjusted = if let Some(remove_idx) = self.pending_queue_removal.take() {
                            if remove_idx < self.player_tab.items.len() {
                                self.player_tab.items.remove(remove_idx);
                            }
                            self.player.send_command(PlayerCommand::PlaylistRemove(remove_idx));
                            if remove_idx < idx { idx - 1 } else { idx }
                        } else {
                            idx
                        };
                        self.player_tab.playlist_cursor = adjusted;
                        if let Some(item) = self.player_tab.items.get(adjusted) {
                            self.last_played_item_id = Some(item.id.clone());
                        }
                    }
                    PlayerEvent::PlaylistNextUp { next_idx } => {
                        if let Some(item) = self.player_tab.items.get(next_idx) {
                            let item_id    = item.id.clone();
                            let show_title = item.series_name.clone();
                            let ep_title   = item.name.clone();
                            self.next_up_item = Some(item.clone());
                            // Daemon sends NextUpShow to mpv directly; only send from local player.
                            if !self.player.is_remote() {
                                self.player.send_command(PlayerCommand::NextUpShow { item_id, show_title, ep_title });
                            }
                        }
                    }
                    PlayerEvent::NextUpThreshold { .. } => {
                        // Series episodes now use play_playlist; this only fires for movies
                        // (always_play_next=false or non-series content). No action needed.
                    }
                    PlayerEvent::NextUpPlay => {
                        self.log.push(Level::Warn, "app", "next-up: play triggered");
                        if let Some(item) = self.next_up_item.take() {
                            let label = item.playback_label();
                            if let Some(idx) = self.player_tab.items.iter().position(|i| i.id == item.id) {
                                self.player.send_command(PlayerCommand::JumpTo(idx));
                                self.player_tab.playlist_cursor = idx;
                                self.flash_status(label);
                            } else {
                                self.log.push(Level::Warn, "app", "next-up: item not in queue, cannot jump");
                            }
                        } else {
                            self.log.push(Level::Warn, "app", "next-up: NextUpPlay fired but next_up_item is None");
                        }
                    }
                    PlayerEvent::QueueUpdated { items, cursor } => {
                        self.player_tab.items = items;
                        self.player_tab.playlist_cursor = cursor;
                    }
                    PlayerEvent::IntroStarted { intro_end_ticks } => {
                        self.skip_intro_end_ticks = Some(intro_end_ticks);
                        self.status = "Skip intro? [Y/n]".into();
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
                }
            }

            while let Ok(ev) = self.lib_rx.try_recv() {
                self.handle_lib_event(ev);
            }

            while let Ok(ev) = self.sessions_rx.try_recv() {
                match ev {
                    SessionEvent::Loaded(sessions) => {
                        let old_id = self.sessions.get(self.sessions_cursor).map(|s| s.id.clone());
                        self.sessions = sessions;
                        self.sessions_loading = false;
                        self.last_session_poll = Instant::now();
                        if let Some(id) = old_id {
                            if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
                                self.sessions_cursor = pos;
                            } else {
                                self.sessions_cursor = self.sessions_cursor.min(self.sessions.len().saturating_sub(1));
                                if !self.sessions.is_empty() {
                                    self.log.push(Level::Warn, "sessions", "selected session gone; cursor clamped");
                                }
                            }
                        }
                        // Update connected session state; auto-disconnect if gone
                        if let Some(ref conn_id) = self.connected_session_id.clone() {
                            if let Some(s) = self.sessions.iter().find(|s| &s.id == conn_id) {
                                // Maintain a monotonic position estimate within a single video.
                                // Reset the anchor when the item changes (different runtime or
                                // title) so the new video's position isn't clamped by the old one.
                                let now = Instant::now();
                                let prev_runtime = self.connected_session_state
                                    .as_ref().map(|p| p.runtime_s).unwrap_or(0);
                                let prev_title = self.connected_session_state
                                    .as_ref().and_then(|p| p.now_playing.clone());
                                let item_changed = s.runtime_s != prev_runtime
                                    || s.now_playing != prev_title;
                                if item_changed || s.is_paused {
                                    self.remote_pos_s = s.position_s;
                                } else {
                                    let extrapolated = self.remote_pos_s
                                        + self.remote_pos_at.elapsed().as_secs() as i64;
                                    self.remote_pos_s = s.position_s.max(extrapolated);
                                }
                                self.remote_pos_at = now;
                                self.connected_session_state = Some(s.clone());
                                // Remote hasn't started playing yet — repoll sooner
                                if s.runtime_s == 0 {
                                    self.last_session_poll = Instant::now() - Duration::from_millis(500);
                                }
                            } else {
                                self.log.push(Level::Warn, "sessions", "connected session gone; disconnecting");
                                self.flash_status("Remote session ended; disconnected".to_string());
                                self.connected_session_id = None;
                                self.connected_session_state = None;
                                self.remote_pos_s = 0;
                            }
                        }
                    }
                    SessionEvent::Error(e) => {
                        self.sessions_loading = false;
                        self.flash_status(format!("Sessions error: {e}"));
                    }
                }
            }

            while let Ok((item_id, bytes_opt)) = self.card_image_rx.try_recv() {
                self.card_image_loading.remove(&item_id);
                let state: Option<StatefulProtocol> = bytes_opt
                    .and_then(|b| image::load_from_memory(&b).ok())
                    .and_then(|dyn_img| {
                        self.image_picker.as_ref().map(|p| p.new_resize_protocol(dyn_img))
                    });
                self.card_image_states.insert(item_id, state);
            }

            while let Ok(ev) = self.ws_rx.try_recv() {
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

            if event::poll(Duration::from_millis(50))? {
                let ev = event::read()?;
                let is_home_card_nav = self.home_card_view && self.tab_idx == 0;
                match ev {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press { continue; }
                        let nav_code = is_home_card_nav
                            && matches!(key.code, KeyCode::Left | KeyCode::Right);
                        if self.handle_key(key) { break; }
                        // Drain queued duplicate nav keys to prevent scroll backlog.
                        if nav_code {
                            while event::poll(Duration::ZERO)? {
                                match event::read()? {
                                    Event::Key(k) if k.kind == KeyEventKind::Press
                                        && k.code == key.code => {}
                                    other => {
                                        match other {
                                            Event::Key(k) if k.kind == KeyEventKind::Press => {
                                                if self.handle_key(k) { break 'outer; }
                                            }
                                            Event::Mouse(m) => self.handle_mouse(m),
                                            _ => {}
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        let nav_scroll = is_home_card_nav
                            && matches!(mouse.kind,
                                crossterm::event::MouseEventKind::ScrollUp |
                                crossterm::event::MouseEventKind::ScrollDown)
                            && self.home_rect.contains((mouse.column, mouse.row).into());
                        self.handle_mouse(mouse);
                        // Drain queued scroll events to prevent scroll backlog.
                        if nav_scroll {
                            while event::poll(Duration::ZERO)? {
                                match event::read()? {
                                    Event::Mouse(m) if matches!(m.kind,
                                        crossterm::event::MouseEventKind::ScrollUp |
                                        crossterm::event::MouseEventKind::ScrollDown) => {}
                                    other => {
                                        match other {
                                            Event::Key(k) if k.kind == KeyEventKind::Press => {
                                                if self.handle_key(k) { break 'outer; }
                                            }
                                            Event::Mouse(m) => self.handle_mouse(m),
                                            _ => {}
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if self.force_clear {
                self.force_clear = false;
                terminal.clear()?;
            }
            terminal.draw(|f| self.render(f))?;
        }

        // Leave the daemon's player running when the TUI disconnects; only
        // stop the player when we own it locally.
        let (was_playing, current_idx, position_ticks) = {
            let st = self.player.status.lock().unwrap();
            (st.active, st.current_idx, st.position_ticks)
        };
        if !self.player.is_remote() {
            self.player.stop();
        }
        self.player.join();
        // Update the playing item's position before saving — the PlayerEvent::Stopped
        // that carries this update is never processed after we break out of the event loop.
        if was_playing {
            if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                if position_ticks > 0 && !item.is_audio() {
                    item.playback_position_ticks = position_ticks;
                }
                self.last_played_item_id = Some(item.id.clone());
            }
        }
        self.save_playlist(was_playing);
        restore_terminal(terminal)?;
        Ok(())
    }

    fn tab_count(&self) -> usize { 2 + self.libs.len() + if self.show_log_tab { 1 } else { 0 } }
    fn log_tab_idx(&self) -> usize { 2 + self.libs.len() }
    fn lib_tab_offset(&self) -> usize { 2 }

    // ── key handling ────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.show_settings {
            if self.multiselect_popup.is_some() {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => { self.close_multiselect_popup(); }
                    KeyCode::Up => {
                        if let Some(p) = &mut self.multiselect_popup {
                            if p.cursor > 0 { p.cursor -= 1; }
                        }
                    }
                    KeyCode::Down => {
                        if let Some(p) = &mut self.multiselect_popup {
                            if p.cursor + 1 < p.items.len() { p.cursor += 1; }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if let Some(p) = &mut self.multiselect_popup {
                            let i = p.cursor;
                            p.items[i].2 = !p.items[i].2;
                        }
                    }
                    _ => {}
                }
                return false;
            }
            if self.confirm_logout {
                if matches!(key.code, KeyCode::Char('y')) {
                    crate::api::clear_cached_token();
                    self.confirm_logout = false;
                    self.show_settings = false;
                    self.flash_status("Logged out — restart mbv to sign in again".into());
                } else {
                    self.confirm_logout = false;
                }
                return false;
            }
            match key.code {
                KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
                KeyCode::Esc => { self.close_settings(); }
                KeyCode::F(1) => { self.close_settings(); self.show_help = true; }
                KeyCode::F(3) => { self.close_settings(); self.show_sessions = true; }
                KeyCode::Up => {
                    if self.settings_cursor > 0 {
                        self.settings_cursor -= 1;
                        self.settings_scroll_follow();
                    }
                }
                KeyCode::Down => {
                    if self.settings_cursor + 1 < settings_total_rows() {
                        self.settings_cursor += 1;
                        self.settings_scroll_follow();
                    }
                }
                KeyCode::PageUp => {
                    self.settings_scroll = self.settings_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.settings_scroll += 10;
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter => {
                    self.handle_settings_activate();
                }
                _ => {}
            }
            return false;
        }
        if self.show_help {
            match key.code {
                KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
                KeyCode::Esc | KeyCode::F(1) => { self.show_help = false; }
                KeyCode::F(2) => { self.show_help = false; self.show_settings = true; }
                KeyCode::F(3) => { self.show_help = false; self.show_sessions = true; }
                KeyCode::Up       => { self.help_scroll = self.help_scroll.saturating_sub(1); }
                KeyCode::Down     => { self.help_scroll += 1; }
                KeyCode::PageUp   => { self.help_scroll = self.help_scroll.saturating_sub(10); }
                KeyCode::PageDown => { self.help_scroll += 10; }
                KeyCode::Home     => { self.help_scroll = 0; }
                _ => {}
            }
            return false;
        }
        if self.show_sessions {
            match key.code {
                KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
                KeyCode::Esc => { self.show_sessions = false; }
                KeyCode::F(1) => { self.show_sessions = false; self.show_help = true; }
                KeyCode::F(2) => { self.show_sessions = false; self.show_settings = true; }
                KeyCode::Up => {
                    self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
                }
                KeyCode::Down => {
                    if !self.sessions.is_empty() {
                        self.sessions_cursor = (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                    }
                }
                KeyCode::Char('r') => { self.spawn_sessions_load(); }
                KeyCode::Enter => {
                    if let Some(sess) = self.sessions.get(self.sessions_cursor) {
                        let id = sess.id.clone();
                        let name = sess.device_name.clone();
                        self.connected_session_id = Some(id);
                        self.connected_session_state = Some(sess.clone());
                        self.remote_pos_s = sess.position_s;
                        self.remote_pos_at = Instant::now();
                        self.show_sessions = false;
                        self.flash_status(format!("Connected to {name}"));
                        self.spawn_sessions_load();
                    }
                }
                KeyCode::Char('d') => {
                    self.connected_session_id = None;
                    self.connected_session_state = None;
                    self.remote_pos_s = 0;
                    self.show_sessions = false;
                    self.flash_status("Disconnected from remote session".to_string());
                }
                _ => {}
            }
            return false;
        }
        // When library search is active, unmodified keys feed the search; Alt-shortcuts pass through
        if self.tab_idx > 1
            && self.tab_idx != self.log_tab_idx()
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.libs.get(self.tab_idx - self.lib_tab_offset()).is_some_and(|l| l.search.is_some())
        {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let alt = key.modifiers.contains(KeyModifiers::ALT);
            match key.code {
                KeyCode::Esc => { self.libs[lib_idx].search = None; }
                KeyCode::Backspace => {
                    let empty = self.libs[lib_idx].search.as_ref().is_none_or(|s| s.query.is_empty());
                    if empty { self.libs[lib_idx].search = None; }
                    else {
                        self.libs[lib_idx].search.as_mut().unwrap().query.pop();
                        self.update_lib_search(lib_idx);
                    }
                }
                KeyCode::Up       => self.move_lib_cursor(-1),
                KeyCode::Down     => self.move_lib_cursor(1),
                KeyCode::PageUp   => { let p = self.lib_page_size(); self.move_lib_cursor(-(p as i64)); }
                KeyCode::PageDown => { let p = self.lib_page_size(); self.move_lib_cursor(p as i64); }
                KeyCode::Home     => self.jump_lib_cursor(false),
                KeyCode::End      => self.jump_lib_cursor(true),
                KeyCode::Enter => self.select(),
                KeyCode::Char(c) if !alt => {
                    self.libs[lib_idx].search.as_mut().unwrap().query.push(c);
                    self.update_lib_search(lib_idx);
                }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::F(1) {
            self.show_help = true;
            return false;
        }
        if key.code == KeyCode::F(2) {
            self.show_settings = !self.show_settings;
            return false;
        }
        if key.code == KeyCode::F(3) {
            self.show_sessions = true;
            self.spawn_sessions_load();
            return false;
        }
        // Global c: clear playlist (not when typing in library search)
        let in_lib_search = self.tab_idx > 1
            && self.tab_idx != self.log_tab_idx()
            && self.libs.get(self.tab_idx - self.lib_tab_offset()).is_some_and(|l| l.search.is_some());
        if self.confirm_clear_playlist {
            self.confirm_clear_playlist = false;
            if matches!(key.code, KeyCode::Char('y')) {
                self.player.stop();
                self.player_tab.items.clear();
                self.player_tab.playlist_cursor = 0;
                self.flash_status("Playlist cleared".into());
            } else {
                self.status.clear();
            }
            return false;
        }
        if self.skip_intro_end_ticks.is_some() {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                    let secs = end_ticks as f64 / crate::api::TICKS_PER_SECOND as f64;
                    self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                    self.status.clear();
                }
            } else {
                self.skip_intro_end_ticks = None;
                self.status.clear();
            }
            return false;
        }
        if self.tab_idx != self.log_tab_idx() {
            if key.code == KeyCode::Char('c') && !key.modifiers.contains(KeyModifiers::ALT) && !in_lib_search {
                if self.player_tab.items.is_empty() { return false; }
                self.status = "Clear playlist? (y/N)".into();
                self.confirm_clear_playlist = true;
                return false;
            }
        }
        if self.tab_idx != self.log_tab_idx() {
            if let Some(quit) = self.handle_playback_key(key) { return quit; }
        }
        // Context menu intercepts all keys while open
        if self.context_menu.is_some() {
            match key.code {
                KeyCode::Esc => { self.context_menu = None; self.force_clear = true; }
                KeyCode::Up   => {
                    if let Some(m) = &mut self.context_menu {
                        if m.cursor > 0 { m.cursor -= 1; }
                    }
                }
                KeyCode::Down => {
                    if let Some(m) = &mut self.context_menu {
                        if m.cursor + 1 < m.items.len() { m.cursor += 1; }
                    }
                }
                KeyCode::Enter => {
                    if let Some(m) = self.context_menu.take() {
                        self.force_clear = true;
                        let action = m.actions.get(m.cursor).cloned();
                        self.execute_context_action(action);
                    }
                }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.force_clear = true;
            return false;
        }
        if key.code == KeyCode::F(5) {
            self.refresh_current_view();
            return false;
        }
        if self.tab_idx == 0 { return self.handle_combined_key(key); }
        if self.tab_idx == 1 { return self.handle_playlist_key(key); }
        if self.tab_idx == self.log_tab_idx() { return self.handle_log_key(key); }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
            KeyCode::Tab => { let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); }
            KeyCode::BackTab => { let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); }
            KeyCode::Esc | KeyCode::Backspace => self.go_back(),
            KeyCode::Up       => self.move_lib_cursor(-1),
            KeyCode::Down     => self.move_lib_cursor(1),
            KeyCode::PageUp   => { let p = self.lib_page_size(); self.move_lib_cursor(-(p as i64)); }
            KeyCode::PageDown => { let p = self.lib_page_size(); self.move_lib_cursor(p as i64); }
            KeyCode::Home     => self.jump_lib_cursor(false),
            KeyCode::End      => self.jump_lib_cursor(true),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let item = self.current_lib_item();
                if let Some(item) = item {
                    if item.is_folder { self.play_folder(&item.id.clone()); }
                    else { self.select(); }
                }
            }
            KeyCode::Enter => self.select(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_watched(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.shuffle_play(),
            KeyCode::Char('o') if alt => self.open_context_menu(),
            KeyCode::Char('o') if !alt => self.open_context_menu(),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            KeyCode::Char('/') => {
                let items = self.libs[lib_idx].nav_stack.last()
                    .map(|l| l.items.clone())
                    .unwrap_or_default();
                let n = items.len();
                self.libs[lib_idx].search = Some(LibSearch {
                    query: String::new(),
                    items,
                    results: (0..n).collect(),
                    cursor: 0,
                });
                self.update_lib_search(lib_idx);
            }
            _ => {}
        }
        false
    }

    fn handle_combined_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
            KeyCode::Tab => {
                let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); return false;
            }
            KeyCode::BackTab => {
                let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); return false;
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + n - 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() { self.force_clear = true; }
                return false;
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() { self.force_clear = true; }
                return false;
            }
            KeyCode::Char('v') => {
                if self.images_enabled() {
                    self.home_card_view = !self.home_card_view;
                    self.save_home_card_view();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                }
                return false;
            }
            KeyCode::Char('o') => {
                self.open_context_menu(); return false;
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
                return false;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Up => {
                if self.home_card_view {
                    let n = 1 + self.home.latest.len();
                    self.home.section = (self.home.section + n - 1) % n;
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                } else {
                    self.move_home_cursor(-1);
                }
            }
            KeyCode::Down => {
                if self.home_card_view {
                    let n = 1 + self.home.latest.len();
                    self.home.section = (self.home.section + 1) % n;
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                } else {
                    self.move_home_cursor(1);
                }
            }
            KeyCode::Left  => { if self.home_card_view { self.move_home_cursor(-1); } }
            KeyCode::Right => { if self.home_card_view { self.move_home_cursor(1); } }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            KeyCode::Enter => self.select_home(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_watched_home(),
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            _ => {}
        }
        false
    }

    fn adjust_volume(&mut self, delta: i64) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let vol = self.connected_session_state.as_ref().map(|s| s.volume).unwrap_or(50);
            let new_vol = (vol + delta).clamp(0, 100);
            let id = conn_id.clone();
            self.do_session_command(move |c| c.session_set_volume(&id, new_vol));
            return;
        }
        let active = self.player.status.lock().unwrap().active;
        if active {
            let st = self.player.status.lock().unwrap();
            let v = (st.volume as i64 + delta).clamp(0, st.volume_max as i64) as u8;
            drop(st);
            self.player.send_command(PlayerCommand::SetVolume(v as i64));
            self.ui_volume = v;
        } else {
            self.ui_volume = (self.ui_volume as i64 + delta).clamp(0, 200) as u8;
        }
        self.save_prefs();
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
        let active = self.player.status.lock().unwrap().active;
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        // Route playback controls to the connected remote session when active
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let pos_s = self.connected_session_state.as_ref().map(|s| s.position_s).unwrap_or(0);
            let id = conn_id.clone();
            match key.code {
                KeyCode::Char(' ') => {
                    self.do_session_command(move |c| c.session_transport(&id, "PlayPause"));
                    return Some(false);
                }
                KeyCode::Enter if alt => {
                    self.do_session_command(move |c| c.session_transport(&id, "Stop"));
                    return Some(false);
                }
                KeyCode::Left if key.modifiers == KeyModifiers::ALT => {
                    let ticks = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Right if key.modifiers == KeyModifiers::ALT => {
                    let ticks = (pos_s + 5) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('<') => {
                    let ticks = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('>') => {
                    let ticks = (pos_s + 5) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('z') => {
                    self.cycle_sub();
                    return Some(false);
                }
                // +/- fall through to adjust_volume which handles remote routing
                _ => {}
            }
        }
        // Volume and subtitle preference work regardless of playback state
        match key.code {
            KeyCode::Char('-') => { self.adjust_volume(-5); return Some(false); }
            KeyCode::Char('+') | KeyCode::Char('=') => { self.adjust_volume(5); return Some(false); }
            KeyCode::Char('z') => { self.toggle_sub(); return Some(false); }
            _ => {}
        }
        if !active { return None; }
        match key.code {
            KeyCode::Enter if alt => { self.player.stop(); Some(false) }
            KeyCode::Char(' ') => { self.player.send_command(PlayerCommand::TogglePause); Some(false) }
            KeyCode::Left  if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(-5.0)); Some(false) }
            KeyCode::Right if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(5.0));  Some(false) }
            KeyCode::Char('<') => { self.player.send_command(PlayerCommand::Seek(-5.0)); Some(false) }
            KeyCode::Char('>') => { self.player.send_command(PlayerCommand::Seek(5.0));  Some(false) }
            KeyCode::Char('a') if alt => { if self.is_audio_item() { self.toggle_mute(); } else { self.cycle_audio(); } Some(false) }
            KeyCode::Char('a') if !alt => { if self.is_audio_item() { self.toggle_mute(); } else { self.cycle_audio(); } Some(false) }
            _ => None,
        }
    }

    fn handle_playlist_key(&mut self, key: KeyEvent) -> bool {
        // Confirmation dialog for removing a playing item
        if let Some(t) = self.confirm_remove_idx {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                self.player.stop();
                self.player_tab.items.remove(t);
                self.player_tab.playlist_cursor =
                    if self.player_tab.items.is_empty() { 0 }
                    else { t.min(self.player_tab.items.len() - 1) };
            }
            self.confirm_remove_idx = None;
            return false;
        }

        match key.code {
            KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
            KeyCode::Tab => { let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); }
            KeyCode::BackTab => { let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); }
            KeyCode::Up | KeyCode::Left
                if self.player_tab.playlist_cursor > 0 && (key.code == KeyCode::Up || self.playlist_view == 1) => {
                    self.player_tab.playlist_cursor -= 1;
                }
            KeyCode::Down | KeyCode::Right
                if self.player_tab.playlist_cursor + 1 < self.player_tab.items.len()
                && (key.code == KeyCode::Down || self.playlist_view == 1) => {
                    self.player_tab.playlist_cursor += 1;
                }
            KeyCode::PageUp => {
                let p = self.playlist_page_size();
                self.player_tab.playlist_cursor = self.player_tab.playlist_cursor.saturating_sub(p);
            }
            KeyCode::PageDown => {
                let p = self.playlist_page_size();
                let n = self.player_tab.items.len();
                self.player_tab.playlist_cursor = (self.player_tab.playlist_cursor + p).min(n.saturating_sub(1));
            }
            KeyCode::Home => {
                self.player_tab.playlist_cursor = 0;
            }
            KeyCode::End => {
                let n = self.player_tab.items.len();
                if n > 0 { self.player_tab.playlist_cursor = n - 1; }
            }
            KeyCode::Enter => {
                let t = self.player_tab.playlist_cursor;
                let n = self.player_tab.items.len();
                if t < n {
                    if let Some(ref conn_id) = self.connected_session_id.clone() {
                        let item = self.player_tab.items[t].clone();
                        let id = conn_id.clone();
                        let item_id = item.id.clone();
                        let start_ticks = item.playback_position_ticks;
                        let label = item.playback_label();
                        self.flash_status(format!("Playing on remote: {label}"));
                        self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
                    } else {
                        let st = self.player.status.lock().unwrap();
                        let active = st.active;
                        let current_idx = st.current_idx;
                        drop(st);
                        if active {
                            if t == current_idx {
                                self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                            } else {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        } else if !self.player_tab.items.is_empty() {
                            let items = self.player_tab.items.clone();
                            let c = Arc::new(self.client.lock().unwrap().clone());
                            self.player.play_playlist(items, t, c, self.log.clone(), self.ui_volume);
                        }
                    }
                }
            }
            KeyCode::Delete => {
                let t = self.player_tab.playlist_cursor;
                if t < self.player_tab.items.len() { self.remove_from_playlist(t); }
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            KeyCode::Char('o') => {
                self.open_context_menu();
            }
            KeyCode::Char('v') => {
                self.playlist_view = (self.playlist_view + 1) % 3;
                self.save_playlist_view();
                if !self.card_image_states.is_empty() { self.force_clear = true; }
            }
            KeyCode::Char('.') => {
                let s = self.player.status.lock().unwrap();
                if s.active {
                    self.player_tab.playlist_cursor = s.current_idx;
                } else {
                    drop(s);
                    self.flash_status("Nothing is playing".into());
                }
            }
            _ => {}
        }
        false
    }

    fn handle_log_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
            KeyCode::Tab | KeyCode::BackTab => { self.set_tab(0); }
            KeyCode::Left  => { self.log_pane = LogPane::Sources; }
            KeyCode::Right => { self.log_pane = LogPane::Log; }
            KeyCode::Up => {
                match self.log_pane {
                    LogPane::Log     => { self.log_scroll += 1; }
                    LogPane::Sources => { self.log_source_cursor = self.log_source_cursor.saturating_sub(1); }
                }
            }
            KeyCode::Down => {
                match self.log_pane {
                    LogPane::Log     => { self.log_scroll = self.log_scroll.saturating_sub(1); }
                    LogPane::Sources => { self.log_source_cursor += 1; }
                }
            }
            KeyCode::PageUp   => { self.log_scroll += 20; }
            KeyCode::PageDown => { self.log_scroll = self.log_scroll.saturating_sub(20); }
            KeyCode::Char(' ') => {
                // Toggle selected source on/off
                let sources = self.log_sources();
                if let Some(src) = sources.get(self.log_source_cursor) {
                    if self.log_disabled_sources.contains(src) {
                        self.log_disabled_sources.remove(src);
                    } else {
                        self.log_disabled_sources.insert(src);
                    }
                }
            }
            KeyCode::Char('c') => {
                let entries = self.visible_log_entries();
                let text = entries.iter()
                    .map(|e| format!("{}│{}│{}", e.level.label(), e.source, e.msg))
                    .collect::<Vec<_>>().join("\n");
                let n = entries.len();
                let copied = std::process::Command::new("wl-copy")
                    .arg(&text).status().map(|s| s.success()).unwrap_or(false)
                    || std::process::Command::new("xclip")
                        .args(["-selection", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut c| {
                            use std::io::Write;
                            c.stdin.take().unwrap().write_all(text.as_bytes())?;
                            c.wait()
                        })
                        .map(|s| s.success()).unwrap_or(false);
                if copied { self.flash_status(format!("Copied {n} log lines to clipboard")); }
                else      { self.flash_status("Copy failed — wl-copy/xclip not found".into()); }
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            _ => {}
        }
        false
    }

    fn log_sources(&self) -> Vec<&'static str> {
        let mut seen = std::collections::HashSet::new();
        let mut sources: Vec<&'static str> = Vec::new();
        for e in &self.log.snapshot() {
            if seen.insert(e.source) { sources.push(e.source); }
        }
        sources.sort_unstable();
        sources
    }

    fn visible_log_entries(&self) -> Vec<crate::applog::LogEntry> {
        self.log.snapshot().into_iter()
            .filter(|e| !self.log_disabled_sources.contains(e.source))
            .collect()
    }

    // ── mouse ────────────────────────────────────────────────────────────────

    // Returns (first_visible_idx, end_exclusive) of tabs that fit in avail_w.
    fn visible_tab_range(&self, avail_w: u16) -> (usize, usize) {
        let widths = self.tab_title_widths();
        let n = widths.len();
        let start = self.tab_scroll.min(if n > 0 { n - 1 } else { 0 });
        let left_w: u16 = if start > 0 { 2 } else { 0 }; // "< "
        let mut budget = avail_w.saturating_sub(left_w);
        let mut end = start;
        while end < n {
            let _is_last_in_list = end + 1 == n;
            let tab_w: u16 = widths[end] + 2; // no divider
            let right_w: u16 = if end + 1 < n { 2 } else { 0 }; // " >"
            if budget < tab_w + right_w && end > start { break; }
            budget = budget.saturating_sub(tab_w);
            end += 1;
        }
        (start, end)
    }

    fn ensure_tab_visible(&mut self) {
        let n = self.tab_count();
        if n == 0 { return; }
        if self.tab_idx < self.tab_scroll {
            self.tab_scroll = self.tab_idx;
            return;
        }
        const RIGHT_W: u16 = 14 + 1 + 2; // VOL_W + GAP + SETTINGS_W, must match render
        let tab_w = self.terminal_width.saturating_sub(RIGHT_W);
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if self.tab_idx < end { break; }
            self.tab_scroll += 1;
        }
    }

    fn tab_title_widths(&self) -> Vec<u16> {
        // Each title has 1 space each side via the span format " name ".
        let pad: u16 = 2;
        let mut w = vec![
            "Home".chars().count() as u16 + pad,
            "Queue".chars().count() as u16 + pad,
        ];
        for l in &self.libs {
            w.push(l.library.name.chars().count() as u16 + pad);
        }
        if self.show_log_tab {
            w.push("Log".chars().count() as u16 + pad);
        }
        w
    }

    fn tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout_tabs_area;
        if col < area.x || col >= area.x + area.width { return None; }
        let rel = col - area.x;
        let (vis_start, vis_end) = self.visible_tab_range(area.width);
        let has_left  = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let left_w:  u16 = if has_left  { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        // Clicks on scroll indicators scroll the tab bar
        if has_left && rel < left_w  { return Some(usize::MAX - 1); } // sentinel: scroll left
        if has_right && rel >= area.width - right_w { return Some(usize::MAX); } // sentinel: scroll right
        let rel = rel - left_w;
        let widths = self.tab_title_widths();
        let pad = 1u16;
        let mut x = 0u16;
        for i in vis_start..vis_end {
            let w = widths[i];
            let end = x + pad + w + pad; // no divider
            if rel < end { return Some(i); }
            x = end;
        }
        None
    }

    fn handle_button_click(&mut self, btn: usize) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let pos_s = self.connected_session_state.as_ref().map(|s| s.position_s).unwrap_or(0);
            let id = conn_id.clone();
            match btn {
                1 => { let t = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND; self.do_session_command(move |c| c.session_seek(&id, t)); }
                2 => { self.do_session_command(move |c| c.session_transport(&id, "PlayPause")); }
                3 => { self.do_session_command(move |c| c.session_transport(&id, "Stop")); }
                4 => { let t = (pos_s + 5) * crate::api::TICKS_PER_SECOND; self.do_session_command(move |c| c.session_seek(&id, t)); }
                _ => {}
            }
            return;
        }
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        match btn {
            0 if active && current_idx > 0 => { self.player.send_command(PlayerCommand::JumpTo(current_idx - 1)); }
            1 => { self.player.send_command(PlayerCommand::Seek(-5.0)); }
            2 => { self.player.send_command(PlayerCommand::TogglePause); }
            3 => { self.player.stop(); }
            4 => { self.player.send_command(PlayerCommand::Seek(5.0)); }
            5 if active && current_idx + 1 < self.player_tab.items.len() => { self.player.send_command(PlayerCommand::JumpTo(current_idx + 1)); }
            _ => {}
        }
    }

    fn open_context_menu(&mut self) {
        let mut items: Vec<&'static str> = vec![];
        let mut actions: Vec<ContextAction> = vec![];

        let current_item = if self.tab_idx == 0 {
            self.current_home_item()
        } else if self.tab_idx == 1 {
            self.player_tab.items.get(self.player_tab.playlist_cursor).cloned()
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            self.current_lib_item()
        } else {
            None
        };

        if let Some(ref item) = current_item {
            if item.is_folder {
                items.push("Play All");
                actions.push(ContextAction::PlayFolder(item.id.clone()));
                items.push("Shuffle");
                actions.push(ContextAction::ShuffleFolder(item.id.clone()));
                items.push("Add to Queue");
                actions.push(ContextAction::EnqueueFolder(item.clone()));
                items.push("Mark Watched");
                actions.push(ContextAction::MarkPlayed(item.id.clone()));
                items.push("Mark Unwatched");
                actions.push(ContextAction::MarkUnplayed(item.id.clone()));
            } else {
                items.push("Play");
                actions.push(ContextAction::Play);
                if self.tab_idx != 1 {
                    items.push("Add to Queue");
                    actions.push(ContextAction::Enqueue);
                }
                let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
                if !is_audio {
                    if item.played {
                        items.push("Mark Unwatched");
                        actions.push(ContextAction::MarkUnplayed(item.id.clone()));
                    } else {
                        items.push("Mark Watched");
                        actions.push(ContextAction::MarkPlayed(item.id.clone()));
                    }
                }
                if self.tab_idx == 0 && self.home.section == 0 {
                    items.push("Remove from Continue Watching");
                    actions.push(ContextAction::RemoveFromContinueWatching);
                }
                if self.tab_idx == 1 {
                    items.push("Remove from Playlist");
                    actions.push(ContextAction::RemoveFromPlaylist(self.player_tab.playlist_cursor));
                }
            }
        }

        if items.is_empty() { return; }

        let (x, y) = self.context_menu_spawn_point();
        self.context_menu = Some(ContextMenu { x, y, items, actions, cursor: 0 });
    }

    fn open_context_menu_at(&mut self, x: u16, y: u16) {
        self.open_context_menu();
        if let Some(ref mut menu) = self.context_menu {
            menu.x = x;
            menu.y = y;
        }
    }

    fn context_menu_spawn_point(&self) -> (u16, u16) {
        if (self.tab_idx == 0 && self.home_card_view) || (self.tab_idx == 1 && self.playlist_view == 1) {
            let center = self.layout_carousel_slots[1].1;
            return (center.x + center.width / 2, center.y + center.height / 2);
        }
        if self.tab_idx == 0 {
            let sec = self.home.section;
            if let Some(area) = self.layout_section_areas.get(sec) {
                let scroll = self.layout_home_scrolls.get(sec).copied().unwrap_or(0);
                let cursor = match sec {
                    0 => self.home.continue_cursor,
                    n => self.home.latest.get(n - 1).map(|(_, _, _, c)| *c).unwrap_or(0),
                };
                let row = cursor.saturating_sub(scroll) as u16;
                return (self.terminal_width / 2, area.y + 1 + row);
            }
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &self.libs[lib_idx];
            let cursor = lib.nav_stack.last().map(|lvl| {
                lib.search.as_ref()
                    .and_then(|s| s.results.get(s.cursor).copied())
                    .unwrap_or(lvl.cursor)
            }).unwrap_or(0);
            let scroll = self.layout_lib_scroll;
            let row = cursor.saturating_sub(scroll) as u16;
            let tbl = self.layout_lib_table_area;
            return (self.terminal_width / 2, tbl.y + row * 3);
        }
        (4, 4)
    }

    fn load_prefs() -> serde_json::Value {
        let path = crate::config::prefs_path();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_default()
    }

    fn load_playlist_view() -> u8 {
        let prefs = Self::load_prefs();
        if let Some(v) = prefs["playlist_view"].as_u64() {
            return v.min(2) as u8;
        }
        prefs["playlist_card_view"].as_bool().unwrap_or(false) as u8
    }

    fn load_home_card_view() -> bool {
        Self::load_prefs()["home_card_view"].as_bool().unwrap_or(false)
    }

    fn load_ui_volume() -> u8 {
        Self::load_prefs()["ui_volume"].as_u64().unwrap_or(100).min(200) as u8
    }

    fn load_subs_off() -> bool {
        Self::load_prefs()["subs_off"].as_bool().unwrap_or(true)
    }

    fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        let subs_off = self.player.subs_off.load(std::sync::atomic::Ordering::Relaxed);
        let v = serde_json::json!({
            "playlist_view": self.playlist_view,
            "home_card_view": self.home_card_view,
            "ui_volume": self.ui_volume,
            "subs_off": subs_off,
        });
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }

    fn save_playlist_view(&self) { self.save_prefs(); }
    fn save_home_card_view(&self) { self.save_prefs(); }

    fn save_playlist(&self, was_playing: bool) {
        let payload = serde_json::json!({
            "items": self.player_tab.items,
            "last_played_item_id": self.last_played_item_id,
            "was_playing": was_playing,
        });
        if let Ok(json) = serde_json::to_string(&payload) {
            let path = crate::config::playlist_cache_path();
            let _ = std::fs::write(path, json);
        }
    }

    fn restore_playlist(&mut self) {
        let path = crate::config::playlist_cache_path();
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };

        let was_playing = v["was_playing"].as_bool().unwrap_or(false);
        let last_played_item_id = v["last_played_item_id"].as_str().map(String::from);

        // Extract IDs from whatever format is present, then re-fetch from server
        // so positions and played flags are always authoritative from Emby.
        let ids: Vec<String> = if let Some(arr) = v["items"].as_array() {
            arr.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect()
        } else if v.is_array() {
            serde_json::from_value(v).unwrap_or_default()
        } else {
            v["ids"].as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default()
        };
        if ids.is_empty() { return }
        let items: Vec<crate::api::MediaItem> = {
            let client = self.client.lock().unwrap();
            let Ok(fetched) = client.get_items_by_ids(&ids) else { return };
            fetched
        };

        if items.is_empty() { return }
        self.last_played_item_id = last_played_item_id;
        self.player_tab.playlist_cursor = if was_playing {
            self.last_played_item_id.as_deref()
                .and_then(|id| items.iter().position(|i| i.id == id))
                .unwrap_or(0)
        } else {
            0
        };
        self.player_tab.items = items;
    }

    fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout_seekbar_area;
        if bar.width == 0 { return; }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let runtime_s = self.connected_session_state.as_ref().map(|s| s.runtime_s).unwrap_or(0);
            if runtime_s == 0 { return; }
            let ticks = (fraction * (runtime_s * crate::api::TICKS_PER_SECOND) as f64) as i64;
            let id = conn_id.clone();
            self.do_session_command(move |c| c.session_seek(&id, ticks));
            return;
        }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 { return; }
        let target_secs = (fraction * runtime_ticks as f64) / TICKS_PER_SECOND as f64;
        self.player.send_command(PlayerCommand::SeekAbsolute(target_secs));
    }

    // Returns true if the click landed on a valid item row.
    fn click_set_cursor(&mut self, col: u16, row: u16) -> bool {
        if self.tab_idx == 1 {
            let inner = self.layout_playlist_inner;
            if inner.contains((col, row).into()) {
                let row_idx = (row - inner.y) as usize;
                if row_idx > 0 {
                    let data_row = row_idx - 1;
                    let visible = inner.height.saturating_sub(1) as usize;
                    let n = self.player_tab.items.len();
                    let cur = self.player_tab.playlist_cursor;
                    let scroll_start = cur.saturating_sub(visible.saturating_sub(1))
                        .min(n.saturating_sub(visible));
                    let clicked = scroll_start + data_row;
                    if clicked < n {
                        self.player_tab.playlist_cursor = clicked;
                        return true;
                    }
                }
            }
        } else if self.tab_idx == 0 {
            if self.home_rect.contains((col, row).into()) {
                let n_secs = self.layout_section_areas.len();
                let mut found_sec: Option<(usize, Rect)> = None;
                for sec in 0..n_secs {
                    let sect_area = self.layout_section_areas[sec];
                    if sect_area.contains((col, row).into()) {
                        found_sec = Some((sec, sect_area));
                        break;
                    }
                }
                if let Some((sec, sect_area)) = found_sec {
                    self.home.section = sec;
                    let inner = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).inner(sect_area);
                    if inner.contains((col, row).into()) {
                        let row_idx = (row - inner.y) as usize;
                        let scroll_start = self.layout_home_scrolls.get(sec).copied().unwrap_or(0);
                        let inner_h = inner.height as usize;
                        let inner_w = inner.width.max(1) as usize;
                        let item_texts: Vec<String> = {
                            let items_slice: &[MediaItem] = if sec == 0 {
                                &self.home.continue_items
                            } else {
                                self.home.latest.get(sec - 1).map(|c| c.2.as_slice()).unwrap_or(&[])
                            };
                            items_slice.iter().skip(scroll_start)
                                .map(|item| { let (t, _) = item_text_and_style(item, false); t })
                                .collect()
                        };
                        let mut line_acc = 0usize;
                        let mut found_item = None;
                        for (i, text) in item_texts.iter().enumerate() {
                            let n_lines = wrap(text, inner_w).len().max(1);
                            if row_idx < line_acc + n_lines {
                                found_item = Some(scroll_start + i);
                                break;
                            }
                            line_acc += n_lines;
                            if line_acc >= inner_h { break; }
                        }
                        if let Some(clicked) = found_item {
                            let (len, _) = self.home_section_len_cur(sec);
                            if clicked < len {
                                self.set_home_cursor(sec, clicked);
                                return true;
                            }
                        }
                    }
                }
            }
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let tbl = self.layout_lib_table_area;
            if tbl.contains((col, row).into()) {
                let click_y = row - tbl.y;
                let display_pos = {
                    let mut y = 0u16;
                    let mut found = self.layout_lib_scroll;
                    for (vi, &h) in self.layout_lib_row_heights.iter().enumerate() {
                        if click_y < y + h { found = self.layout_lib_scroll + vi; break; }
                        y += h;
                    }
                    found
                };
                let lib_off = self.lib_tab_offset();
                let lib = &mut self.libs[self.tab_idx - lib_off];
                let hit = if let Some(s) = &mut lib.search {
                    if display_pos < s.results.len() { s.cursor = display_pos; true } else { false }
                } else if let Some(lvl) = lib.nav_stack.last_mut() {
                    if display_pos < lvl.items.len() { lvl.cursor = display_pos; true } else { false }
                } else { false };
                return hit;
            }
        }
        false
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};
        let col = mouse.column;
        let row = mouse.row;
        if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
            let now = Instant::now();
            if now.duration_since(self.last_scroll_at) < Duration::from_millis(120) {
                return;
            }
            self.last_scroll_at = now;
        }

        if self.show_help {
            match mouse.kind {
                MouseEventKind::ScrollDown => { self.help_scroll += 3; }
                MouseEventKind::ScrollUp   => { self.help_scroll = self.help_scroll.saturating_sub(3); }
                _ => {}
            }
            return;
        }

        // Tab bar — always consume clicks in this row
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout_tabs_area.contains((col, row).into()) {
                if let Some(idx) = self.tab_idx_at(col) {
                    if idx == usize::MAX - 1 {
                        // scroll left indicator
                        self.tab_scroll = self.tab_scroll.saturating_sub(1);
                    } else if idx == usize::MAX {
                        // scroll right indicator
                        let max_scroll = self.tab_count().saturating_sub(1);
                        self.tab_scroll = (self.tab_scroll + 1).min(max_scroll);
                    } else {
                        self.set_tab(idx);
                    }
                }
                return;
            }

        // Gear icon click
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout_settings_area.contains((col, row).into()) {
            self.show_settings = !self.show_settings;
            return;
        }

        // Settings screen intercepts all mouse events while open
        if self.show_settings {
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollDown) { 1 } else { -1 };
                if self.layout_tabbar_vol_area.contains((col, row).into())
                    || self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(-delta * 5);
                    return;
                }
                if self.tab_idx == 0 {
                    let sb = self.layout_home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        let active = self.player.status.lock().unwrap().active;
                        let chrome: u16 = if active { 6 } else { 3 };
                        let panel_h = self.terminal_height.saturating_sub(chrome);
                        let n_sections = 1 + self.home.latest.len();
                        let visible = ((panel_h / HOME_MIN_SECTION_H) as usize).max(1).min(n_sections);
                        let max_offset = n_sections.saturating_sub(visible);
                        self.home_panel_section_offset =
                            (self.home_panel_section_offset as i64 + delta).clamp(0, max_offset as i64) as usize;
                    } else if self.home_rect.contains((col, row).into()) {
                        self.move_home_cursor(delta);
                    }
                } else if self.tab_idx == 1 {
                    let n = self.player_tab.items.len();
                    if n > 0 {
                        self.player_tab.playlist_cursor =
                            (self.player_tab.playlist_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else if self.tab_idx == self.log_tab_idx() {
                    if delta > 0 { self.log_scroll += 1; }
                    else { self.log_scroll = self.log_scroll.saturating_sub(1); }
                } else {
                    self.move_lib_cursor(delta);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Click inside context menu: navigate or dismiss
                if self.context_menu.is_some() {
                    if let Some(rect) = self.context_menu_rect {
                        if rect.contains((col, row).into()) {
                            let inner_y = rect.y + 1;
                            if row >= inner_y && (row - inner_y) < self.context_menu.as_ref().unwrap().items.len() as u16 {
                                let idx = (row - inner_y) as usize;
                                let action = self.context_menu.as_ref().unwrap().actions.get(idx).cloned();
                                self.context_menu = None;
                                self.context_menu_rect = None;
                                self.force_clear = true;
                                self.execute_context_action(action);
                            } else {
                                self.context_menu = None;
                                self.force_clear = true;
                            }
                            return;
                        }
                    }
                    self.context_menu = None;
                    self.force_clear = true;
                    return;
                }

                let now = Instant::now();

                // Carousel directional arrow clicks
                if let Some(r) = self.layout_carousel_left_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 { self.move_home_cursor(-1); }
                        else { if self.player_tab.playlist_cursor > 0 { self.player_tab.playlist_cursor -= 1; } }
                        return;
                    }
                }
                if let Some(r) = self.layout_carousel_right_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 { self.move_home_cursor(1); }
                        else { let n = self.player_tab.items.len(); if self.player_tab.playlist_cursor + 1 < n { self.player_tab.playlist_cursor += 1; } }
                        return;
                    }
                }
                if let Some(r) = self.layout_carousel_up_arrow {
                    if r.contains((col, row).into()) {
                        if self.home.section > 0 {
                            self.home.section -= 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }
                if let Some(r) = self.layout_carousel_down_arrow {
                    if r.contains((col, row).into()) {
                        let n_sections = 1 + self.home.latest.len();
                        if self.home.section + 1 < n_sections {
                            self.home.section += 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }

                // Carousel card clicks — own double-click tracking independent of position-exact is_double
                if self.tab_idx == 1 && self.playlist_view == 1 {
                    let slots = self.layout_carousel_slots;
                    self.log.push(Level::Info, "mouse", format!(
                        "carousel click ({col},{row}): slots=[({:?},{:?}),({:?},{:?}),({:?},{:?})]",
                        slots[0].0, slots[0].1, slots[1].0, slots[1].1, slots[2].0, slots[2].1
                    ));
                    for (slot_idx, (maybe_item_idx, card_rect)) in slots.iter().enumerate() {
                        if card_rect.contains((col, row).into()) {
                            let elapsed_ms = now.duration_since(self.last_carousel_click_time).as_millis();
                            let is_double_slot = self.last_carousel_click_slot == Some(slot_idx)
                                && now.duration_since(self.last_carousel_click_time) < Duration::from_millis(400);
                            self.log.push(Level::Info, "mouse", format!(
                                "carousel hit slot={slot_idx} item={maybe_item_idx:?} is_double={is_double_slot} elapsed={elapsed_ms}ms last_slot={:?}",
                                self.last_carousel_click_slot
                            ));
                            self.last_carousel_click_slot = Some(slot_idx);
                            self.last_carousel_click_time = now;
                            if slot_idx == 1 {
                                if is_double_slot {
                                    if let Some(item_idx) = maybe_item_idx {
                                        let (active, active_idx) = {
                                            let s = self.player.status.lock().unwrap();
                                            (s.active, s.current_idx)
                                        };
                                        self.log.push(Level::Info, "mouse", format!(
                                            "carousel dbl-center: active={active} active_idx={active_idx} item_idx={item_idx}"
                                        ));
                                        if active && active_idx == *item_idx {
                                            self.player.send_command(PlayerCommand::TogglePause);
                                        } else if active {
                                            self.player.send_command(PlayerCommand::JumpTo(*item_idx));
                                        } else if !self.player_tab.items.is_empty() {
                                            let items = self.player_tab.items.clone();
                                            let c = Arc::new(self.client.lock().unwrap().clone());
                                            self.player.play_playlist(items, *item_idx, c, self.log.clone(), self.ui_volume);
                                        }
                                    }
                                }
                                // single-click center: no-op
                            } else if let Some(item_idx) = maybe_item_idx {
                                self.player_tab.playlist_cursor = *item_idx;
                            }
                            return;
                        }
                    }
                    self.log.push(Level::Info, "mouse", format!("carousel click ({col},{row}): no slot hit"));
                    if self.layout_playlist_inner.contains((col, row).into()) {
                        return; // click between cards but inside carousel area — consume it
                    }
                    // click is outside carousel (e.g. playback controls) — fall through
                }

                let is_double = now.duration_since(self.last_click_time) < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                if is_double {
                    if self.layout_seekbar_area.contains((col, row).into()) {
                        self.seek_to_col(col);
                        return;
                    }
                    if self.tab_idx == 0 {
                        if self.home_rect.contains((col, row).into()) { self.select_home(); }
                    } else if self.tab_idx == 1 {
                        let t = self.player_tab.playlist_cursor;
                        if t < self.player_tab.items.len() {
                            if let Some(ref conn_id) = self.connected_session_id.clone() {
                                let item = self.player_tab.items[t].clone();
                                let id = conn_id.clone();
                                let item_id = item.id.clone();
                                let start_ticks = item.playback_position_ticks;
                                let label = item.playback_label();
                                self.flash_status(format!("Playing on remote: {label}"));
                                self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
                            } else {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        }
                    } else if self.tab_idx != self.log_tab_idx() {
                        self.select();
                    }
                    return;
                }

                // Click on playback buttons (global)
                if self.layout_button_area.contains((col, row).into()) {
                    let btn = (col.saturating_sub(self.layout_button_area.x) / 5) as usize;
                    if btn < 6 { self.handle_button_click(btn); }
                    return;
                }

                // Click on info row: exact chip rects
                if self.layout_sub_area.contains((col, row).into())
                    || self.layout_sub_indicator_area.contains((col, row).into()) {
                    self.toggle_sub();
                    return;
                }
                if self.layout_audio_indicator_area.contains((col, row).into())
                    || self.layout_audio_area.contains((col, row).into()) {
                    if self.is_audio_item() { self.toggle_mute(); } else { self.cycle_audio(); }
                    return;
                }
                if self.layout_sessions_btn_area.contains((col, row).into()) {
                    self.show_sessions = true;
                    self.spawn_sessions_load();
                    return;
                }
                if self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(-5);
                    return;
                }

                // Home panel scrollbar click
                if self.tab_idx == 0 {
                    let sb = self.layout_home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        self.home_scrollbar_seek(row);
                        return;
                    }
                }

                // Breadcrumb click: navigate back to that depth
                if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
                    let crumbs = self.layout_breadcrumbs.clone();
                    let lib_off = self.lib_tab_offset();
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            let lib = &mut self.libs[self.tab_idx - lib_off];
                            lib.nav_stack.truncate(target_depth);
                            lib.search = None;
                            return;
                        }
                    }
                }
                self.click_set_cursor(col, row);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(5);
                    return;
                }
                if (self.tab_idx == 1 && self.playlist_view == 1) || (self.tab_idx == 0 && self.home_card_view) {
                    let slots = self.layout_carousel_slots;
                    for (maybe_item_idx, card_rect) in slots.iter() {
                        if card_rect.contains((col, row).into()) {
                            if let Some(item_idx) = maybe_item_idx {
                                if self.tab_idx == 1 {
                                    self.player_tab.playlist_cursor = *item_idx;
                                } else {
                                    let sec = self.home.section;
                                    self.set_home_cursor(sec, *item_idx);
                                }
                                let cx = card_rect.x + card_rect.width / 2;
                                let cy = card_rect.y + card_rect.height / 2;
                                self.open_context_menu_at(cx, cy);
                            }
                            return;
                        }
                    }
                    return;
                }
                if self.click_set_cursor(col, row) {
                    self.open_context_menu_at(col, row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left)
                if self.tab_idx == 0 && {
                    let sb = self.layout_home_scrollbar;
                    sb.width > 0 && sb.contains((col, row).into())
                } => {
                    self.home_scrollbar_seek(row);
                }
            MouseEventKind::Drag(MouseButton::Left)
                if self.layout_seekbar_area.contains((col, row).into())
                    && self.last_drag_seek.elapsed() >= Duration::from_millis(150)
                => {
                    self.last_drag_seek = Instant::now();
                    self.seek_to_col(col);
                }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Right) => {
                if let (Some(ref mut menu), Some(rect)) = (&mut self.context_menu, self.context_menu_rect) {
                    let inner_y = rect.y + 1;
                    if rect.contains((col, row).into()) && row >= inner_y {
                        let idx = (row - inner_y) as usize;
                        if idx < menu.items.len() {
                            menu.cursor = idx;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ── navigation ───────────────────────────────────────────────────────────

    fn lib_page_size(&self) -> usize {
        self.layout_lib_row_heights.len().saturating_sub(1).max(1)
    }

    fn playlist_page_size(&self) -> usize {
        self.layout_playlist_inner.height.saturating_sub(2).max(1) as usize // minus header row
    }

    fn move_lib_cursor(&mut self, delta: i64) {
        let lib_off = self.lib_tab_offset();
        let lib = &mut self.libs[self.tab_idx - lib_off];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 {
                s.cursor = (s.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
            }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                lvl.cursor = (lvl.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
            }
        }
    }

    fn jump_lib_cursor(&mut self, to_end: bool) {
        let lib_off = self.lib_tab_offset();
        let lib = &mut self.libs[self.tab_idx - lib_off];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 { s.cursor = if to_end { n - 1 } else { 0 }; }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 { lvl.cursor = if to_end { n - 1 } else { 0 }; }
        }
    }

    fn move_home_cursor(&mut self, delta: i64) {
        let sec = self.home.section;
        let (len, cur) = self.home_section_len_cur(sec);
        if delta > 0 {
            if cur + 1 < len { self.set_home_cursor(sec, cur + 1); }
        } else {
            if cur > 0 { self.set_home_cursor(sec, cur - 1); }
        }
    }

    fn ensure_home_section_visible(&mut self) {
        let active = self.player.status.lock().unwrap().active;
        let chrome: u16 = if active { 6 } else { 3 };
        let panel_h = self.terminal_height.saturating_sub(chrome);

        let n_latest = self.home.latest.len();
        let n_rows = 1 + (n_latest + 1) / 2;
        let visible_rows = if (n_rows as u16) * HOME_MIN_SECTION_H <= panel_h {
            n_rows
        } else {
            ((panel_h / HOME_MIN_SECTION_H) as usize).max(1)
        };

        let sec = self.home.section;
        let sec_row = if sec == 0 { 0 } else { 1 + (sec - 1) / 2 };
        if sec_row < self.home_panel_section_offset {
            self.home_panel_section_offset = sec_row;
        } else if sec_row >= self.home_panel_section_offset + visible_rows {
            self.home_panel_section_offset = sec_row + 1 - visible_rows;
        }
        let max_offset = n_rows.saturating_sub(visible_rows);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
    }

    fn home_scrollbar_seek(&mut self, row: u16) {
        let sb = self.layout_home_scrollbar;
        if sb.height == 0 { return; }
        let active = self.player.status.lock().unwrap().active;
        let chrome: u16 = if active { 6 } else { 3 };
        let panel_h = self.terminal_height.saturating_sub(chrome);
        let n_latest = self.home.latest.len();
        let n_rows = 1 + (n_latest + 1) / 2;
        let visible_rows = ((panel_h / HOME_MIN_SECTION_H) as usize).max(1).min(n_rows);
        let max_offset = n_rows.saturating_sub(visible_rows);
        if max_offset == 0 { return; }
        let frac = (row.saturating_sub(sb.y)) as f64 / sb.height as f64;
        let new_offset = ((frac * max_offset as f64).round() as usize).min(max_offset);
        self.home_panel_section_offset = new_offset;
    }

    fn home_section_len_cur(&self, sec: usize) -> (usize, usize) {
        if sec == 0 {
            (self.home.continue_items.len(), self.home.continue_cursor)
        } else {
            self.home.latest.get(sec - 1)
                .map(|c| (c.2.len(), c.3))
                .unwrap_or((0, 0))
        }
    }

    fn set_home_cursor(&mut self, sec: usize, val: usize) {
        if sec == 0 {
            self.home.continue_cursor = val;
        } else if let Some(col) = self.home.latest.get_mut(sec - 1) {
            col.3 = val;
        }
    }

    fn current_home_item(&self) -> Option<MediaItem> {
        let sec = self.home.section;
        if sec == 0 {
            self.home.continue_items.get(self.home.continue_cursor).cloned()
        } else {
            let col = self.home.latest.get(sec - 1)?;
            col.2.get(col.3).cloned()
        }
    }

    fn current_lib_item(&self) -> Option<MediaItem> {
        let lib = self.libs.get(self.tab_idx - self.lib_tab_offset())?;
        if lib.nav_stack.is_empty() {
            Some(lib.library.clone())
        } else {
            if let Some(s) = &lib.search {
                let idx = *s.results.get(s.cursor)?;
                return s.items.get(idx).cloned();
            }
            let lvl = lib.nav_stack.last()?;
            lvl.items.get(lvl.cursor).cloned()
        }
    }

    fn is_album_level(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" { return false; }
        if self.music_levels.is_empty() { return false; }
        let stack_len = lib.nav_stack.len();
        if stack_len < 2 { return false; }
        self.music_levels.get(stack_len - 2).map(|s| s == "album").unwrap_or(false)
    }

    fn is_audio_item(&self) -> bool {
        let idx = self.player_tab.playlist_cursor;
        self.player_tab.items.get(idx)
            .map(|i| i.media_type == "Audio" || i.item_type == "Audio")
            .unwrap_or(false)
    }

    fn toggle_mute(&mut self) {
        if self.ui_volume == 0 {
            if let Some(v) = self.pre_mute_volume.take() {
                self.player.send_command(PlayerCommand::SetVolume(v as i64));
                self.ui_volume = v;
            }
        } else {
            self.pre_mute_volume = Some(self.ui_volume);
            self.player.send_command(PlayerCommand::SetVolume(0));
            self.ui_volume = 0;
        }
    }

    fn cycle_audio(&mut self) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let cur = self.connected_session_state.as_ref().map(|s| s.audio_index).unwrap_or(1);
            let next = if cur <= 1 { 2 } else { 1 };
            let id = conn_id.clone();
            if let Some(ref mut state) = self.connected_session_state {
                state.audio_index = next;
            }
            self.do_session_command(move |c| c.session_set_audio_index(&id, next));
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        if tracks.is_empty() { return; }
        // Cycle: muted → track1 → track2 → … → muted
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        if next_id == 0 {
            self.pre_mute_volume = Some(self.ui_volume);
            self.player.send_command(PlayerCommand::SetVolume(0));
            self.ui_volume = 0;
        } else if current_id == 0 {
            if let Some(v) = self.pre_mute_volume.take() {
                self.player.send_command(PlayerCommand::SetVolume(v as i64));
                self.ui_volume = v;
            }
        }
        self.player.send_command(PlayerCommand::SetAudio(next_id));
    }

    fn toggle_sub(&mut self) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let idx = self.connected_session_state.as_ref().map(|s| s.sub_index).unwrap_or(-1);
            let next = if idx == -1 { 1i64 } else { -1i64 };
            let id = conn_id.clone();
            if let Some(ref mut state) = self.connected_session_state {
                state.sub_index = next;
            }
            self.do_session_command(move |c| c.session_set_subtitle_index(&id, next));
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.sub_tracks.clone(), s.sub_id)
        };
        let currently_off = self.player.subs_off.load(std::sync::atomic::Ordering::Relaxed);
        if currently_off {
            self.player.subs_off.store(false, std::sync::atomic::Ordering::Relaxed);
            if let Some(&(first_id, _)) = tracks.first() {
                if current_id == 0 {
                    self.player.send_command(PlayerCommand::SetSub(first_id));
                }
            }
        } else {
            self.player.subs_off.store(true, std::sync::atomic::Ordering::Relaxed);
            if current_id != 0 {
                self.player.send_command(PlayerCommand::SetSub(0));
            }
        }
        self.save_prefs();
    }

    fn cycle_sub(&mut self) {
        if let Some(ref _conn_id) = self.connected_session_id.clone() {
            // Without track list from remote, just toggle off/on
            self.toggle_sub();
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.sub_tracks.clone(), s.sub_id)
        };
        if tracks.is_empty() { return; }
        // Cycle: off → track1 → track2 → … → off
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        self.player.subs_off.store(next_id == 0, std::sync::atomic::Ordering::Relaxed);
        self.player.send_command(PlayerCommand::SetSub(next_id));
        self.save_prefs();
    }


    fn remove_from_playlist(&mut self, pos: usize) {
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if active && current_idx == pos {
            self.confirm_remove_idx = Some(pos);
            return;
        }
        let name = self.player_tab.items[pos].display_name();
        self.player_tab.items.remove(pos);
        if !self.player_tab.items.is_empty() {
            self.player_tab.playlist_cursor =
                self.player_tab.playlist_cursor.min(self.player_tab.items.len() - 1);
        } else {
            self.player_tab.playlist_cursor = 0;
        }
        self.status = format!("Removed: {name}");
    }

    fn flash_status(&mut self, msg: String) {
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(3));
    }

    /// Play a single item. For series episodes with always_play_next, expands
    /// to the full series queue starting from this episode — matching Emby Web's model.
    fn play_item(&mut self, item: MediaItem) {
        let label = item.playback_label();
        // Route to connected remote session instead of local player
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let id = conn_id.clone();
            let item_id = item.id.clone();
            let start_ticks = item.playback_position_ticks;
            self.flash_status(format!("Playing on remote: {label}"));
            self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
            return;
        }
        if !item.series_id.is_empty() && self.player.always_play_next {
            let c = self.client.lock().unwrap();
            let episodes = c.get_episodes_from(&item.series_id, &item.id, &self.log);
            drop(c);
            if episodes.len() > 1 {
                let c = Arc::new(self.client.lock().unwrap().clone());
                self.player_tab.items = episodes.clone();
                self.player_tab.playlist_cursor = 0;
                self.flash_status(label);
                self.player.play_playlist(episodes, 0, c, self.log.clone(), self.ui_volume);
                return;
            }
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        self.player_tab.items = vec![item.clone()];
        self.player_tab.playlist_cursor = 0;
        self.flash_status(label);
        self.player.play(&item, c, self.log.clone(), self.ui_volume);
    }

    fn enqueue_selected(&mut self) {
        if self.tab_idx == 0 {
            let Some(item) = self.current_home_item() else { return };
            if item.is_folder { self.do_enqueue_folder(item); return; }
            if !is_playable(&item) { return; }
            let name = item.display_name();
            self.player_tab.items.push(item);
            self.flash_status(format!("Added: {name}"));
        } else if self.tab_idx >= 2 && self.tab_idx != self.log_tab_idx() {
            let Some(item) = self.current_lib_item() else { return };
            if item.is_folder { self.do_enqueue_folder(item); return; }
            if !is_playable(&item) { return; }
            let name = item.display_name();
            self.player_tab.items.push(item);
            self.flash_status(format!("Added: {name}"));
        }
    }

    fn do_enqueue_folder(&mut self, item: crate::api::MediaItem) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(&item.id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                let count = items.len();
                drop(client);
                if count == 0 { self.flash_status("Nothing to enqueue".into()); return; }
                for i in items { self.player_tab.items.push(i); }
                self.flash_status(format!("Enqueued {count} items from {}", item.display_name()));
            }
            Err(e) => { drop(client); self.flash_status(format!("Error: {e}")); }
        }
    }

    fn select_home(&mut self) {
        let Some(item) = self.current_home_item() else { return };
        if item.is_folder {
            // Switch to the matching library tab if it's a top-level library
            if let Some(i) = self.libs.iter().position(|l| l.library.id == item.id) {
                self.set_tab(i + 2);
                return;
            }
            // Otherwise navigate into the folder within the corresponding library tab
            let sec = self.home.section;
            if sec > 0 {
                if let Some(lib_id) = self.home.latest.get(sec - 1).map(|c| c.1.clone()) {
                    if let Some(lib_idx) = self.libs.iter().position(|l| l.library.id == lib_id) {
                        let lib = &mut self.libs[lib_idx];
                        lib.search = None;
                        lib.nav_stack.push(BrowseLevel {
                            parent_id: item.id.clone(), title: item.name.clone(),
                            items: vec![], cursor: 0,
                            item_types: None, unplayed_only: false, loading: true,
                        });
                        self.set_tab(lib_idx + 2);
                        self.spawn_browse(lib_idx, item.id, item.name, None, false);
                    }
                }
            }
            return;
        }
        if is_playable(&item) {
            let fresh = {
                let c = self.client.lock().unwrap();
                c.get_items_by_ids(std::slice::from_ref(&item.id))
                    .ok()
                    .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
                    .unwrap_or(item)
            };
            self.play_item(fresh);
        }
    }

    fn select(&mut self) {
        let Some(item) = self.current_lib_item() else { return };
        if item.is_folder {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &mut self.libs[lib_idx];
            lib.search = None;
            lib.nav_stack.push(BrowseLevel {
                parent_id: item.id.clone(), title: item.name.clone(),
                items: vec![], cursor: 0,
                item_types: None, unplayed_only: false, loading: true,
            });
            self.layout_lib_scroll = 0;
            self.spawn_browse(lib_idx, item.id, item.name, None, false);
        } else if is_playable(&item) {
            let fresh = {
                let c = self.client.lock().unwrap();
                c.get_items_by_ids(std::slice::from_ref(&item.id))
                    .ok()
                    .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
                    .unwrap_or(item)
            };
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            if self.libs[lib_idx].search.is_none() && self.is_album_level(lib_idx) {
                let level_items = self.libs[lib_idx].nav_stack.last()
                    .map(|l| l.items.clone())
                    .unwrap_or_default();
                let mut tracks: Vec<MediaItem> = level_items.into_iter()
                    .filter(|i| is_playable(i))
                    .collect();
                tracks.sort_by_key(|i| {
                    if i.index_number > 0 { (0i64, i.index_number, String::new()) }
                    else { (1i64, 0, natural_sort_key(i.sort_key())) }
                });
                if let Some(start_idx) = tracks.iter().position(|i| i.id == fresh.id) {
                    let label = fresh.playback_label();
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    self.player_tab.items = tracks.clone();
                    self.player_tab.playlist_cursor = 0;
                    self.flash_status(label);
                    self.player.play_playlist(tracks, start_idx, c, self.log.clone(), self.ui_volume);
                    return;
                }
            }
            let autoload = self.client.lock().unwrap().config.autoload;
            if autoload {
                if let Some(parent_id) = self.libs[lib_idx].nav_stack.last().map(|l| l.parent_id.clone()) {
                    let client = self.client.lock().unwrap();
                    match client.get_direct_playable(&parent_id) {
                        Ok(mut siblings) => {
                            siblings.retain(|i| !i.is_folder);
                            siblings.sort_by_key(|a| natural_sort_key(a.sort_key()));
                            if let Some(start_idx) = siblings.iter().position(|i| i.id == fresh.id) {
                                let label = fresh.playback_label();
                                let c = Arc::new(client.clone());
                                drop(client);
                                self.player_tab.items = siblings.clone();
                                self.player_tab.playlist_cursor = 0;
                                self.flash_status(label);
                                self.player.play_playlist(siblings, start_idx, c, self.log.clone(), self.ui_volume);
                                return;
                            }
                            drop(client);
                        }
                        Err(_) => { drop(client); }
                    }
                }
            }
            self.play_item(fresh);
        }
    }

    fn go_back(&mut self) {
        if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib_off = self.lib_tab_offset();
            let lib = &mut self.libs[self.tab_idx - lib_off];
            if lib.search.take().is_none() && lib.nav_stack.len() > 1 {
                lib.nav_stack.pop();
                self.layout_lib_scroll = 0;
            }
        }
    }

    fn execute_context_action(&mut self, action: Option<ContextAction>) {
        match action {
            Some(ContextAction::Play) => {
                if self.tab_idx == 0 { self.select_home(); }
                else if self.tab_idx == 1 {
                    let t = self.player_tab.playlist_cursor;
                    if t < self.player_tab.items.len() {
                        if let Some(ref conn_id) = self.connected_session_id.clone() {
                            let item = self.player_tab.items[t].clone();
                            let id = conn_id.clone();
                            let item_id = item.id.clone();
                            let start_ticks = item.playback_position_ticks;
                            let label = item.playback_label();
                            self.flash_status(format!("Playing on remote: {label}"));
                            self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
                        } else {
                            self.player.send_command(PlayerCommand::JumpTo(t));
                        }
                    }
                }
                else { self.select(); }
            }
            Some(ContextAction::PlayFolder(id)) => self.play_folder(&id),
            Some(ContextAction::ShuffleFolder(id)) => self.shuffle_folder(&id),
            Some(ContextAction::Enqueue) => self.enqueue_selected(),
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder(item),
            Some(ContextAction::MarkPlayed(id))   => self.context_set_played(&id, true),
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false),
            Some(ContextAction::RemoveFromContinueWatching) => self.remove_from_continue_watching(),
            Some(ContextAction::RemoveFromPlaylist(pos)) => self.remove_from_playlist(pos),
            None => {}
        }
    }

    fn context_set_played(&mut self, item_id: &str, played: bool) {
        let client = self.client.lock().unwrap();
        let result = if played { client.mark_played(item_id) } else { client.mark_unplayed(item_id) };
        drop(client);
        match result {
            Ok(()) => {
                if self.tab_idx == 0 { let _ = self.fetch_home(); } else { self.refresh_lib(); }
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn remove_from_continue_watching(&mut self) {
        let Some(item) = self.home.continue_items.get(self.home.continue_cursor).cloned() else { return };
        let client = self.client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => { let _ = self.fetch_home(); }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn toggle_watched_home(&mut self) {
        let Some(item) = self.current_home_item() else { return };
        if item.is_folder || item.is_audio() { return; }
        let client = self.client.lock().unwrap();
        let result = if item.played { client.mark_unplayed(&item.id) } else { client.mark_played(&item.id) };
        drop(client);
        match result {
            Ok(()) => { let _ = self.fetch_home(); }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn toggle_watched(&mut self) {
        let Some(item) = self.current_lib_item() else { return };
        if item.is_folder || item.is_audio() { return; }
        let client = self.client.lock().unwrap();
        let result = if item.played { client.mark_unplayed(&item.id) } else { client.mark_played(&item.id) };
        drop(client);
        match result {
            Ok(()) => self.refresh_lib(),
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn refresh_lib(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.loading = true;
            let parent_id = lvl.parent_id.clone();
            let item_types = lvl.item_types.clone();
            let unplayed_only = lvl.unplayed_only;
            self.spawn_refresh(lib_idx, parent_id, item_types, unplayed_only);
        }
    }

    fn refresh_queue(&mut self) {
        if self.player_tab.items.is_empty() { return; }
        let ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        let client = self.client.lock().unwrap();
        if let Ok(fetched) = client.get_items_by_ids(&ids) {
            let mut map: std::collections::HashMap<String, crate::api::MediaItem> =
                fetched.into_iter().map(|i| (i.id.clone(), i)).collect();
            for item in &mut self.player_tab.items {
                if let Some(fresh) = map.remove(&item.id) {
                    *item = fresh;
                }
            }
        }
    }

    fn refresh_current_view(&mut self) {
        self.force_clear = true;
        if self.tab_idx == 0 {
            match self.fetch_home() {
                Ok(()) => self.flash_status("Home refreshed".into()),
                Err(e) => self.flash_status(format!("Refresh error: {e}")),
            }
        } else if self.tab_idx == 1 {
            self.refresh_queue();
            self.flash_status("Queue refreshed".into());
        } else if self.tab_idx != self.log_tab_idx() {
            self.refresh_lib();
        }
    }

    fn shuffle_play(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        let parent_id = {
            let lib = &self.libs[lib_idx];
            let item = lib.nav_stack.last().and_then(|lvl| {
                let idx = lib.search.as_ref()
                    .and_then(|s| s.results.get(s.cursor).copied())
                    .unwrap_or(lvl.cursor);
                lvl.items.get(idx)
            });
            item.filter(|i| i.is_folder)
                .map(|i| i.id.clone())
                .or_else(|| lib.nav_stack.last().map(|l| l.parent_id.clone()))
                .unwrap_or_else(|| lib.library.id.clone())
        };
        let client = self.client.lock().unwrap();
        match client.get_all_videos_recursive(&parent_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() { self.status = "Nothing to shuffle".into(); return; }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                let c = Arc::new(client.clone());
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.player.play_playlist(items, 0, c, self.log.clone(), self.ui_volume);
                self.tab_idx = 1;
                self.flash_status(format!("Shuffling {count} items"));
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    fn play_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                if items.is_empty() { drop(client); self.status = "Nothing to play".into(); return; }
                let count = items.len();
                let c = Arc::new(client.clone());
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.player.play_playlist(items, 0, c, self.log.clone(), self.ui_volume);
                self.tab_idx = 1;
                self.status = format!("Playing {count} items");
            }
            Err(e) => { drop(client); self.status = format!("Error: {e}"); }
        }
    }

    fn shuffle_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() { drop(client); self.status = "Nothing to shuffle".into(); return; }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                let c = Arc::new(client.clone());
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.player.play_playlist(items, 0, c, self.log.clone(), self.ui_volume);
                self.tab_idx = 1;
                self.flash_status(format!("Shuffling {count} items"));
            }
            Err(e) => { drop(client); self.status = format!("Error: {e}"); }
        }
    }

    fn set_tab(&mut self, idx: usize) {
        if idx != self.tab_idx && !self.card_image_states.is_empty() {
            self.force_clear = true;
        }
        self.tab_idx = idx;
        self.ensure_tab_visible();
        if self.tab_idx == 0 {
            self.home.section = 0;
            let _ = self.fetch_home();
        } else {
            self.ensure_library_loaded();
        }
    }

    fn ensure_library_loaded(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let idx = self.tab_idx - self.lib_tab_offset();
        if self.libs[idx].nav_stack.is_empty() {
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let (item_types, unplayed_only) = match self.libs[idx].library.collection_type.as_str() {
                "movies"               => (Some("Movie".to_string()), false),
                "channels"|"homevideos" if lib_name == "Youtube" => (Some("Video".to_string()), true),
                _                      => (None, false),
            };
            self.libs[idx].nav_stack.push(BrowseLevel {
                parent_id: lib_id.clone(), title: lib_name.clone(),
                items: vec![], cursor: 0,
                item_types: item_types.clone(), unplayed_only, loading: true,
            });
            self.spawn_browse(idx, lib_id, lib_name, item_types, unplayed_only);
        }
    }

    fn refresh_after_stop(&mut self) {
        let _ = self.fetch_home();
        let fetches: Vec<(usize, String, Option<String>, bool)> = self.libs.iter().enumerate()
            .filter_map(|(i, lib)| lib.nav_stack.last().map(|lvl| {
                (i, lvl.parent_id.clone(), lvl.item_types.clone(), lvl.unplayed_only)
            }))
            .collect();
        for (lib_idx, parent_id, item_types, unplayed_only) in fetches {
            self.spawn_refresh(lib_idx, parent_id, item_types, unplayed_only);
        }
    }

    fn spawn_browse(&self, lib_idx: usize, parent_id: String, title: String,
                    item_types: Option<String>, unplayed_only: bool) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items(&parent_id, item_types.as_deref(), unplayed_only) {
                Ok((mut items, _)) => {
                    items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                    let _ = tx.send(LibEvent::Loaded {
                        lib_idx,
                        parent_id: parent_id.clone(),
                        level: BrowseLevel {
                            parent_id, title, items, cursor: 0,
                            item_types, unplayed_only, loading: false,
                        },
                    });
                }
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); }
            }
        });
    }

    fn spawn_refresh(&self, lib_idx: usize, parent_id: String,
                     item_types: Option<String>, unplayed_only: bool) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items(&parent_id, item_types.as_deref(), unplayed_only) {
                Ok((mut items, _)) => {
                    items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                    let _ = tx.send(LibEvent::Refreshed { lib_idx, parent_id, items });
                }
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); }
            }
        });
    }

    fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            match client.get_sessions() {
                Ok(sessions) => { let _ = tx.send(SessionEvent::Loaded(sessions)); }
                Err(e)       => { let _ = tx.send(SessionEvent::Error(e)); }
            }
        });
    }

    fn do_session_command(&self, f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::Error(e));
                return;
            }
            match client.get_sessions() {
                Ok(sessions) => { let _ = tx.send(SessionEvent::Loaded(sessions)); }
                Err(e)       => { let _ = tx.send(SessionEvent::Error(e)); }
            }
        });
    }

    fn handle_lib_event(&mut self, ev: LibEvent) {
        match ev {
            LibEvent::Loaded { lib_idx, parent_id, level } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && last.loading {
                            *last = level;
                        }
                    }
                }
                if self.is_album_level(lib_idx) {
                    let title = self.libs[lib_idx].nav_stack.last()
                        .map(|l| l.title.clone())
                        .unwrap_or_default();
                    self.log.push(Level::Debug, "app", format!("album: entered «{title}»"));
                }
            }
            LibEvent::Refreshed { lib_idx, parent_id, items } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id {
                            last.items = items;
                            last.loading = false;
                        }
                    }
                }
                if self.tab_idx == lib_idx + self.lib_tab_offset() {
                    self.flash_status("Refreshed".into());
                }
            }
            LibEvent::Error(e) => {
                self.status = format!("Error: {e}");
            }
        }
    }

    fn fetch_home(&mut self) -> Result<(), String> {
        let client = self.client.lock().unwrap();

        self.home.continue_items = client.get_continue_watching(10).unwrap_or_default();

        let all_views = client.get_views()?;
        let old_libs: HashMap<String, Vec<BrowseLevel>> = self.libs.drain(..)
            .map(|mut l| (l.library.id.clone(), std::mem::take(&mut l.nav_stack)))
            .collect();

        for view in all_views.iter().filter(|v| !self.hidden_libraries.contains(&v.name.to_lowercase())) {
            let stack = old_libs.get(&view.id)
                .map(|s| s.iter().map(|lvl| BrowseLevel {
                    parent_id: lvl.parent_id.clone(), title: lvl.title.clone(),
                    items: lvl.items.clone(), cursor: lvl.cursor,
                    item_types: lvl.item_types.clone(), unplayed_only: lvl.unplayed_only,
                    loading: false,
                }).collect())
                .unwrap_or_default();
            self.libs.push(LibraryTab { library: view.clone(), nav_stack: stack, search: None });
        }

        let old_cursors: HashMap<String, usize> = self.home.latest.iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        let user_views = client.get_user_views().unwrap_or_default();
        let mut latest: Vec<(String, String, Vec<MediaItem>, usize)> = Vec::new();
        for v in user_views.iter().filter(|v| {
            let lower = v.name.to_lowercase();
            !self.hidden_latest.contains(&lower) && !self.hidden_libraries.contains(&lower)
        }) {
            let title = format!("Latest {}", v.name);
            let items = if v.collection_type == "tvshows" {
                client.get_latest_episodes(&v.id, 15).unwrap_or_default()
            } else {
                client.get_latest(&v.id, 15).unwrap_or_default()
            };
            let cursor = old_cursors.get(&v.id).copied().unwrap_or(0)
                .min(items.len().saturating_sub(1));
            latest.push((title, v.id.clone(), items, cursor));
        }
        drop(client);
        self.home.latest = latest;

        let n = 1 + self.home.latest.len();
        if self.home.section >= n {
            self.home.section = n.saturating_sub(1);
        }
        self.ensure_home_section_visible();
        Ok(())
    }

    fn handle_ws_event(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Play { item_ids, play_now, start_position_ticks, start_index } => {
                self.log.push(Level::Info, "ws", format!("Play: {} id(s), play_now={play_now}", item_ids.len()));
                if !play_now { return; }
                let items = {
                    let c = self.client.lock().unwrap();
                    match c.get_items_by_ids(&item_ids) {
                        Ok(v) => v,
                        Err(e) => { self.status = format!("WS play error: {e}"); return; }
                    }
                };
                if items.is_empty() {
                    self.log.push(Level::Warn, "ws", format!("Play: no items found for ids={}", item_ids.join(",")));
                    return;
                }
                let start_idx = start_index.min(items.len().saturating_sub(1));
                self.tab_idx = 1;
                if items.len() == 1 {
                    let mut item = items[0].clone();
                    if start_position_ticks > 0 { item.playback_position_ticks = start_position_ticks; }
                    self.player_tab.items = vec![item.clone()];
                    self.player_tab.playlist_cursor = 0;
                    self.flash_status(item.playback_label());
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    self.player.play(&item, c, self.log.clone(), self.ui_volume);
                } else {
                    let count = items.len();
                    self.player_tab.items = items.clone();
                    self.player_tab.playlist_cursor = start_idx;
                    self.status = format!("Playing {count} items");
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    let active = self.player.status.lock().unwrap().active;
                    self.log.push(Level::Info, "ws", format!("Play multi: active={active}, count={count}, start_idx={start_idx}"));
                    if active {
                        let mut start_item = items[start_idx].clone();
                        if start_position_ticks > 0 { start_item.playback_position_ticks = start_position_ticks; }
                        self.player.play(&start_item, c, self.log.clone(), self.ui_volume);
                    } else {
                        let mut items_with_pos = items.clone();
                        if start_position_ticks > 0 {
                            items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                        }
                        self.player.play_playlist(items_with_pos, start_idx, c, self.log.clone(), self.ui_volume);
                    }
                }
            }
            WsEvent::Stop => { self.player.stop(); }
            WsEvent::Pause => {
                if !self.player.status.lock().unwrap().paused {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            WsEvent::Unpause => {
                if self.player.status.lock().unwrap().paused {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            WsEvent::NextTrack => {
                let idx = self.player.status.lock().unwrap().current_idx;
                if idx + 1 < self.player_tab.items.len() {
                    self.player.send_command(PlayerCommand::JumpTo(idx + 1));
                }
            }
            WsEvent::PreviousTrack => {
                let idx = self.player.status.lock().unwrap().current_idx;
                if idx > 0 { self.player.send_command(PlayerCommand::JumpTo(idx - 1)); }
            }
            WsEvent::TogglePause => {
                self.player.send_command(PlayerCommand::TogglePause);
            }
            WsEvent::Seek(ticks) => {
                self.player.send_command(PlayerCommand::SeekAbsolute(
                    ticks as f64 / TICKS_PER_SECOND as f64,
                ));
            }
            WsEvent::SeekRelative(secs) => {
                self.player.send_command(PlayerCommand::Seek(secs));
            }
            WsEvent::SetVolume(v) => {
                let vol_max = self.player.status.lock().unwrap().volume_max;
                self.player.send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
            }
            WsEvent::VolumeUp => {
                let st = self.player.status.lock().unwrap();
                let v = (st.volume + 5).min(st.volume_max);
                drop(st);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::VolumeDown => {
                let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::UserDataChanged => { let _ = self.fetch_home(); }
        }
    }

    fn update_lib_search(&mut self, lib_idx: usize) {
        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;

        let query = match self.libs[lib_idx].search.as_ref() {
            Some(s) => s.query.clone(),
            None => return,
        };

        if query.is_empty() {
            if let Some(s) = self.libs[lib_idx].search.as_mut() {
                let n = s.items.len();
                s.results = (0..n).collect();
                s.cursor = 0;
            }
            return;
        }

        let scored: Vec<(i64, usize)> = {
            let items = self.libs[lib_idx].search.as_ref()
                .map(|s| s.items.as_slice())
                .unwrap_or(&[]);
            let matcher = SkimMatcherV2::default();
            items.iter().enumerate()
                .filter_map(|(i, item)| matcher.fuzzy_match(&item.display_name(), &query).map(|s| (s, i)))
                .collect()
        };

        let mut results: Vec<(i64, usize)> = scored;
        results.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
        let results: Vec<usize> = results.into_iter().map(|(_, i)| i).collect();

        if let Some(s) = self.libs[lib_idx].search.as_mut() {
            s.results = results;
            s.cursor = 0;
        }
    }

    // ── rendering ────────────────────────────────────────────────────────────

    fn render(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let status_h:   u16 = if show_controls { 1 } else { 0 };
        let controls_h: u16 = if show_controls { 2 } else { 0 };

        let [tabs_area, gap_area, toast_area, controls_area, status_area, main_area] = Layout::vertical([
            Constraint::Length(1),            // tabs
            Constraint::Length(1),            // spacer
            Constraint::Length(1),            // toast notifications
            Constraint::Length(controls_h),   // playback controls (when active)
            Constraint::Length(status_h),     // now-playing title bar (when active)
            Constraint::Min(0),               // main content
        ]).areas(area);

        // Right side: Vol (14) cols total
        const VOL_W:  u16 = 14; // " Volume: XXX%"
        let right_w = VOL_W;

        // Thin underline below tab row, with [字] sub and [♪:🏳] audio indicators embedded inline
        {
            let (sub_active, player_active, audio_muted, audio_label) =
                if let Some(ref remote) = self.connected_session_state {
                    let sub_on = remote.sub_index != -1;
                    let muted  = remote.volume == 0;
                    (sub_on, true, muted, None::<String>)
                } else {
                    let s = self.player.status.lock().unwrap();
                    let sub_on = !self.player.subs_off.load(std::sync::atomic::Ordering::Relaxed);
                    let muted = s.active && (s.audio_id == 0 || self.ui_volume == 0);
                    let aud_label = if s.active && s.audio_id != 0 {
                        s.audio_tracks.iter().find(|(id, _)| *id == s.audio_id)
                            .map(|(_, l)| l.clone())
                    } else { None };
                    (sub_on, s.active, muted, aud_label)
                };
            let is_audio_item = self.is_audio_item();
            // audio_icon: single 2-cell glyph — muted, speaker (audio), flag, or ♪
            let audio_icon: &str = if audio_muted {
                "\u{1F507}"   // 🔇 muted
            } else if is_audio_item {
                "\u{1F50A}"   // 🔊 speaker (audio file, unmuted)
            } else {
                audio_label.as_deref()
                    .map(crate::player::lang_to_flag)
                    .map(|f| if f.is_empty() { "\u{1F509}" } else { f })
                    .unwrap_or("\u{1F509}")
            };

            // right-to-left: ───[℻]─[字]─[♫]──── leading dashes
            const AUD_W:     u16 = 4;
            const SUB_W:     u16 = 4;
            const SESS_W:    u16 = 3; // [✚] — ✚ is 1 col wide
            const SEP_W:     u16 = 1;
            const RIGHT_PAD: u16 = 1;
            let sess_end   = (gap_area.width as usize).saturating_sub(RIGHT_PAD as usize);
            let sess_start = sess_end.saturating_sub(SESS_W as usize);
            let sub_end    = sess_start.saturating_sub(SEP_W as usize);
            let sub_start  = sub_end.saturating_sub(SUB_W as usize);
            let aud_end    = sub_start.saturating_sub(SEP_W as usize);
            let aud_start  = aud_end.saturating_sub(AUD_W as usize);

            let dash      = Style::default().fg(palette::MUTED);
            let bra       = Style::default().fg(palette::MUTED);
            let mut spans: Vec<Span> = Vec::new();
            let sub_color  = if sub_active { palette::RED } else { palette::MUTED };
            let sess_color = if self.connected_session_id.is_some() { palette::IRIS } else { palette::MUTED };
            spans.push(Span::styled("─".repeat(aud_start), dash));
            if player_active {
                let icon_color = if audio_muted { palette::MUTED } else { palette::IRIS };
                spans.push(Span::styled("[",        bra));
                spans.push(Span::styled(audio_icon, Style::default().fg(icon_color)));
                spans.push(Span::styled("]",        bra));
            } else {
                spans.push(Span::styled("─".repeat(AUD_W as usize), dash));
            }
            spans.push(Span::styled("─".repeat(SEP_W as usize), dash));
            spans.push(Span::styled("[",  bra));
            spans.push(Span::styled("字", Style::default().fg(sub_color)));
            spans.push(Span::styled("]",  bra));
            spans.push(Span::styled("─".repeat(SEP_W as usize), dash));
            spans.push(Span::styled("[",            bra));
            spans.push(Span::styled("\u{271A}",     Style::default().fg(sess_color)));
            spans.push(Span::styled("]",            bra));
            spans.push(Span::styled("─".repeat(RIGHT_PAD as usize), dash));
            f.render_widget(Paragraph::new(Line::from(spans)), gap_area);

            let aud_x  = gap_area.x + aud_start as u16;
            self.layout_audio_indicator_area = if player_active {
                Rect { x: aud_x, y: gap_area.y, width: AUD_W, height: 1 }
            } else {
                Rect::default()
            };
            let sub_x  = gap_area.x + sub_start as u16;
            self.layout_sub_indicator_area   = Rect { x: sub_x,  y: gap_area.y, width: SUB_W,  height: 1 };
            let sess_x = gap_area.x + sess_start as u16;
            self.layout_sessions_btn_area    = Rect { x: sess_x, y: gap_area.y, width: SESS_W, height: 1 };
        }
        let vol_area = Rect {
            x: tabs_area.x + tabs_area.width.saturating_sub(right_w),
            y: tabs_area.y, width: VOL_W, height: 1,
        };
        self.layout_tabbar_vol_area = vol_area;
        self.render_volume_bar(f, vol_area);
        let tabs_area = Rect { width: tabs_area.width.saturating_sub(right_w), ..tabs_area };
        self.layout_tabs_area = tabs_area;

        // Tab bar with scroll indicators when tabs overflow
        let (vis_start, vis_end) = self.visible_tab_range(tabs_area.width);
        let has_left  = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let ind_style = Style::default().fg(palette::WHITE);
        let left_w:  u16 = if has_left  { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left {
            f.render_widget(
                Paragraph::new("« ").style(ind_style),
                Rect { x: tabs_area.x, y: tabs_area.y, width: 2, height: 1 },
            );
        }
        if has_right {
            f.render_widget(
                Paragraph::new(" »").style(ind_style),
                Rect { x: tabs_area.x + tabs_area.width - 2, y: tabs_area.y, width: 2, height: 1 },
            );
        }
        let inner_tabs = Rect {
            x: tabs_area.x + left_w,
            y: tabs_area.y,
            width: tabs_area.width.saturating_sub(left_w + right_w),
            height: tabs_area.height,
        };
        let all_names: Vec<String> = std::iter::once("Home".to_string())
            .chain(std::iter::once("Queue".to_string()))
            .chain(self.libs.iter().map(|l| l.library.name.clone()))
            .chain(self.show_log_tab.then(|| "Log".to_string()))
            .collect();
        let selected_tab = if (!self.show_log_tab && self.tab_idx == self.log_tab_idx()) || self.tab_idx < vis_start || self.tab_idx >= vis_end {
            usize::MAX
        } else {
            self.tab_idx - vis_start
        };
        let tab_titles: Vec<Span> = all_names[vis_start..vis_end]
            .iter().enumerate().map(|(i, n)| {
                if i == selected_tab {
                    Span::styled(format!("  {n}  "), Style::default().fg(palette::WHITE).bg(palette::IRIS).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled(format!("  {n}  "), Style::default().fg(palette::SUBTLE))
                }
            }).collect();
        f.render_widget(
            Tabs::new(tab_titles)
                .select(usize::MAX)
                .style(Style::default().fg(palette::SUBTLE))
                .highlight_style(Style::default())
                .divider(Span::raw(""))
                .padding("", ""),
            inner_tabs,
        );

        // When playing: derive title from player state to avoid races with PlayerEvent::Stopped.
        // Otherwise show transient status messages.
        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            self.player_tab.items.get(idx).map(|i| i.playback_label())
        } else {
            None
        };
        // Expire flash status
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
        }
        // Compute now-playing title for use in toast row fallback
        let now_playing_title: Option<(String, ratatui::style::Color)> = if show_controls {
            if active {
                now_playing.map(|t| (t, palette::FOAM))
            } else if let Some(ref state) = self.connected_session_state {
                state.now_playing.clone().map(|t| (t, palette::IRIS))
            } else {
                None
            }
        } else {
            None
        };
        // Toast area: search query when active, then flash message, then now-playing title
        let search_toast: Option<String> = if self.tab_idx >= self.lib_tab_offset()
            && self.tab_idx != self.log_tab_idx()
        {
            let li = self.tab_idx - self.lib_tab_offset();
            self.libs.get(li).and_then(|l| {
                l.search.as_ref().map(|s| format!("Search {}: {}█", l.library.name, s.query))
            })
        } else {
            None
        };
        if let Some(ref q) = search_toast {
            f.render_widget(
                Paragraph::new(q.as_str())
                    .style(Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                toast_area,
            );
        } else if !self.status.is_empty() {
            f.render_widget(
                Paragraph::new(self.status.as_str())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                toast_area,
            );
        } else if let Some((ref title, color)) = now_playing_title {
            f.render_widget(
                Paragraph::new(title.as_str())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                toast_area,
            );
        }
        if show_controls {
            // Status / now-playing HR (title now lives in the toast row)
            f.render_widget(
                Paragraph::new(Span::styled(
                    "─".repeat(area.width as usize),
                    Style::default().fg(palette::MUTED),
                )),
                status_area,
            );
            self.render_playback_controls(f, controls_area);
        } else {
            self.layout_seekbar_area = Rect::default();
            self.layout_button_area  = Rect::default();
            self.layout_tracks_area  = Rect::default();
            self.layout_vol_area     = Rect::default();
            self.layout_sub_area     = Rect::default();
            self.layout_audio_area   = Rect::default();
        }

        if self.tab_idx == 0 {
            self.render_combined(f, main_area);
        } else if self.tab_idx == 1 {
            self.render_playlist_panel(f, main_area);
        } else if self.tab_idx == self.log_tab_idx() {
            self.render_log(f, main_area);
        } else {
            self.render_library(f, main_area, self.tab_idx - self.lib_tab_offset());
        }

        self.render_context_menu(f);

        if self.show_sessions  { self.render_sessions_overlay(f); }
        if self.show_help      { self.render_help_panel(f); }
        if self.show_settings  {
            self.render_settings_panel(f);
            if self.multiselect_popup.is_some() { self.render_multiselect_popup(f); }
        }
    }

    fn render_volume_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let (volume, _volume_max) = if let Some(ref remote) = self.connected_session_state {
            (remote.volume, 100)
        } else {
            let s = self.player.status.lock().unwrap();
            if s.active { (s.volume, s.volume_max) }
            else { (self.ui_volume as i64, 100) }
        };
        let color = if volume > 100 { palette::RED }
            else if volume > 60 { palette::YELLOW }
            else { palette::PINE };
        let line = Line::from(vec![
            Span::styled(" Volume: ", Style::default().fg(Color::Rgb(230, 230, 230))),
            Span::styled(format!("{}%", volume), Style::default().fg(color)),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    fn render_panel_shell(
        f: &mut ratatui::Frame,
        full: Rect,
        width: u16,
        icon: &str,
        title: &str,
        hints: &str,
    ) -> Rect {
        let sidebar = Rect { x: full.x, y: full.y, width: width.min(full.width), height: full.height };
        f.render_widget(Clear, sidebar);
        f.render_widget(Block::default().style(Style::default().bg(palette::BASE)), sidebar);
        for row in sidebar.y..sidebar.y + sidebar.height {
            f.render_widget(
                Paragraph::new(Span::styled("\u{2502}", Style::default().fg(palette::OVERLAY))),
                Rect { x: sidebar.x + sidebar.width - 1, y: row, width: 1, height: 1 },
            );
        }
        let inner_w = sidebar.width.saturating_sub(2);
        let ix = sidebar.x + 1;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(palette::IRIS)),
                Span::styled(title.to_owned(), Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD)),
            ])).style(Style::default().bg(palette::FOCUSED)),
            Rect { x: ix, y: sidebar.y, width: inner_w, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect { x: sidebar.x + sidebar.width - 1, y: sidebar.y, width: 1, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::styled("\u{2500}".repeat(inner_w as usize), Style::default().fg(palette::OVERLAY))),
            Rect { x: ix, y: sidebar.y + 1, width: inner_w, height: 1 },
        );
        let footer_y = sidebar.y + sidebar.height - 2;
        f.render_widget(
            Paragraph::new(Span::styled("\u{2500}".repeat(inner_w as usize), Style::default().fg(palette::OVERLAY))),
            Rect { x: ix, y: footer_y, width: inner_w, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Span::styled(trunc_str(hints, inner_w as usize), Style::default().fg(palette::MUTED))),
            Rect { x: ix, y: footer_y + 1, width: inner_w, height: 1 },
        );
        Rect { x: ix, y: sidebar.y + 2, width: inner_w, height: sidebar.height.saturating_sub(4) }
    }

    fn render_sessions_overlay(&self, f: &mut ratatui::Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), SESSIONS_PANEL_W,
            "\u{271A}", "Remote Sessions",
            "[↑↓]select [↵]connect [d]disc [r]refresh [Esc]close",
        );
        let ix = content.x;
        let inner_w = content.width;
        let iw = inner_w as usize;
        let list_y = content.y;
        let list_h = content.height;
        let list_area = content;

        if self.sessions_loading && self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" Loading\u{2026}", Style::default().fg(palette::SUBTLE))),
                list_area,
            );
            return;
        }
        if self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(" No other active sessions", Style::default().fg(palette::SUBTLE))),
                list_area,
            );
            return;
        }

        const CARD_H: u16 = 3;
        const DIV_H:  u16 = 1;
        let entry_h = CARD_H + DIV_H;

        for (i, s) in self.sessions.iter().enumerate() {
            let entry_y = list_y + i as u16 * entry_h;
            if entry_y + CARD_H > list_y + list_h { break; }

            let selected = i == self.sessions_cursor;
            let is_connected = self.connected_session_id.as_deref() == Some(s.id.as_str());
            let bg = if selected { palette::FOCUSED } else { palette::BASE };
            let name_color = if selected { palette::IRIS } else { palette::TEXT };
            let dim = Style::default().fg(palette::MUTED).bg(bg);
            let card_style = Style::default().bg(bg);

            let prefix = if selected { "\u{25b6} " } else { "  " };
            let badge = if is_connected { " \u{271A}" } else { "" };
            let name_max = iw.saturating_sub(prefix.len() + badge.len());
            let name_line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(palette::IRIS).bg(bg)),
                Span::styled(trunc_str(&s.device_name, name_max), Style::default().fg(name_color).bg(bg).add_modifier(Modifier::BOLD)),
                Span::styled(badge, Style::default().fg(palette::IRIS).bg(bg)),
            ]);
            f.render_widget(Paragraph::new(name_line).style(card_style), Rect { x: ix, y: entry_y, width: inner_w, height: 1 });

            let meta = format!("  {} \u{b7} {}@{}", s.client, s.user_name, s.host);
            f.render_widget(
                Paragraph::new(Span::styled(trunc_str(&meta, iw), dim.fg(palette::SUBTLE))),
                Rect { x: ix, y: entry_y + 1, width: inner_w, height: 1 },
            );

            let state_icon = if s.now_playing.is_some() {
                if s.is_paused { "\u{23f8}" } else { "\u{25b6}" }
            } else { "\u{25a0}" };
            let time = if s.now_playing.is_some() {
                format!(" {}/{}", fmt_duration(s.position_s), fmt_duration(s.runtime_s))
            } else { String::new() };
            let title = s.now_playing.as_deref().unwrap_or("idle");
            let playing = format!("  {} {}{}", state_icon, trunc_str(title, iw.saturating_sub(12)), time);
            f.render_widget(
                Paragraph::new(Span::styled(trunc_str(&playing, iw), dim)),
                Rect { x: ix, y: entry_y + 2, width: inner_w, height: 1 },
            );

            if entry_y + entry_h <= list_y + list_h {
                f.render_widget(
                    Paragraph::new(Span::styled("\u{2500}".repeat(iw), Style::default().fg(palette::OVERLAY))),
                    Rect { x: ix, y: entry_y + CARD_H, width: inner_w, height: 1 },
                );
            }
        }
    }


    fn render_help_panel(&mut self, f: &mut ratatui::Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), HELP_PANEL_W,
            "\u{2328}", "Keyboard Shortcuts",
            "[↑↓]scroll [Esc]close",
        );
        let w = content.width as usize;
        let key_w = 20usize;

        let mk = |key: &str, desc: &str| -> Line<'static> {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{:<kw$}", key, kw = key_w),
                    Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_owned(), Style::default().fg(palette::SUBTLE)),
            ])
        };
        let section = |label: &str| -> Line<'static> {
            let dash_count = w.saturating_sub(2 + label.len() + 1);
            Line::from(vec![
                Span::raw("  "),
                Span::styled(label.to_owned(), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {}", "─".repeat(dash_count)),
                    Style::default().fg(palette::OVERLAY),
                ),
            ])
        };
        let blank = || Line::from("");

        let show_log = self.show_log_tab;
        let mut lines: Vec<Line> = vec![
            blank(),
            section("GLOBAL"),
            mk("F1",               "Help"),
            mk("F2",               "Settings"),
            mk("F3",               "Remote sessions"),
            mk("F5",               "Refresh current view"),
            mk("Tab",              "Cycle menu"),
            mk("1 – 9",            "Jump to tab"),
            mk("↑ / ↓",            "Move cursor"),
            mk("PgUp / PgDn",      "Page scroll"),
            mk("Home / End",       "First / last"),
            mk("Enter",            "Select / Play / Open"),
            mk("o",                "Context menu"),
            mk("c",                "Clear Queue (confirms)"),
            mk("q",                "Quit"),
        ];
        lines.extend(vec![
            blank(),
            section("PLAYBACK"),
            mk("Space",            "Pause / Resume"),
            mk("< / >",            "Seek ±5 seconds"),
            mk("Alt+Enter",        "Stop"),
            mk("- / +",            "Volume down / up"),
            mk("a",                "Cycle audio track"),
            mk("z",                "Enable subtitles"),

            blank(),
            section("QUEUE"),
            mk(".",                "Jump to playing item"),
            mk("Del",              "Remove from Queue"),
            mk("v",                "Toggle view"),

            blank(),
            section("HOME"),
            mk("Alt+↑ / ↓",        "Switch sections"),
            mk("Ctrl+W",           "Toggle watched"),
            mk("Ctrl+Q",           "Add to Queue"),

            blank(),
            section("LIBRARY"),
            mk("Esc / Backspace",  "Go back"),
            mk("/",                "Search library"),
            mk("Ctrl+W",           "Toggle watched"),
            mk("Ctrl+S",           "Shuffle and play selection"),
            mk("Ctrl+P",           "Play all (recursive)"),
            mk("Ctrl+Q",           "Add to Queue"),

            blank(),
        ]);
        if show_log {
            lines.extend(vec![
                section("LOG"),
                mk("Alt+L",            "Open Log"),
                mk("← / →",            "Switch pane (Sources / Log)"),
                mk("↑ / ↓",            "Scroll log / navigate sources"),
                mk("PgUp / PgDn",      "Page scroll"),
                mk("Space",            "Toggle source on/off"),
                mk("c",                "Copy log to clipboard"),
                blank(),
                blank(),
            ]);
        }

        let total = lines.len();
        let visible = content.height as usize;
        self.help_scroll = self.help_scroll.min(total.saturating_sub(visible) as u16);
        f.render_widget(Paragraph::new(lines).scroll((self.help_scroll, 0)), content);
    }

    fn close_settings(&mut self) {
        if self.settings_save_at.take().is_some() {
            let cfg = self.client.lock().unwrap().config.clone();
            crate::config::save_config_settings(&cfg);
        }
        self.show_settings = false;
    }

    fn handle_settings_activate(&mut self) {
        let key = settings_cursor_to_key(self.settings_cursor);
        match key {
            SettingKey::HiddenLibraries => { self.open_multiselect_popup(MultiSelectKind::HiddenLibraries); return; }
            SettingKey::HiddenLatest    => { self.open_multiselect_popup(MultiSelectKind::HiddenLatest);    return; }
            SettingKey::LogOut => { self.confirm_logout = true; }
            SettingKey::ImageProtocol => {
                let now_none = {
                    let mut c = self.client.lock().unwrap();
                    c.config.image_protocol = match c.config.image_protocol.as_deref() {
                        None               => Some("halfblocks".into()),
                        Some("halfblocks") => Some("sixel".into()),
                        Some("sixel")      => Some("kitty".into()),
                        Some("kitty")      => Some("iterm2".into()),
                        Some("iterm2")     => Some("auto".into()),
                        _                  => None,
                    };
                    c.config.image_protocol.is_none()
                };
                if now_none {
                    self.home_card_view = false;
                    self.playlist_view = 0;
                    self.save_prefs();
                }
            }
            SettingKey::ShowLogTab => {
                let new_val = {
                    let mut c = self.client.lock().unwrap();
                    c.config.show_log_tab = !c.config.show_log_tab;
                    c.config.show_log_tab
                };
                self.show_log_tab = new_val;
            }
            _ => {
                let mut c = self.client.lock().unwrap();
                match key {
                    SettingKey::DaemonModeOnExit => c.config.daemon_mode_on_exit = !c.config.daemon_mode_on_exit,
                    SettingKey::StartOnQueue     => c.config.start_on_queue = !c.config.start_on_queue,
                    SettingKey::AlwaysPlayNext      => c.config.always_play_next = !c.config.always_play_next,
                    SettingKey::ConsumeVideos => c.config.consume_videos = !c.config.consume_videos,
                    SettingKey::AlwaysSkipIntro     => c.config.always_skip_intro = !c.config.always_skip_intro,
                    SettingKey::ShowAudioWindow  => c.config.show_audio_window = !c.config.show_audio_window,
                    SettingKey::UseMpvConfig     => c.config.use_mpv_config = !c.config.use_mpv_config,
                    SettingKey::NoScripts        => c.config.no_scripts = !c.config.no_scripts,
                    SettingKey::ShowSysTrayIcon  => c.config.show_systray_icon = !c.config.show_systray_icon,
                    _ => {}
                }
            }
        }
        self.settings_save_at = Some(Instant::now() + Duration::from_millis(500));
    }

    fn settings_scroll_follow(&mut self) {} // no-op: grid layout always fits

    fn open_multiselect_popup(&mut self, kind: MultiSelectKind) {
        let client = self.client.lock().unwrap();
        let all = match kind {
            MultiSelectKind::HiddenLibraries => client.get_views().unwrap_or_default(),
            MultiSelectKind::HiddenLatest    => client.get_user_views().unwrap_or_default(),
        };
        let hidden_list = match kind {
            MultiSelectKind::HiddenLibraries => &client.config.hidden_libraries,
            MultiSelectKind::HiddenLatest    => &client.config.hidden_latest,
        };
        let items: Vec<(String, String, bool)> = all.iter().map(|v| {
            let lower = v.name.to_lowercase();
            let is_hidden = hidden_list.contains(&lower);
            (lower, v.name.clone(), is_hidden)
        }).collect();
        drop(client);
        self.multiselect_popup = Some(MultiSelectPopup { kind, items, cursor: 0 });
    }

    fn close_multiselect_popup(&mut self) {
        let Some(popup) = self.multiselect_popup.take() else { return; };
        let hidden: Vec<String> = popup.items.iter()
            .filter(|(_, _, is_hidden)| *is_hidden)
            .map(|(lower, _, _)| lower.clone())
            .collect();
        {
            let mut c = self.client.lock().unwrap();
            match popup.kind {
                MultiSelectKind::HiddenLibraries => c.config.hidden_libraries = hidden.clone(),
                MultiSelectKind::HiddenLatest    => c.config.hidden_latest    = hidden.clone(),
            }
        }
        match popup.kind {
            MultiSelectKind::HiddenLibraries => self.hidden_libraries = hidden,
            MultiSelectKind::HiddenLatest    => self.hidden_latest    = hidden,
        }
        let cfg = self.client.lock().unwrap().config.clone();
        crate::config::save_config_settings(&cfg);
        let _ = self.fetch_home();
    }

    fn render_multiselect_popup(&mut self, f: &mut ratatui::Frame) {
        let Some(ref popup) = self.multiselect_popup else { return; };
        let title = match popup.kind {
            MultiSelectKind::HiddenLibraries => " Hidden Libraries ",
            MultiSelectKind::HiddenLatest    => " Hidden Latest ",
        };
        let max_name = popup.items.iter().map(|(_, n, _)| n.len()).max().unwrap_or(0);
        let inner_w = ((max_name + 6) as u16).max(36).min(60); // "▸ [x] " = 6 chars
        let width = inner_w + 2;
        let content_h = popup.items.len() as u16 + 1; // items + hint line
        let area = f.area();
        let height = (content_h + 2).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };

        f.render_widget(Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Span::styled(title, Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let hint = "Space toggle  \u{b7}  Esc / Enter close";
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(palette::MUTED))),
            Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
        );

        let list_area = Rect {
            x: inner.x, y: inner.y + 1,
            width: inner.width, height: inner.height.saturating_sub(1),
        };
        let list_h = list_area.height as usize;
        let cursor = popup.cursor;
        let scroll = if cursor >= list_h { cursor + 1 - list_h } else { 0 };

        let lines: Vec<Line> = popup.items.iter().enumerate()
            .skip(scroll).take(list_h)
            .map(|(i, (_, name, is_hidden))| {
                let focused = i == cursor;
                let arrow = if focused { "\u{25b8} " } else { "  " };
                let check = if *is_hidden { "[x]" } else { "[ ]" };
                let check_style = if focused {
                    Style::default().fg(palette::FOAM)
                } else {
                    Style::default().fg(palette::MUTED)
                };
                let name_style = if focused {
                    Style::default().fg(palette::TEXT)
                } else {
                    Style::default().fg(palette::SUBTLE)
                };
                Line::from(vec![
                    Span::raw(arrow),
                    Span::styled(check, check_style),
                    Span::raw(" "),
                    Span::styled(name.clone(), name_style),
                ])
            }).collect();
        f.render_widget(Paragraph::new(lines), list_area);
    }

    fn render_settings_panel(&mut self, f: &mut ratatui::Frame) {
        let content = Self::render_panel_shell(
            f, f.area(), SETTINGS_PANEL_W,
            "\u{22ee}", "Settings",
            "[↑↓]navigate [Space/\u{21b5}]toggle [Esc]close",
        );
        let cfg = self.client.lock().unwrap().config.clone();
        let cursor = self.settings_cursor;
        let confirm_logout = self.confirm_logout;
        let label_w = 22usize;
        let w = content.width as usize;

        let data_sections = &SETTING_SECTIONS[..SETTING_SECTIONS.len() - 1];

        let mut lines: Vec<Line> = vec![Line::from("")];
        let mut cursor_line = 0usize;
        let mut item_idx = 0usize;

        for (sec_name, keys) in data_sections {
            let dash_count = w.saturating_sub(2 + sec_name.len() + 1);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled((*sec_name).to_owned(), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", "\u{2500}".repeat(dash_count)), Style::default().fg(palette::OVERLAY)),
            ]));
            for &key in *keys {
                if item_idx == cursor { cursor_line = lines.len(); }
                let focused = item_idx == cursor;
                let arrow = if focused { "\u{25b8} " } else { "  " };
                let label = setting_label(key);
                let val = setting_value(key, &cfg);
                let label_style = if focused { Style::default().fg(palette::TEXT) } else { Style::default().fg(palette::MUTED) };
                lines.push(Line::from(vec![
                    Span::raw(arrow),
                    Span::styled(format!("{:<lw$}", label, lw = label_w), label_style),
                    Span::styled(val, Style::default().fg(palette::FOAM)),
                ]));
                item_idx += 1;
            }
            lines.push(Line::from(""));
        }

        let logout_cursor_idx = settings_total_rows() - 1;
        if cursor == logout_cursor_idx { cursor_line = lines.len(); }
        let focused = cursor == logout_cursor_idx;
        let (logout_text, logout_style) = if confirm_logout && focused {
            ("\u{25b8} Log out? Press y to confirm", Style::default().fg(palette::RED))
        } else if focused {
            ("\u{25b8} Log out", Style::default().fg(palette::RED))
        } else {
            ("  Log out", Style::default().fg(palette::MUTED))
        };
        lines.push(Line::from(Span::styled(logout_text, logout_style)));

        let visible = content.height as usize;
        if cursor_line < self.settings_scroll {
            self.settings_scroll = cursor_line;
        } else if cursor_line >= self.settings_scroll + visible {
            self.settings_scroll = cursor_line + 1 - visible;
        }
        let total = lines.len();
        self.settings_scroll = self.settings_scroll.min(total.saturating_sub(visible));

        f.render_widget(Paragraph::new(lines).scroll((self.settings_scroll as u16, 0)), content);
    }

    fn render_context_menu(&mut self, f: &mut ratatui::Frame) {
        let Some(ref menu) = self.context_menu else {
            self.context_menu_rect = None;
            return;
        };
        let width = (menu.items.iter().map(|s| s.len()).max().unwrap_or(4) + 4) as u16;
        let height = menu.items.len() as u16 + 2;
        let full = f.area();
        let x = menu.x.min(full.width.saturating_sub(width));
        let y = menu.y.min(full.height.saturating_sub(height));
        let rect = Rect { x, y, width, height };
        self.context_menu_rect = Some(rect);
        f.render_widget(Clear, rect);
        f.render_widget(
            Block::default()
                .borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS)),
            rect,
        );
        let list_items: Vec<ListItem> = menu.items.iter().enumerate().map(|(i, &label)| {
            let style = if i == menu.cursor {
                Style::default().fg(palette::BASE).bg(palette::IRIS)
            } else {
                Style::default().fg(palette::TEXT)
            };
            ListItem::new(format!(" {label} ")).style(style)
        }).collect();
        let inner = Rect { x: x + 1, y: y + 1, width: width - 2, height: height - 2 };
        f.render_widget(List::new(list_items), inner);
    }

    fn render_playback_controls(&mut self, f: &mut ratatui::Frame, area: Rect) {
        if area.height == 0 { return; }

        let inner_w = area.width.min(100);
        let inner_x = area.x + (area.width.saturating_sub(inner_w)) / 2;
        let area = Rect { x: inner_x, y: area.y, width: inner_w, height: area.height };

        let (position_ticks, runtime_ticks, paused) = if let Some(ref remote) = self.connected_session_state {
            let pos = if remote.is_paused {
                self.remote_pos_s
            } else {
                (self.remote_pos_s + self.remote_pos_at.elapsed().as_secs() as i64)
                    .min(remote.runtime_s)
            };
            (
                pos * crate::api::TICKS_PER_SECOND,
                remote.runtime_s * crate::api::TICKS_PER_SECOND,
                remote.is_paused,
            )
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        };

        let pos_s = position_ticks / TICKS_PER_SECOND;
        let dur_s = runtime_ticks / TICKS_PER_SECOND;

        let pos_str = fmt_duration(pos_s);
        let dur_str = fmt_duration(dur_s);

        let btn_style = Style::default().fg(Color::Rgb(203, 212, 241));
        let pp_icon   = if !paused { "\u{f04c}" } else { "\u{f04b}" };
        let btn_icons = ["\u{f048}", "\u{f04a}", pp_icon, "\u{f04d}", "\u{f04e}", "\u{f051}"];
        // Buttons: 6 buttons × 5 cells = 30 cells total
        let mut btn_spans: Vec<Span> = Vec::new();
        for icon in btn_icons.iter() {
            let style = btn_style;
            btn_spans.push(Span::styled(format!("  {icon}  "), style));
        }

        const BTNS_W: u16 = 30; // 6 buttons × 5
        let btn_x = area.x + area.width.saturating_sub(BTNS_W) / 2;

        let btn_row_y = area.y + 1;

        self.layout_seekbar_area      = Rect { x: area.x, y: area.y,    width: area.width, height: 1 };
        self.layout_button_area       = Rect { x: btn_x,  y: btn_row_y, width: BTNS_W,     height: 1 };
        self.layout_tracks_area       = Rect::default();
        self.layout_vol_area          = Rect::default();
        self.layout_sub_area          = Rect::default();
        self.layout_audio_area        = Rect::default();

        // Row 0 — seekbar
        let ratio = if runtime_ticks > 0 {
            (position_ticks as f64 / runtime_ticks as f64).clamp(0.0, 1.0)
        } else { 0.0 };
        let seek_rect = Rect { x: area.x, y: area.y, width: area.width, height: 1 };
        let bar_w = seek_rect.width as usize;
        let filled = (ratio * bar_w as f64).round() as usize;
        let unfilled = bar_w.saturating_sub(filled);
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("\u{2501}".repeat(filled),   Style::default().fg(palette::IRIS)),
            Span::styled("\u{2500}".repeat(unfilled), Style::default().fg(palette::IRIS_DIM)),
        ])), seek_rect);

        // Row 1 — elapsed (left), buttons (center), total (right)
        let time_style = Style::default().fg(palette::MUTED);
        let elapsed_w = pos_str.chars().count() as u16;
        let total_w   = dur_str.chars().count() as u16;
        f.render_widget(
            Paragraph::new(Span::styled(pos_str, time_style)),
            Rect { x: area.x, y: btn_row_y, width: elapsed_w.min(area.width), height: 1 },
        );
        f.render_widget(
            Paragraph::new(Line::from(btn_spans)).alignment(Alignment::Center),
            Rect { x: area.x, y: btn_row_y, width: area.width, height: 1 },
        );
        let total_x = area.x + area.width.saturating_sub(total_w);
        f.render_widget(
            Paragraph::new(Span::styled(dur_str, time_style)),
            Rect { x: total_x, y: btn_row_y, width: total_w.min(area.width), height: 1 },
        );
    }

    fn render_combined(&mut self, f: &mut ratatui::Frame, area: Rect) {
        self.home_rect = area;
        self.layout_carousel_left_arrow = None;
        self.layout_carousel_right_arrow = None;
        self.layout_carousel_up_arrow = None;
        self.layout_carousel_down_arrow = None;
        if self.home_card_view {
            self.render_home_cards(f, area);
        } else {
            self.render_home_panel(f, area);
        }
    }

    fn render_playlist_panel(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let (active, current_idx, live_pos, live_runtime) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx, s.position_ticks, s.runtime_ticks)
        };

        self.playlist_rect = area;

        if self.playlist_view == 1 {
            let v_pad: u16 = if area.height >= 30 { 2 } else if area.height >= 20 { 1 } else { 0 };
            let inner = Rect {
                x: area.x,
                y: area.y + v_pad,
                width: area.width,
                height: area.height.saturating_sub(v_pad * 2),
            };
            self.layout_playlist_inner = inner;

            if self.player_tab.items.is_empty() {
                f.render_widget(
                    Paragraph::new("Add items with p from Home or library tabs")
                        .style(Style::default().fg(palette::MUTED)),
                    inner,
                );
                return;
            }

            self.render_playlist_cards(f, inner);
            return;
        }

        if self.playlist_view == 2 {
            self.layout_playlist_inner = area;

            if self.player_tab.items.is_empty() {
                f.render_widget(
                    Paragraph::new("Add items with p from Home or library tabs")
                        .style(Style::default().fg(palette::MUTED)),
                    area,
                );
                return;
            }

            self.render_playlist_presentation(f, area);
            return;
        }

        let inner = area;
        self.layout_playlist_inner = inner;

        if self.player_tab.items.is_empty() {
            f.render_widget(
                Paragraph::new("Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        let cursor = self.player_tab.playlist_cursor;

        let table_area = inner;

        let show_ep_cols = self.player_tab.items.iter().any(|it| it.item_type == "Episode");

        // Fixed column widths + 5 inter-column gaps of 2 = 10 overhead
        let title_col_width = (table_area.width as i32
            - if show_ep_cols { 37 } else { 29 }).max(0) as usize;

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let row_style = if i == current_idx && active {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::WHITE)
            };

            let indicator = if i == cursor {
                Cell::from("▌").style(Style::default().fg(palette::IRIS))
            } else {
                Cell::from(" ")
            };

            let title = item.playback_label();
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
            let media_type_str = if !item.item_type.is_empty() { item.item_type.clone() } else { "—".to_string() };
            let (pos_ticks, rt_ticks) = if i == current_idx && active {
                (live_pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            let title_cell = if pos_ticks > 0 && rt_ticks > 0 && !item.is_audio() {
                let pct = (pos_ticks * 100 / rt_ticks.max(1)) as u64;
                let pct_str = format!(" {pct}%");
                let max_title = title_col_width.saturating_sub(pct_str.chars().count());
                Cell::from(Line::from(vec![
                    Span::raw(trunc_str(&title, max_title)),
                    Span::styled(pct_str, Style::default().fg(palette::YELLOW)),
                ]))
            } else {
                Cell::from(trunc_str(&title, title_col_width))
            };

            if show_ep_cols {
                let ep_tag = if item.item_type == "Episode" && item.parent_index_number > 0 {
                    format!("S{:02}/E{:02}", item.parent_index_number, item.index_number)
                } else { String::new() };
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(Line::from(ep_tag).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(Line::from(media_type_str).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(""),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(Line::from(media_type_str).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            }
        }).collect();

        let header_style = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let header = Row::new([
            Cell::from(""),
            Cell::from("Title").style(header_style),
            Cell::from(""),
            Cell::from(Line::from("Length").alignment(Alignment::Right)).style(header_style),
            Cell::from(Line::from("Type").alignment(Alignment::Right)).style(header_style),
            Cell::from(""),
        ]);

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(if show_ep_cols { 8 } else { 0 }),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .header(header)
        .column_spacing(2)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
    }

    fn fetch_card_image(&mut self, cache_key: String, item_id: String, series_id: String, types: &[&str]) {
        if self.card_image_loading.contains(&cache_key) || self.card_image_states.contains_key(&cache_key) {
            return;
        }
        self.card_image_loading.insert(cache_key.clone());
        let (server_url, token) = {
            let c = self.client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let types_owned: Vec<String> = types.iter().map(|s| s.to_string()).collect();
        let tx = self.card_image_tx.clone();
        let log = self.log.clone();
        std::thread::spawn(move || {
            let fetch_url = |url: &str| -> Option<Vec<u8>> {
                ureq::get(url).call().ok().and_then(|r| {
                    let mut buf = Vec::new();
                    r.into_reader().read_to_end(&mut buf).ok()?;
                    Some(buf)
                })
            };
            let bytes = types_owned.iter().find_map(|t| {
                // "AudioChild": get the first Audio child of item_id, then fetch its Primary.
                if t == "AudioChild" {
                    let child_url = format!("{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=1&api_key={}",
                        server_url, item_id, token);
                    let child_id: Option<String> = fetch_url(&child_url)
                        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                        .and_then(|v| v["Items"].get(0).and_then(|i| i["Id"].as_str().map(|s| s.to_string())));
                    let child_id = child_id?;
                    let url = format!("{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}",
                        server_url, child_id, token);
                    return fetch_url(&url);
                }
                // Logo and Backdrop images live on the series item, not the episode.
                let src = match t.as_str() {
                    "Logo" | "Backdrop" if !series_id.is_empty() => &series_id,
                    _ => &item_id,
                };
                let url = match t.as_str() {
                    "Backdrop" => format!("{}/Items/{}/Images/Backdrop/0?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                    "Logo"     => format!("{}/Items/{}/Images/Logo?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                    _          => format!("{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                };
                fetch_url(&url)
            });
            let bytes = bytes.map(|b| {
                match magick_resize(&b) {
                    Some(resized) => resized,
                    None => {
                        log.push(crate::applog::Level::Warn, "img", format!("magick_resize failed for {cache_key}, using raw bytes"));
                        b
                    }
                }
            });
            let _ = tx.send((cache_key, bytes));
        });
    }

    fn images_enabled(&self) -> bool {
        self.client.lock().unwrap().config.image_protocol.is_some()
    }

    fn evict_card_images(&mut self) {
        let mut valid: std::collections::HashSet<String> = self.player_tab.items.iter()
            .flat_map(|item| [format!("{}:A", item.id), format!("{}:S", item.id)])
            .collect();
        for lib in &self.libs {
            if let Some(lvl) = lib.nav_stack.last() {
                if let Some(item) = lvl.items.get(lvl.cursor) {
                    valid.insert(format!("{}:lib", item.id));
                }
            }
        }
        self.card_image_states.retain(|k, _| valid.contains(k));
        self.card_image_loading.retain(|k| valid.contains(k));
    }

    fn render_playlist_cards(&mut self, f: &mut ratatui::Frame, area: Rect) {
        self.layout_carousel_left_arrow = None;
        self.layout_carousel_right_arrow = None;
        let n = self.player_tab.items.len();
        if n == 0 { return; }
        let expected_max = n * 2;
        if self.card_image_states.len() > expected_max + 10 {
            self.evict_card_images();
        }

        let cursor = self.player_tab.playlist_cursor;
        let (active, active_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };

        let cards_h = area.height;

        // Panels are 80% of available height, centered vertically, capped 6 rows below area.
        // Side cards are 80% of center height, also centered.
        let compact    = self.terminal_height < 28;
        let max_h      = if cards_h < 12 { cards_h } else { ((cards_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let side_h     = ((max_h as u32 * 4 / 5) as u16).max(3);
        let center_h   = if compact { side_h } else { side_h + 2 };
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        let side_v_pad = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        // Below this width threshold, hide side cards and give all space to center.
        const SIDE_HIDE_W: u16 = 60;
        let show_sides = area.width >= SIDE_HIDE_W;

        // Width split: gap | side 30% | gap | center 40% | gap | side 30% | gap
        // Four equal gaps consumed from total width before distributing to panels.
        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w  = area.width.saturating_sub(GAP * 4 + 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = area.x + GAP + 2;
            let xc = xl + sw + GAP;
            let xr = xc + cw + GAP;
            (cw, sw, xl, xc, xr)
        } else {
            let avail_w = area.width.saturating_sub(GAP * 2);
            (avail_w, 0, area.x, area.x + GAP, area.x)
        };

        // is_center: true for the selected middle slot
        let slots: [(Option<usize>, Rect, bool); 3] = [
            (
                if show_sides && cursor > 0 { Some(cursor - 1) } else { None },
                Rect { x: x_left + 2, y: area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
            (
                Some(cursor),
                Rect { x: x_center, y: area.y + center_v_pad, width: center_w, height: center_h },
                true,
            ),
            (
                if show_sides && cursor + 1 < n { Some(cursor + 1) } else { None },
                Rect { x: x_right + 1, y: area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
        ];

        self.layout_carousel_slots = [
            (slots[0].0, slots[0].1),
            (slots[1].0, slots[1].1),
            (slots[2].0, slots[2].1),
        ];

        for (maybe_idx, card_rect, is_center) in &slots {
            let i = match maybe_idx { None => continue, Some(i) => *i };
            if card_rect.width < 3 { continue; }

            let (item_id, series_id, name, series, season, episode, runtime, is_ep,
                 pos_ticks, rt_ticks, played) = {
                let item = &self.player_tab.items[i];
                let is_ep = item.item_type == "Episode" && item.parent_index_number > 0;
                let (pos, rt) = if active && active_idx == i {
                    let s = self.player.status.lock().unwrap();
                    (s.position_ticks, s.runtime_ticks)
                } else {
                    (item.playback_position_ticks, item.runtime_ticks)
                };
                (item.id.clone(), item.series_id.clone(), item.name.clone(), item.series_name.clone(),
                 item.parent_index_number, item.index_number, item.runtime_ticks,
                 is_ep, pos, rt, item.played)
            };

            let selected    = i == cursor;
            let now_playing = active && active_idx == i;
            let _in_prog    = pos_ticks > 0 && rt_ticks > 0 && !played;

            // Kick off image fetch with per-slot priority order.
            let (cache_key, img_types): (String, &[&str]) = if *is_center {
                (format!("{}:A", item_id), &["Primary", "Backdrop", "Logo"])
            } else {
                (format!("{}:S", item_id), &["Logo", "Primary", "Backdrop"])
            };
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }

            let ep_tag = if is_ep { format!("S{:02}E{:02}", season, episode) } else { String::new() };
            let count_label = if *is_center { Some(format!("{}/{}", cursor + 1, n)) } else { None };
            self.render_card_slot(f, *card_rect, *is_center, selected, now_playing, false, false,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played,
                count_label.as_deref(), None);
        }

        // Prefetch images for the three items before and after the cursor.
        if self.images_enabled() {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                let (item_id, series_id) = {
                    let item = &self.player_tab.items[pi];
                    (item.id.clone(), item.series_id.clone())
                };
                self.fetch_card_image(format!("{}:A", item_id.clone()), item_id.clone(), series_id.clone(), &["Primary", "Backdrop", "Logo"]);
                if pi != cursor {
                    self.fetch_card_image(format!("{}:S", item_id), item_id, series_id, &["Logo", "Primary", "Backdrop"]);
                }
            }
        }

        let lr_arrow_style = Style::default().fg(palette::WHITE);
        let y_mid = area.y + center_v_pad + center_h / 2;
        if show_sides && cursor > 0 {
            let r = Rect { x: x_left, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_left_arrow = Some(r);
            f.render_widget(Paragraph::new("◀").style(lr_arrow_style), r);
        }
        if show_sides && cursor + 1 < n {
            let r = Rect { x: x_right + side_w - 1, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_right_arrow = Some(r);
            f.render_widget(Paragraph::new("▶").style(lr_arrow_style), r);
        }
    }

    fn render_playlist_presentation(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let n = self.player_tab.items.len();
        if n == 0 { return; }

        let (active, active_idx, live_pos, live_runtime) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx, s.position_ticks, s.runtime_ticks)
        };

        let cursor = self.player_tab.playlist_cursor;

        // Horizontal split: left card panel | right list (1-row top padding)
        let top_pad: u16 = 1;
        let left_w = ((area.width as u32 * 2 / 5) as u16).clamp(20, 60);
        let right_x = area.x + left_w + 1;
        let right_w = area.width.saturating_sub(left_w + 1);
        let inner_y = area.y + top_pad;
        let inner_h = area.height.saturating_sub(top_pad);
        let left_area  = Rect { x: area.x, y: inner_y, width: left_w, height: inner_h };
        let right_area = Rect { x: right_x, y: inner_y, width: right_w, height: inner_h };

        // Left panel: borderless card for cursor item
        {
            let item = &self.player_tab.items[cursor];
            let item_id   = item.id.clone();
            let series_id = item.series_id.clone();
            let name      = item.name.clone();
            let series    = item.series_name.clone();
            let is_ep     = item.item_type == "Episode" && item.parent_index_number > 0;
            let ep_tag    = if is_ep { format!("S{:02}E{:02}", item.parent_index_number, item.index_number) } else { String::new() };
            let runtime   = item.runtime_ticks;
            let now_playing = active && active_idx == cursor;
            let (pos_ticks, rt_ticks) = if now_playing {
                (live_pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            let played = item.played;
            let img_types: &[&str] = match item.item_type.as_str() {
                "MusicAlbum" => &["AudioChild"],
                "Audio"      => &["Primary"],
                "Movie"      => &["Backdrop", "Primary", "Logo"],
                _            => &["Primary", "Backdrop", "Logo"],
            };
            let cache_key = format!("{}:A", item_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }
            self.render_card_slot(f, left_area, true, true, now_playing, true, true,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played,
                None, None);
        }

        // Prefetch images for 3 items before and after the cursor.
        if self.images_enabled() {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                if pi == cursor { continue; } // already fetched above
                let item = &self.player_tab.items[pi];
                let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
                let img_types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    "Movie"      => &["Backdrop", "Primary", "Logo"],
                    _            => &["Primary", "Backdrop", "Logo"],
                };
                self.fetch_card_image(format!("{}:A", item_id), item_id, series_id, img_types);
            }
        }

        // Right panel: simple 2-column table (title + length)
        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let row_style = if i == active_idx && active {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::WHITE)
            };
            let title = item.playback_label();
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
            let title_cell = if i == cursor {
                Cell::from(Line::from(vec![
                    Span::styled("▌", Style::default().fg(palette::IRIS)),
                    Span::raw(title),
                ]))
            } else {
                Cell::from(Line::from(vec![
                    Span::raw(" "),
                    Span::raw(title),
                ]))
            };
            Row::new([
                title_cell,
                Cell::from(Line::from(length).alignment(Alignment::Right)),
                Cell::from(""),
            ]).style(row_style)
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .column_spacing(2)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, right_area, &mut state);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_card_slot(
        &mut self,
        f: &mut ratatui::Frame,
        card_rect: Rect,
        is_center: bool,
        selected: bool,
        now_playing: bool,
        no_border: bool,
        text_top_aligned: bool,
        cache_key: &str,
        name: &str,
        series: &str,
        ep_tag: &str,
        runtime: i64,
        pos_ticks: i64,
        rt_ticks: i64,
        played: bool,
        count_label: Option<&str>,
        section_title: Option<&str>,
    ) {
        let inner = if no_border {
            card_rect
        } else {
            let border_fg = if selected { palette::IRIS } else { palette::WHITE };
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_fg));
            if let Some(title) = section_title {
                block = block
                    .title(Span::styled(format!(" {} ", title), Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)))
                    .title_alignment(Alignment::Center);
            } else if now_playing {
                block = block
                    .title(Span::styled(" Now Playing ", Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)))
                    .title_alignment(Alignment::Center);
            }
            if let Some(label) = count_label {
                block = block.title_bottom(
                    Line::from(Span::styled(format!(" {} ", label), Style::default().fg(palette::MUTED)))
                        .centered()
                );
            }
            let inner = block.inner(card_rect);
            f.render_widget(block, card_rect);
            inner
        };

        if inner.height < 2 || inner.width == 0 { return; }

        let trunc = |s: &str| -> String {
            let w = inner.width as usize;
            if s.chars().count() > w {
                format!("{}…", &s[..s.char_indices()
                    .nth(w.saturating_sub(1))
                    .map(|(b, _)| b)
                    .unwrap_or(s.len())])
            } else { s.to_string() }
        };

        let put = |f: &mut ratatui::Frame, y: u16, para: Paragraph| {
            if y < inner.bottom() {
                f.render_widget(para, Rect { x: inner.x, y, width: inner.width, height: 1 });
            }
        };

        let fmt_m = |t: i64| -> String {
            let s = t / TICKS_PER_SECOND;
            if s >= 3600 { format!("{}h{:02}m", s/3600, (s%3600)/60) }
            else         { format!("{}m", s/60) }
        };
        let fmt_ms = |t: i64| -> String {
            let s = t / TICKS_PER_SECOND;
            if s >= 3600 { format!("{}:{:02}:{:02}", s/3600, (s%3600)/60, s%60) }
            else         { format!("{}:{:02}", s/60, s%60) }
        };

        // Text rows pinned to the bottom, scaled to available height:
        //   >=8 rows inner: title(2) + series(1) + progress(2) = 5
        //   >=5 rows inner: title(2) + series(1) = 3  (drop progress bar)
        //   <5  rows inner: title(1) only = 1
        let text_rows = if inner.height >= 8 { 5u16 }
                        else if inner.height >= 5 { 3 }
                        else { 1 };
        let img_top    = inner.y;
        let img_bottom = inner.bottom().saturating_sub(text_rows);
        let img_h      = img_bottom.saturating_sub(img_top);

        // Render image if available and there is space.
        let mut actual_img_h: u16 = 0;
        if img_h >= 2 {
            if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                if is_center {
                    let avail = ratatui::layout::Size { width: inner.width.saturating_sub(2), height: img_h };
                    let actual = state.size_for(ratatui_image::Resize::Scale(None), avail);
                    let img_x = inner.x + 1 + (avail.width.saturating_sub(actual.width)) / 2;
                    let img_rect = Rect { x: img_x, y: img_top, width: actual.width, height: actual.height };
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(None)),
                        img_rect, state,
                    );
                    actual_img_h = actual.height;
                } else {
                    let w     = (inner.width as u32 * 36 / 100) as u16;
                    let avail = ratatui::layout::Size { width: w, height: img_h };
                    let actual = state.size_for(ratatui_image::Resize::Fit(None), avail);
                    let img_x = inner.x + (inner.width.saturating_sub(actual.width)) / 2;
                    let img_y = img_top + (img_h.saturating_sub(actual.height)) / 2;
                    let img_rect = Rect { x: img_x, y: img_y, width: actual.width, height: actual.height };
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Fit(None)),
                        img_rect, state,
                    );
                    actual_img_h = actual.height;
                }
            }
        }
        let mut text_y = if text_top_aligned {
            img_top + actual_img_h
        } else {
            img_bottom
        };

        // Title line: "Bold Title (dim 43m)"
        {
            let title_fg  = if selected { palette::WHITE } else { palette::TEXT };
            let title_mod = if selected { Modifier::BOLD } else { Modifier::empty() };
            let dur_suffix = if runtime > 0 { format!(" ({})", fmt_m(runtime)) } else { String::new() };
            let w           = inner.width as usize;
            let name_chars: Vec<char> = name.chars().collect();
            let name_len    = name_chars.len();
            let suffix_len  = dur_suffix.chars().count();
            if name_len + suffix_len <= w {
                let mut spans = vec![Span::styled(name.to_string(), Style::default().fg(title_fg).add_modifier(title_mod))];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(dur_suffix, Style::default().fg(palette::MUTED)));
                }
                put(f, text_y, Paragraph::new(Line::from(spans)).alignment(Alignment::Center));
                text_y += 1;
            } else {
                let wrapped = wrap(name, w);
                let line1: String = wrapped.first().map(|s| s.to_string()).unwrap_or_default();
                let skip = line1.chars().count();
                let line2: String = name.chars().skip(skip).collect::<String>()
                    .trim_start().chars().take(w).collect();
                put(f, text_y, Paragraph::new(Line::from(
                    Span::styled(line1, Style::default().fg(title_fg).add_modifier(title_mod))
                )).alignment(Alignment::Center));
                text_y += 1;
                let mut spans = vec![Span::styled(line2, Style::default().fg(title_fg).add_modifier(title_mod))];
                if !dur_suffix.is_empty() {
                    spans.push(Span::styled(dur_suffix, Style::default().fg(palette::MUTED)));
                }
                put(f, text_y, Paragraph::new(Line::from(spans)).alignment(Alignment::Center));
                text_y += 1;
            }
        }

        if text_rows >= 3 && (!series.is_empty() || !ep_tag.is_empty()) {
            let line = if !series.is_empty() && !ep_tag.is_empty() {
                Line::from(vec![
                    Span::styled(trunc(series), Style::default().fg(palette::SUBTLE)),
                    Span::styled(" • ",         Style::default().fg(palette::IRIS)),
                    Span::styled(ep_tag.to_string(), Style::default().fg(palette::MUTED)),
                ])
            } else if !series.is_empty() {
                Line::from(Span::styled(trunc(series), Style::default().fg(palette::SUBTLE)))
            } else {
                Line::from(Span::styled(ep_tag.to_string(), Style::default().fg(palette::MUTED)))
            };
            put(f, text_y, Paragraph::new(line).alignment(Alignment::Center));
            text_y += 1;
        }

        if text_rows >= 5 && pos_ticks > 0 && rt_ticks > 0 {
            let full_w = inner.width as usize;
            let bar_w  = (full_w as u32 * 3 / 5) as usize;
            let pad    = (full_w.saturating_sub(bar_w)) / 2;
            let fraction = (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0);
            let filled = ((fraction * bar_w as f64).round() as usize).min(bar_w);
            put(f, text_y, Paragraph::new(Line::from(vec![
                Span::raw(" ".repeat(pad)),
                Span::styled("━".repeat(filled),         Style::default().fg(if now_playing { palette::IRIS } else { palette::FOAM })),
                Span::styled("─".repeat(bar_w - filled), Style::default().fg(if now_playing { palette::IRIS_DIM } else { Color::Rgb(0, 80, 128) })),
            ])));
            text_y += 1;
            if now_playing && text_y < inner.bottom() {
                let time_style = Style::default().fg(palette::MUTED);
                let elapsed_str = fmt_ms(pos_ticks);
                let total_str   = fmt_ms(rt_ticks);
                let elapsed_w   = elapsed_str.chars().count() as u16;
                let total_w     = total_str.chars().count() as u16;
                let bar_x       = inner.x + pad as u16;
                let bar_end_x   = bar_x + bar_w as u16;
                f.render_widget(
                    Paragraph::new(Span::styled(elapsed_str, time_style)),
                    Rect { x: bar_x, y: text_y, width: elapsed_w.min(bar_w as u16), height: 1 },
                );
                let total_x = bar_end_x.saturating_sub(total_w);
                f.render_widget(
                    Paragraph::new(Span::styled(total_str, time_style)),
                    Rect { x: total_x, y: text_y, width: total_w.min(bar_w as u16), height: 1 },
                );
            } else {
                put(f, text_y, Paragraph::new(format!("{} / {}", fmt_m(pos_ticks), fmt_m(rt_ticks)))
                    .style(Style::default().fg(palette::MUTED))
                    .alignment(Alignment::Center));
            }
        } else if text_rows >= 5 && played {
            put(f, text_y, Paragraph::new("Played")
                .style(Style::default().fg(palette::MUTED))
                .alignment(Alignment::Center));
        }
    }

    fn render_home_cards(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let n_sections = 1 + self.home.latest.len();
        if n_sections == 0 { return; }

        // Clamp section index in case data changed.
        if self.home.section >= n_sections { self.home.section = 0; }
        let sec = self.home.section;

        // Get current section's title, items, and cursor.
        let (sec_title, items, cursor) = if sec == 0 {
            (
                "Continue Watching".to_string(),
                self.home.continue_items.clone(),
                self.home.continue_cursor,
            )
        } else {
            let (t, _, items, c) = &self.home.latest[sec - 1];
            (t.clone(), items.clone(), *c)
        };

        let n = items.len();
        if n == 0 {
            f.render_widget(
                Paragraph::new("(empty)").style(Style::default().fg(palette::MUTED)).alignment(Alignment::Center),
                area,
            );
            return;
        }

        let cursor = cursor.min(n - 1);

        let cards_area = area;
        let cards_h    = cards_area.height;

        // Same geometry as render_playlist_cards.
        let compact    = self.terminal_height < 28;
        let max_h      = if cards_h < 12 { cards_h } else { ((cards_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let side_h     = ((max_h as u32 * 4 / 5) as u16).max(3);
        let center_h   = if compact { side_h } else { side_h + 2 };
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        // Row just below the center card — used for ▼ scroll arrow.
        let arrow_gap  = if compact { 0 } else { 1 };
        let gutter_y = (cards_area.y + center_v_pad + center_h + arrow_gap).min(area.bottom().saturating_sub(1));
        let side_v_pad = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        const SIDE_HIDE_W: u16 = 60;
        let show_sides = cards_area.width >= SIDE_HIDE_W;

        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w  = cards_area.width.saturating_sub(GAP * 4 + 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = cards_area.x + GAP + 2;
            let xc = xl + sw + GAP;
            let xr = xc + cw + GAP;
            (cw, sw, xl, xc, xr)
        } else {
            let avail_w = cards_area.width.saturating_sub(GAP * 2);
            (avail_w, 0, cards_area.x, cards_area.x + GAP, cards_area.x)
        };

        let slots: [(Option<usize>, Rect, bool); 3] = [
            (
                if show_sides && cursor > 0 { Some(cursor - 1) } else { None },
                Rect { x: x_left + 2, y: cards_area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
            (
                Some(cursor),
                Rect { x: x_center, y: cards_area.y + center_v_pad, width: center_w, height: center_h },
                true,
            ),
            (
                if show_sides && cursor + 1 < n { Some(cursor + 1) } else { None },
                Rect { x: x_right + 1, y: cards_area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
        ];

        self.layout_carousel_slots = [
            (slots[0].0, slots[0].1),
            (slots[1].0, slots[1].1),
            (slots[2].0, slots[2].1),
        ];

        if self.images_enabled() {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                let item = &items[pi];
                let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
                let types_a: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    "Movie"      => &["Backdrop", "Primary", "Logo"],
                    _            => &["Primary", "Backdrop", "Logo"],
                };
                self.fetch_card_image(format!("{}:A", item_id.clone()), item_id.clone(), series_id.clone(), types_a);
                if pi != cursor {
                    let types_s: &[&str] = match item.item_type.as_str() {
                        "MusicAlbum" => &["AudioChild"],
                        "Audio"      => &["Primary"],
                        _ => &["Logo", "Primary", "Backdrop"],
                    };
                    self.fetch_card_image(format!("{}:S", item_id), item_id, series_id, types_s);
                }
            }
        }

        for (maybe_idx, card_rect, is_center) in &slots {
            let i = match maybe_idx { None => continue, Some(i) => *i };
            if card_rect.width < 3 { continue; }

            let item = &items[i];
            let is_ep = item.item_type == "Episode" && item.parent_index_number > 0;
            let ep_tag = if is_ep { format!("S{:02}E{:02}", item.parent_index_number, item.index_number) } else { String::new() };
            let name       = item.name.clone();
            let series     = item.series_name.clone();
            let runtime    = item.runtime_ticks;
            let pos_ticks  = item.playback_position_ticks;
            let rt_ticks   = item.runtime_ticks;
            let played     = item.played;
            let item_id    = item.id.clone();
            let series_id  = item.series_id.clone();
            let selected   = i == cursor;

            let (cache_key, img_types): (String, &[&str]) = if *is_center {
                let types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    "Movie"      => &["Backdrop", "Primary", "Logo"],
                    _            => &["Primary", "Backdrop", "Logo"],
                };
                (format!("{}:A", item_id), types)
            } else {
                let types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    _ => &["Logo", "Primary", "Backdrop"],
                };
                (format!("{}:S", item_id), types)
            };
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }

            let count_label = if *is_center { Some(format!("{}/{}", cursor + 1, n)) } else { None };
            let sec_title_label = if *is_center { Some(sec_title.as_str()) } else { None };
            self.render_card_slot(f, *card_rect, *is_center, selected, false, false, false,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played,
                count_label.as_deref(), sec_title_label);
        }

        // Left/right item scroll arrows.
        let lr_arrow_style = Style::default().fg(palette::WHITE);
        let y_mid = cards_area.y + center_v_pad + center_h / 2;
        if show_sides && cursor > 0 {
            let r = Rect { x: x_left, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_left_arrow = Some(r);
            f.render_widget(Paragraph::new("◀").style(lr_arrow_style), r);
        }
        if show_sides && cursor + 1 < n {
            let r = Rect { x: x_right + side_w - 1, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_right_arrow = Some(r);
            f.render_widget(Paragraph::new("▶").style(lr_arrow_style), r);
        }

        // Section scroll arrows.
        let n_sections = 1 + self.home.latest.len();
        let ud_arrow_style = Style::default().fg(palette::IRIS);
        let up_arrow_offset = 1 + arrow_gap;
        if self.home.section > 0 && center_v_pad >= up_arrow_offset {
            let r = Rect { x: area.x, y: cards_area.y + center_v_pad - up_arrow_offset, width: area.width, height: 1 };
            self.layout_carousel_up_arrow = Some(r);
            f.render_widget(Paragraph::new("▲").style(ud_arrow_style).alignment(Alignment::Center), r);
        }
        if self.home.section + 1 < n_sections && gutter_y < area.bottom() {
            let r = Rect { x: area.x, y: gutter_y, width: area.width, height: 1 };
            self.layout_carousel_down_arrow = Some(r);
            f.render_widget(Paragraph::new("▼").style(ud_arrow_style).alignment(Alignment::Center), r);
        }
    }

    fn render_home_panel(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let home_focused = true;
        let n_latest = self.home.latest.len();
        let n_sections = 1 + n_latest;
        let n_rows = 1 + (n_latest + 1) / 2;

        let visible_rows = if (n_rows as u16) * HOME_MIN_SECTION_H <= area.height {
            n_rows
        } else {
            ((area.height / HOME_MIN_SECTION_H) as usize).max(1)
        };

        let max_offset = n_rows.saturating_sub(visible_rows);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
        let row_offset = self.home_panel_section_offset;
        let render_row_count = visible_rows.min(n_rows - row_offset);

        let scrollable = n_rows > visible_rows;
        let layout_area = if scrollable && area.width > 2 {
            Rect { width: area.width - 2, ..area }
        } else {
            area
        };

        let constraints: Vec<Constraint> = (0..render_row_count)
            .map(|_| Constraint::Ratio(1, render_row_count as u32))
            .collect();
        let row_areas = Layout::vertical(constraints).split(layout_area);

        // layout_section_areas indexed by logical section; off-screen sections get Rect::default()
        let mut areas: Vec<Rect> = vec![Rect::default(); n_sections];

        // Collect latest data before mutable renders
        let latest_data: Vec<(String, Vec<MediaItem>, usize)> = self.home.latest
            .iter()
            .map(|(t, _, items, c)| (t.clone(), items.clone(), *c))
            .collect();

        let mut scrolls = vec![0usize; n_sections];

        for row_pos in 0..render_row_count {
            let logical_row = row_offset + row_pos;
            let row_area = row_areas[row_pos];

            if logical_row == 0 {
                // Continue Watching — full width
                areas[0] = row_area;
                let cont_focused = home_focused && self.home.section == 0;
                scrolls[0] = self.render_home_section(
                    f, row_area, "Continue Watching",
                    &self.home.continue_items, self.home.continue_cursor, cont_focused, true,
                );
            } else {
                // Latest pair — split 50/50
                let latest_row_idx = logical_row - 1;
                let left_sec = 1 + latest_row_idx * 2;
                let right_sec = left_sec + 1;

                let [left_area, right_area] = Layout::horizontal([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]).areas(row_area);

                let is_last_odd = right_sec >= n_sections;
                let render_left_area = if is_last_odd {
                    Layout::horizontal([
                        Constraint::Percentage(25),
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                    ]).areas::<3>(row_area)[1]
                } else {
                    left_area
                };

                if let Some((title, items, cursor)) = latest_data.get(left_sec - 1) {
                    areas[left_sec] = render_left_area;
                    let focused = home_focused && self.home.section == left_sec;
                    scrolls[left_sec] = self.render_home_section(
                        f, render_left_area, title, items, *cursor, focused, false,
                    );
                }
                if right_sec < n_sections {
                    if let Some((title, items, cursor)) = latest_data.get(right_sec - 1) {
                        areas[right_sec] = right_area;
                        let focused = home_focused && self.home.section == right_sec;
                        scrolls[right_sec] = self.render_home_section(
                            f, right_area, title, items, *cursor, focused, false,
                        );
                    }
                }
            }
        }

        self.layout_section_areas = areas;
        self.layout_home_scrolls = scrolls;

        if scrollable {
            let sb_rect = Rect { x: area.x + area.width.saturating_sub(1), y: area.y, width: 1, height: area.height };
            self.layout_home_scrollbar = sb_rect;
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(row_offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None),
                area,
                &mut sb_state,
            );
        } else {
            self.layout_home_scrollbar = Rect::default();
        }
    }

    fn render_home_section(
        &self, f: &mut ratatui::Frame, area: Rect,
        title: &str, items: &[MediaItem], cursor: usize, focused: bool,
        continue_style: bool,
    ) -> usize {
        let border_style = if focused { Style::default().fg(palette::IRIS) } else { Style::default().fg(palette::PINE) };
        let title_style = if focused {
            Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)
        };
        let block = Block::default()
            .borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(Span::styled(format!(" {} ", title), title_style))
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if items.is_empty() {
            f.render_widget(Paragraph::new("(empty)").style(Style::default().fg(palette::MUTED)), inner);
            return 0;
        }

        let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, item)| {
            let sel = focused && i == cursor;
            let li = if continue_style {
                ListItem::new(fmt_item_continue(item, inner.width as usize, sel))
            } else {
                ListItem::new(fmt_item_wrapped(item, inner.width as usize, sel))
            };
            if sel {
                li.style(if continue_style { highlight_style_continue(item) } else { highlight_style(item) })
            } else {
                li.style(Style::default().fg(palette::MUTED))
            }
        }).collect();

        let mut state = ListState::default();
        if focused { state.select(Some(cursor)); }
        f.render_stateful_widget(List::new(list_items), inner, &mut state);
        state.offset()
    }

    fn render_library(&mut self, f: &mut ratatui::Frame, area: Rect, lib_idx: usize) {
        let is_loading = self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(true);
        if is_loading && self.libs[lib_idx].search.is_none() {
            let block = Block::default()
                .borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS));
            let inner = block.inner(area);
            f.render_widget(block, area);
            let mid = inner.y + inner.height / 2;
            let label_area = Rect { y: mid, height: 1, ..inner };
            f.render_widget(
                Paragraph::new("Loading...")
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(Style::default().fg(palette::MUTED)),
                label_area,
            );
            return;
        }

        // Build breadcrumb spans and record click regions
        let lib = &self.libs[lib_idx];
        let skip = if lib.nav_stack.first().map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
        // crumb 0 = library name (target depth 1), then each nav_stack part above skip
        let mut crumb_names: Vec<(String, usize)> = vec![(lib.library.name.clone(), 1)];
        for (i, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
            crumb_names.push((lvl.title.clone(), i + 1));
        }

        let sep = " \u{bb} ";
        let is_deep = crumb_names.len() > 1;

        // Title sits on the top border row; x starts after left corner + 1 space pad.
        let crumb_row = area.y;
        let mut x = area.x + 2; // border char + leading space

        let crumb_style = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let mut crumb_spans: Vec<Span<'static>> = Vec::new();
        let mut new_breadcrumbs: Vec<(u16, u16, u16, usize)> = Vec::new();
        for (ci, (name, target_depth)) in crumb_names.iter().enumerate() {
            let is_last = ci + 1 == crumb_names.len();
            let w = name.chars().count() as u16;
            new_breadcrumbs.push((x, x + w, crumb_row, *target_depth));
            crumb_spans.push(Span::styled(name.clone(), crumb_style));
            x += w;
            if !is_last {
                crumb_spans.push(Span::styled(sep, crumb_style));
                x += sep.len() as u16;
            }
        }
        self.layout_breadcrumbs = if is_deep { new_breadcrumbs } else { Vec::new() };

        let mut block = Block::default()
            .borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS));
        if is_deep {
            crumb_spans.insert(0, Span::raw(" "));
            crumb_spans.push(Span::raw(" "));
            block = block.title(Line::from(crumb_spans));
        }
        let inner = block.inner(area);
        f.render_widget(block, area);
        self.render_library_table(f, inner, lib_idx);
    }

    fn render_library_table(&mut self, f: &mut ratatui::Frame, area: Rect, lib_idx: usize) {
        self.layout_lib_table_area = area;
        const LIB_SELECTED_IMG_W: u16 = 32;
        const LIB_AUDIO_IMG_W: u16 = 12;
        // Height for a square image at LIB_AUDIO_IMG_W columns, derived from actual cell pixel ratio.
        let lib_audio_img_h: u16 = self.image_picker.as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_AUDIO_IMG_W as f32 * fs.width as f32) / fs.height as f32).ceil() as u16
            })
            .unwrap_or(12);
        #[allow(non_snake_case)]
        let LIB_AUDIO_IMG_H = lib_audio_img_h;

        let (display_items, cursor): (Vec<(usize, crate::api::MediaItem)>, usize) = {
            let lib = &self.libs[lib_idx];
            if let Some(s) = &lib.search {
                let items: Vec<(usize, crate::api::MediaItem)> = s.results.iter()
                    .filter_map(|&i| s.items.get(i).map(|item| (i, item.clone())))
                    .collect();
                (items, s.cursor)
            } else {
                let lvl = lib.nav_stack.last();
                let items: Vec<(usize, crate::api::MediaItem)> = lvl.map(|l| {
                    l.items.iter().enumerate().map(|(i, item)| (i, item.clone())).collect()
                }).unwrap_or_default();
                let cur = lvl.map(|l| l.cursor).unwrap_or(0);
                (items, cur)
            }
        };

        let items_len = display_items.len();
        if items_len == 0 {
            let loading = self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(false);
            let msg = if loading { "  Loading..." }
                else if self.libs[lib_idx].search.is_some() { "  (no results)" }
                else { "  (empty)" };
            f.render_widget(Paragraph::new(msg).style(Style::default().fg(palette::MUTED)), area);
            return;
        }

        // Row heights: 2 base + 1 separator. Selected row also gets overview and seekbar
        // when images are enabled.
        let images_enabled = self.images_enabled();
        let lib_ctype = self.libs[lib_idx].library.collection_type.clone();
        let show_seekbar = !matches!(lib_ctype.as_str(), "channels" | "homevideos");
        let content_w_sel = area.width.saturating_sub(1 + LIB_SELECTED_IMG_W) as usize;
        let all_heights: Vec<u16> = display_items.iter().enumerate().map(|(i, (_, item))| {
            let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
            let base: u16 = if is_audio {
                if i == cursor { LIB_AUDIO_IMG_H.max(3) } else { 3 }
            } else if images_enabled && i == cursor {
                let ew = content_w_sel;
                let ov_lines = if item.overview.is_empty() { 0 }
                    else { wrap(&item.overview, ew.max(1)).len().min(4) as u16 };
                let seekbar: u16 = if show_seekbar && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 { 2 } else { 0 };
                2 + ov_lines + seekbar
            } else { 2 };
            base + 1 // +1 for separator line at bottom
        }).collect();

        // Scroll: keep existing scroll, only adjust to keep cursor visible.
        let scroll = {
            let mut s = self.layout_lib_scroll.min(cursor);
            loop {
                let visible_h: u16 = all_heights[s..=cursor].iter().sum();
                if visible_h <= area.height { break; }
                s += 1;
            }
            s
        };
        self.layout_lib_scroll = scroll;

        // Trigger image fetch for selected item before rendering loop
        {
            if let Some((_, item)) = display_items.get(cursor) {
                let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
                if images_enabled || is_audio {
                    let cache_key = format!("{}:lib", item.id);
                    self.fetch_card_image(cache_key, item.id.clone(), String::new(), &["Primary"]);
                }
            }
        }

        // Render visible rows, accumulating y
        let mut row_y = area.y;
        let mut rendered_heights: Vec<u16> = Vec::new();
        for (vi, (_, item)) in display_items[scroll..].iter().enumerate() {
            if row_y >= area.y + area.height { break; }
            let abs_idx = scroll + vi;
            let row_h = all_heights[abs_idx].min(area.y + area.height - row_y);
            let selected = abs_idx == cursor;
            let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
            let show_img = selected && (images_enabled || is_audio);
            let row_rect = Rect { x: area.x, y: row_y, width: area.width, height: row_h };

            // Content area excludes the separator line at the bottom of the row.
            let content_area = Rect { height: row_h.saturating_sub(1), ..row_rect };
            let padded_area = content_area;

            // Compute actual image size first so we can size the column correctly.
            let cache_key = format!("{}:lib", item.id);
            let img_actual = if show_img {
                if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                    let (img_w, img_h) = if is_audio {
                        (LIB_AUDIO_IMG_W, LIB_AUDIO_IMG_H)
                    } else {
                        (LIB_SELECTED_IMG_W, padded_area.height)
                    };
                    let avail = ratatui::layout::Size { width: img_w, height: img_h.min(padded_area.height) };
                    Some(state.size_for(ratatui_image::Resize::Fit(None), avail))
                } else { None }
            } else { None };

            // Split padded_area: for audio, image LEFT; for others, image RIGHT
            // Layout: indicator(1) | [image + gap] | text | [gap + image]
            let (ind_rect, text_rect, img_rect_opt) = if is_audio {
                if let Some(actual) = img_actual {
                    let [a, b, _, c] = Layout::horizontal([
                        Constraint::Length(1),
                        Constraint::Length(actual.width),
                        Constraint::Length(1),
                        Constraint::Min(0),
                    ]).areas(padded_area);
                    let img_h = actual.height.min(b.height);
                    let v_off = b.height.saturating_sub(img_h) / 2;
                    let img_rect = Rect { y: b.y + v_off, height: img_h, ..b };
                    (a, c, Some(img_rect))
                } else {
                    let [a, c] = Layout::horizontal([
                        Constraint::Length(1),
                        Constraint::Min(0),
                    ]).areas(padded_area);
                    (a, c, None)
                }
            } else if let Some(actual) = img_actual {
                let [a, b, _, c] = Layout::horizontal([
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(2),
                    Constraint::Length(actual.width),
                ]).areas(padded_area);
                let img_rect = Rect { height: actual.height.min(c.height), ..c };
                (a, b, Some(img_rect))
            } else {
                let [a, c] = Layout::horizontal([
                    Constraint::Length(1),
                    Constraint::Min(0),
                ]).areas(padded_area);
                (a, c, None)
            };
            let content_w = text_rect.width as usize;

            if selected {
                let bar: Vec<Line> = (0..ind_rect.height)
                    .map(|_| Line::from(Span::styled("▌", Style::default().fg(palette::IRIS))))
                    .collect();
                f.render_widget(Paragraph::new(bar), ind_rect);
            }

            // Render image at the right of the row
            if let Some(img_rect) = img_rect_opt {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                    f.render_stateful_widget(SImg::default().resize(ratatui_image::Resize::Fit(None)), img_rect, state);
                }
            }

            let text_color = if selected { palette::WHITE } else { palette::TEXT };

            // Build title line (line 1)
            let title_line = match item.item_type.as_str() {
                "Episode" => {
                    let n = item.index_number;
                    if n > 0 { format!("{}. {}", n, item.name) } else { item.name.clone() }
                }
                "Series" => {
                    if item.total_count > 0 {
                        format!("{} ({}/{})", item.name, item.unplayed_item_count, item.total_count)
                    } else if item.unplayed_item_count > 0 {
                        format!("{} ({})", item.name, item.unplayed_item_count)
                    } else {
                        item.name.clone()
                    }
                }
                _ => item.name.clone(),
            };
            let title_display = wrap(&title_line, content_w.max(1))
                .into_iter().next().map(|c| c.into_owned()).unwrap_or_default();

            // For audio: artist line between title and metadata
            let artist_line: Option<String> = if is_audio && !item.artist.is_empty() {
                Some(item.artist.clone())
            } else { None };

            // Build metadata line (line 2 for non-audio, line 3 for audio)
            let meta_line: Line = match item.item_type.as_str() {
                "Series" => {
                    let year_str = if item.production_year > 0 && item.end_year > 0 && item.end_year != item.production_year {
                        format!("{} \u{2013} {}", item.production_year, item.end_year)
                    } else if item.production_year > 0 && item.end_year == 0 {
                        format!("{} \u{2013}", item.production_year)
                    } else if item.production_year > 0 {
                        format!("{}", item.production_year)
                    } else {
                        String::new()
                    };
                    Line::from(Span::styled(year_str, Style::default().fg(palette::SUBTLE)))
                }
                "Season" => {
                    let mut parts: Vec<String> = Vec::new();
                    if item.total_count > 0 { parts.push(format!("{} eps", item.total_count)); }
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    Line::from(Span::styled(parts.join(" \u{b7} "), Style::default().fg(palette::SUBTLE)))
                }
                "Episode" => {
                    let mut spans: Vec<Span> = Vec::new();
                    if item.played {
                        spans.push(Span::styled("\u{f00c} ", Style::default().fg(palette::PINE)));
                    }
                    let mut parts: Vec<String> = Vec::new();
                    if !item.premiere_date.is_empty() { parts.push(item.premiere_date.clone()); }
                    let dur_s = item.runtime_ticks / crate::api::TICKS_PER_SECOND;
                    if dur_s > 0 {
                        let h = dur_s / 3600; let m = (dur_s % 3600) / 60;
                        parts.push(if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") });
                    }
                    if !parts.is_empty() {
                        spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
                    }
                    Line::from(spans)
                }
                _ if item.is_folder && item.item_type != "Series" && item.item_type != "Season" => {
                    if item.total_count > 0 {
                        Line::from(Span::styled(
                            format!("{} items", item.total_count),
                            Style::default().fg(palette::SUBTLE),
                        ))
                    } else {
                        Line::from(vec![])
                    }
                }
                _ => {
                    // Movie, Audio, and other non-folder types
                    let mut spans: Vec<Span> = Vec::new();
                    if !is_audio && item.played {
                        spans.push(Span::styled("\u{f00c} ", Style::default().fg(palette::PINE)));
                    }
                    let mut parts: Vec<String> = Vec::new();
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    let dur_s = item.runtime_ticks / crate::api::TICKS_PER_SECOND;
                    if dur_s > 0 {
                        let h = dur_s / 3600; let m = (dur_s % 3600) / 60;
                        parts.push(if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") });
                    }
                    if is_audio && !item.container.is_empty() {
                        parts.push(item.container.to_uppercase());
                    }
                    if !parts.is_empty() {
                        spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
                    }
                    if !is_audio && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                        spans.push(Span::styled(format!("  {pct}%"), Style::default().fg(palette::YELLOW)));
                    }
                    Line::from(spans)
                }
            };

            // Prepend item_type as the first span on the metadata line
            let meta_line = {
                let type_str = if !item.item_type.is_empty() { item.item_type.clone() } else { "—".to_string() };
                let mut spans = vec![Span::styled(format!("{}  ", type_str), Style::default().fg(palette::SUBTLE))];
                spans.extend(meta_line.spans);
                Line::from(spans)
            };

            // Split text_rect vertically into lines
            let in_progress = !is_audio && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0;
            let overview_lines: Vec<String> = if !is_audio && selected && images_enabled && !item.overview.is_empty() {
                let w = content_w.max(1);
                let mut lines: Vec<String> = wrap(&item.overview, w).into_iter().map(|s| s.into_owned()).collect();
                if lines.len() > 4 {
                    lines.truncate(4);
                    let last = lines.last_mut().unwrap();
                    if last.len() + 3 <= w { last.push_str("..."); }
                    else { let i = last.char_indices().rev().nth(2).map(|(i, _)| i).unwrap_or(0); last.replace_range(i.., "..."); }
                }
                lines
            } else { Vec::new() };
            let seekbar_extra: usize = if !is_audio && selected && images_enabled && show_seekbar && in_progress { 2 } else { 0 }; // spacer + bar
            let artist_extra: usize = if artist_line.is_some() { 1 } else { 0 };
            let base_lines = 2 + artist_extra;
            let line_count = (base_lines + overview_lines.len() + seekbar_extra).min(text_rect.height as usize);
            if line_count == 0 { continue; }
            let v_offset = if is_audio && selected {
                (text_rect.height as usize).saturating_sub(line_count) / 2
            } else { 0 };
            let centered_text_rect = Rect {
                y: text_rect.y + v_offset as u16,
                height: text_rect.height.saturating_sub(v_offset as u16),
                ..text_rect
            };
            let constraints: Vec<Constraint> = (0..line_count).map(|_| Constraint::Length(1)).collect();
            let line_rects = Layout::vertical(constraints).split(centered_text_rect);

            f.render_widget(
                Paragraph::new(Line::from(Span::styled(title_display, Style::default().fg(text_color)))),
                line_rects[0],
            );
            if let Some(ref a) = artist_line {
                if line_count >= 2 {
                    f.render_widget(
                        Paragraph::new(Span::styled(a.as_str(), Style::default().fg(palette::SUBTLE))),
                        line_rects[1],
                    );
                }
                if line_count >= 3 {
                    f.render_widget(Paragraph::new(meta_line), line_rects[2]);
                }
            } else if line_count >= 2 {
                f.render_widget(Paragraph::new(meta_line), line_rects[1]);
            }
            for (j, ov_line) in overview_lines.iter().enumerate() {
                let idx = base_lines + j;
                if idx >= line_count { break; }
                f.render_widget(
                    Paragraph::new(Span::styled(ov_line.as_str(), Style::default().fg(palette::MUTED))),
                    line_rects[idx],
                );
            }
            // Seekbar: spacer at line_count-2, bar at line_count-1
            if !is_audio && selected && images_enabled && show_seekbar && in_progress && line_count >= base_lines + seekbar_extra {
                let bar_w = content_w;
                let fraction = (item.playback_position_ticks as f64 / item.runtime_ticks as f64).clamp(0.0, 1.0);
                let filled = ((fraction * bar_w as f64).round() as usize).min(bar_w);
                let seekbar_line = Line::from(vec![
                    Span::styled("━".repeat(filled),           Style::default().fg(palette::YELLOW)),
                    Span::styled("─".repeat(bar_w - filled),   Style::default().fg(palette::MUTED)),
                ]);
                f.render_widget(Paragraph::new(seekbar_line), line_rects[line_count - 1]);
            }

            // Separator line at the bottom of each row
            let sep_y = row_y + row_h - 1;
            if sep_y < area.y + area.height {
                let sep_rect = Rect { x: area.x, y: sep_y, width: area.width, height: 1 };
                let sep_str: String = "\u{2500}".repeat(area.width as usize);
                f.render_widget(
                    Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
                    sep_rect,
                );
            }

            rendered_heights.push(row_h);
            row_y += row_h;
        }
        self.layout_lib_row_heights = rendered_heights;
    }

    fn render_log(&self, f: &mut ratatui::Frame, area: Rect) {
        let [hint_area, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
            .areas(area);

        let k = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let m = Style::default().fg(palette::MUTED);
        let sp = "  ";
        let hints = Line::from(vec![
            Span::raw(sp), Span::styled("←→", k), Span::styled(" pane", m),
            Span::raw(sp), Span::styled("↑↓", k), Span::styled(" scroll", m),
            Span::raw(sp), Span::styled("PgUp/Dn", k), Span::styled(" page", m),
            Span::raw(sp), Span::styled("Space", k), Span::styled(" toggle source", m),
            Span::raw(sp), Span::styled("c", k), Span::styled(" copy to clipboard", m),
        ]);
        f.render_widget(Paragraph::new(hints), hint_area);

        let sources = self.log_sources();
        let src_w = (sources.iter().map(|s| s.len()).max().unwrap_or(4) + 4) as u16;

        let [src_area, log_area] = Layout::horizontal([
            Constraint::Length(src_w),
            Constraint::Min(10),
        ]).areas(body);

        // ── Sources pane ──────────────────────────────────────────────────────
        let src_focused = self.log_pane == LogPane::Sources;
        let src_border = if src_focused { palette::IRIS } else { palette::OVERLAY };
        let src_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(src_border))
            .title(Span::styled(" Sources ", Style::default().fg(palette::SUBTLE).add_modifier(Modifier::BOLD)));
        let src_inner = src_block.inner(src_area);
        f.render_widget(src_block, src_area);

        let src_cursor = self.log_source_cursor.min(sources.len().saturating_sub(1));
        for (i, &src) in sources.iter().enumerate() {
            let y = src_inner.top() + i as u16;
            if y >= src_inner.bottom() { break; }
            let disabled = self.log_disabled_sources.contains(src);
            let selected = i == src_cursor && src_focused;
            let fg = if disabled { palette::OVERLAY } else if selected { palette::YELLOW } else { palette::SUBTLE };
            let prefix = if disabled { "○ " } else { "● " };
            let dot_color = if disabled { palette::OVERLAY } else { palette::IRIS };
            f.buffer_mut().set_stringn(src_inner.left(), y, prefix, 2, Style::default().fg(dot_color));
            f.buffer_mut().set_stringn(src_inner.left() + 2, y, src, src_inner.width as usize, Style::default().fg(fg));
        }

        // ── Log pane ──────────────────────────────────────────────────────────
        let log_focused = self.log_pane == LogPane::Log;
        let log_border = if log_focused { palette::IRIS } else { palette::OVERLAY };
        let entries = self.visible_log_entries();
        let n = entries.len();
        let log_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(log_border))
            .title(Span::styled(
                format!(" Log ({n}) "),
                Style::default().fg(palette::SUBTLE).add_modifier(Modifier::BOLD),
            ));
        let log_inner = log_block.inner(log_area);
        f.render_widget(log_block, log_area);

        let visible = log_inner.height as usize;
        let max_scroll = n.saturating_sub(visible);
        let scroll = self.log_scroll.min(max_scroll);
        let first = max_scroll.saturating_sub(scroll);

        for (row, entry) in entries.iter().skip(first).take(visible).enumerate() {
            let y = log_inner.top() + row as u16;
            let (level_color, label) = match entry.level {
                Level::Error => (Color::Red,        "E"),
                Level::Warn  => (Color::Yellow,     "W"),
                Level::Info  => (palette::WHITE,    "I"),
                Level::Debug => (palette::SUBTLE,   "D"),
            };
            let w = log_inner.width as usize;
            let mut x = log_inner.left();
            // level letter
            f.buffer_mut().set_stringn(x, y, label, 1, Style::default().fg(level_color));
            x += 1;
            // separator
            f.buffer_mut().set_stringn(x, y, "│", 1, Style::default().fg(palette::OVERLAY));
            x += 1;
            // source
            let src_len = entry.source.len().min(6);
            f.buffer_mut().set_stringn(x, y, entry.source, src_len, Style::default().fg(palette::MUTED));
            x += 6 + 1;
            if x >= log_inner.right() { continue; }
            f.buffer_mut().set_stringn(x, y, "│", 1, Style::default().fg(palette::OVERLAY));
            x += 1;
            // message
            if x < log_inner.right() {
                let remaining = (log_inner.right() - x) as usize;
                let msg_w = remaining.min(w);
                f.buffer_mut().set_stringn(x, y, &entry.msg, msg_w, Style::default().fg(level_color));
            }
        }
    }
}

fn natural_sort_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = c.to_string();
            while chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                num.push(chars.next().unwrap());
            }
            out.push_str(&format!("{:0>8}", num));
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

fn is_playable(item: &crate::api::MediaItem) -> bool {
    matches!(item.media_type.as_str(), "Video" | "Audio")
}


fn fmt_duration(s: i64) -> String {
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60) }
    else         { format!("{}:{:02}", s / 60, s % 60) }
}

fn trunc_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { format!("{}\u{2026}", s.chars().take(max.saturating_sub(1)).collect::<String>()) }
}

fn item_text_and_style(item: &MediaItem, selected: bool) -> (String, Style) {
    if item.is_folder {
        let text = if item.unplayed_item_count > 0 {
            format!("{} [{}]", item.display_name(), item.unplayed_item_count)
        } else {
            item.display_name()
        };
        let style = if selected { Style::default() }
            else               { Style::default().fg(palette::WHITE) };
        return (text, style);
    }
    let mut suffix = String::new();
    if item.runtime_ticks > 0 {
        let s = item.runtime_seconds();
        let h = (s / 3600.0) as u64;
        let m = ((s % 3600.0) / 60.0) as u64;
        let dur = if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") };
        suffix = format!(" ({dur})");
    }
    let text = format!("{}{}", item.display_name(), suffix);
    let style = if selected { Style::default() }
        else { Style::default().fg(palette::WHITE) };
    (text, style)
}

fn split_suffix(s: &str) -> (&str, &str) {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") { return (&s[..pos], &s[pos..]); }
    }
    if s.ends_with(']') {
        if let Some(pos) = s.rfind(" [") { return (&s[..pos], &s[pos..]); }
    }
    (s, "")
}

fn fmt_item_wrapped(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, style) = item_text_and_style(item, selected);
    let in_progress = !item.is_folder && item.playback_position_ticks > 0;
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);
    let w = width.max(1);
    let lines: Vec<Line<'static>> = wrap(&full_text, w).into_iter().enumerate()
        .map(|(i, s)| {
            let s = s.into_owned();
            if i == 0 && in_progress {
                let pct_str = if item.runtime_ticks > 0 {
                    let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    format!(" {pct}%")
                } else { String::new() };
                let (name, suf) = split_suffix(&s);
                let mut spans = vec![Span::styled(name.to_string(), style)];
                if !suf.is_empty() { spans.push(Span::styled(suf.to_string(), subtle)); }
                if !pct_str.is_empty() { spans.push(Span::styled(pct_str, yellow)); }
                Line::from(spans)
            } else {
                let (name, suf) = split_suffix(&s);
                if suf.is_empty() {
                    Line::from(Span::styled(s, style))
                } else {
                    Line::from(vec![Span::styled(name.to_string(), style), Span::styled(suf.to_string(), subtle)])
                }
            }
        })
        .collect();
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

fn highlight_style(item: &MediaItem) -> Style {
    if item.is_folder && item.item_type != "Series" && item.item_type != "MusicAlbum" && item.item_type != "MusicArtist" {
        Style::default().fg(palette::BASE).bg(palette::PINE)
    } else if item.playback_position_ticks > 0 {
        Style::default().fg(palette::BASE).bg(palette::YELLOW)
    } else {
        Style::default().fg(palette::WHITE).bg(palette::FOCUSED)
    }
}

fn fmt_item_continue(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, _) = item_text_and_style(item, selected);
    let in_progress = item.playback_position_ticks > 0;
    let span_style = if selected { Style::default() } else { Style::default().fg(palette::WHITE) };
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);
    let w = width.max(1);
    let lines: Vec<Line<'static>> = wrap(&full_text, w).into_iter().enumerate()
        .map(|(i, s)| {
            let s = s.into_owned();
            if i == 0 && in_progress {
                let pct_str = if item.runtime_ticks > 0 {
                    let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    format!(" {pct}%")
                } else { String::new() };
                let (name, suf) = split_suffix(&s);
                let mut spans = vec![Span::styled(name.to_string(), span_style)];
                if !suf.is_empty() { spans.push(Span::styled(suf.to_string(), subtle)); }
                if !pct_str.is_empty() { spans.push(Span::styled(pct_str, yellow)); }
                Line::from(spans)
            } else {
                let (name, suf) = split_suffix(&s);
                if suf.is_empty() {
                    Line::from(Span::styled(s, span_style))
                } else {
                    Line::from(vec![Span::styled(name.to_string(), span_style), Span::styled(suf.to_string(), subtle)])
                }
            }
        })
        .collect();
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

fn highlight_style_continue(_item: &MediaItem) -> Style {
    Style::default().bg(palette::FOCUSED)
}





#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::TICKS_PER_SECOND;

    fn make_item(name: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: "id".into(), name: name.into(), item_type: item_type.into(),
            is_folder: false, media_type: "Video".into(), collection_type: String::new(),
            runtime_ticks: 0, played: false, playback_position_ticks: 0,
            series_id: String::new(), series_name: String::new(), album_id: String::new(),
            index_number: 0, parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
            production_year: 0, end_year: 0, overview: String::new(),
            premiere_date: String::new(), total_count: 0, container: String::new(),
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
        assert!(!text.contains("50%"), "pct should be in span, not text: {text}");
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
        assert!(!text.contains("50%"), "pct should be in span, not text: {text}");
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

    fn make_items(n: usize) -> Vec<MediaItem> {
        (0..n).map(|i| {
            let mut item = make_item(&format!("Item {i}"), "Movie");
            item.id = format!("id{i}");
            item
        }).collect()
    }

    /// Minimal App stub for logic-only tests.
    fn make_app_stub() -> App {
        use std::sync::{Arc, Mutex};
        use crate::player::{PlayerProxy, PlayerStatus};

        let status = Arc::new(Mutex::new(PlayerStatus {
            position_ticks: 0, runtime_ticks: 0, paused: false,
            volume: 100, volume_max: 100, current_idx: 0, active: false,
            title: String::new(), audio_tracks: Vec::new(), sub_tracks: Vec::new(),
            audio_id: 0, sub_id: 0,
        }));

        let (_, player_rx) = std::sync::mpsc::channel();
        let (_, ws_rx)     = std::sync::mpsc::channel();
        let (lib_tx, lib_rx) = std::sync::mpsc::channel();
        let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
        let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();

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
            player_tab: PlayerTab { items: Vec::new(), playlist_cursor: 0 },
            home: HomePane {
                continue_items: Vec::new(),
                continue_cursor: 0,
                latest: Vec::new(),
                section: 0,
            },
            libs: Vec::new(),
            status: String::new(),
            status_expires: None,
            log: crate::applog::AppLog::new(0),
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
            layout_lib_scroll: 0,
            layout_lib_row_heights: Vec::new(),
            layout_home_scrolls: Vec::new(),
            layout_home_scrollbar: Rect::default(),
            home_panel_section_offset: 0,
            layout_lib_table_area: ratatui::layout::Rect::default(),
            layout_breadcrumbs: Vec::new(),
            last_click_time: std::time::Instant::now(),
            last_drag_seek: std::time::Instant::now(),
            last_click_pos: (u16::MAX, u16::MAX),
            layout_seekbar_area: ratatui::layout::Rect::default(),
            layout_button_area: ratatui::layout::Rect::default(),
            layout_tracks_area: ratatui::layout::Rect::default(),
            layout_vol_area: ratatui::layout::Rect::default(),
            layout_sub_area: ratatui::layout::Rect::default(),
            layout_sub_indicator_area: ratatui::layout::Rect::default(),
            layout_audio_indicator_area: ratatui::layout::Rect::default(),
            layout_audio_area: ratatui::layout::Rect::default(),
            confirm_remove_idx: None,
            pending_queue_removal: None,
            confirm_clear_playlist: false,
            skip_intro_end_ticks: None,
            next_up_item: None,
            playlist_view: 0,
            home_card_view: false,
            last_played_item_id: None,
            layout_carousel_slots: [(None, ratatui::layout::Rect::default()); 3],
            layout_carousel_left_arrow: None,
            layout_carousel_right_arrow: None,
            layout_carousel_up_arrow: None,
            layout_carousel_down_arrow: None,
            last_carousel_click_slot: None,
            last_carousel_click_time: std::time::Instant::now(),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
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
            help_scroll: 0,
            show_log_tab: false,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            force_clear: false,
            tab_scroll: 0,
            ui_volume: 100,
            pre_mute_volume: None,
            layout_tabbar_vol_area: Rect::default(),
            sessions: Vec::new(),
            sessions_cursor: 0,
            sessions_loading: false,
            show_sessions: false,
            sessions_tx,
            sessions_rx,
            connected_session_id: None,
            connected_session_state: None,
            last_session_poll: std::time::Instant::now(),
            remote_pos_s: 0,
            remote_pos_at: std::time::Instant::now(),
            layout_sessions_btn_area: Rect::default(),
            last_scroll_at: Instant::now() - Duration::from_secs(1),
        }
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
        app.home.latest = vec![
            ("Latest Movies".into(), "lib1".into(), make_items(7), 2),
        ];
        assert_eq!(app.home_section_len_cur(1), (7, 2));
    }

    #[test]
    fn home_section_len_cur_out_of_bounds_returns_zero() {
        let app = make_app_stub();
        assert_eq!(app.home_section_len_cur(99), (0, 0));
    }

    // ── set_home_cursor ──────────────────────────────────────────────────────

    #[test]
    fn set_home_cursor_section_zero_sets_continue_cursor() {
        let mut app = make_app_stub();
        app.home.continue_items = make_items(5);
        app.set_home_cursor(0, 4);
        assert_eq!(app.home.continue_cursor, 4);
    }

    #[test]
    fn set_home_cursor_section_one_sets_latest_cursor() {
        let mut app = make_app_stub();
        app.home.latest = vec![("T".into(), "lib".into(), make_items(10), 0)];
        app.set_home_cursor(1, 7);
        assert_eq!(app.home.latest[0].3, 7);
    }

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
        (0..n).map(|i| (format!("Sec {i}"), format!("lib{i}"), make_items(3), 0)).collect()
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

    // ── home_card_view toggle ────────────────────────────────────────────────

    #[test]
    fn home_card_view_defaults_false() {
        let app = make_app_stub();
        assert!(!app.home_card_view);
    }

    #[test]
    fn home_card_view_toggle_changes_state() {
        let mut app = make_app_stub();
        app.home_card_view = !app.home_card_view;
        assert!(app.home_card_view);
        app.home_card_view = !app.home_card_view;
        assert!(!app.home_card_view);
    }

    // ── cursor preservation during home refresh ──────────────────────────────

    #[test]
    fn home_refresh_preserves_cursor_by_lib_id() {
        // Simulate what init_home does: old_cursors keyed by lib_id.
        let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
            ("Latest Movies".into(), "lib-movies".into(), make_items(10), 7),
            ("Latest TV".into(),     "lib-tv".into(),     make_items(5),  3),
        ];
        let old_cursors: std::collections::HashMap<String, usize> = old_latest.iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        // New fetch returns same libs but with fresh items.
        let new_items_movies = make_items(12);
        let new_items_tv     = make_items(4);

        let cursor_movies = old_cursors.get("lib-movies").copied().unwrap_or(0)
            .min(new_items_movies.len().saturating_sub(1));
        let cursor_tv = old_cursors.get("lib-tv").copied().unwrap_or(0)
            .min(new_items_tv.len().saturating_sub(1));

        assert_eq!(cursor_movies, 7, "cursor preserved when within bounds");
        assert_eq!(cursor_tv, 3,     "cursor preserved when within bounds");
    }

    #[test]
    fn home_refresh_clamps_cursor_when_new_list_is_shorter() {
        let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
            ("Latest Movies".into(), "lib-movies".into(), make_items(10), 9),
        ];
        let old_cursors: std::collections::HashMap<String, usize> = old_latest.iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        let new_items = make_items(5); // shorter than before
        let cursor = old_cursors.get("lib-movies").copied().unwrap_or(0)
            .min(new_items.len().saturating_sub(1));

        assert_eq!(cursor, 4, "cursor clamped to new last index");
    }

    #[test]
    fn home_refresh_cursor_defaults_zero_for_new_library() {
        let old_cursors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let new_items = make_items(8);
        let cursor = old_cursors.get("brand-new-lib").copied().unwrap_or(0)
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

fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, Box<dyn std::error::Error>> {
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

// Resize image bytes using ImageMagick (Lanczos) if available, otherwise return None.
fn magick_resize(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // IM7 uses `magick convert`; IM6 uses `convert` directly.
    let args: &[&[&str]] = &[
        &["magick", "convert"],
        &["convert"],
    ];
    for a in args {
        let (cmd, extra) = (a[0], &a[1..]);
        let mut child = Command::new(cmd)
            .args(extra)
            .args(["-", "-filter", "Lanczos", "-resize", "400x400>", "-quality", "85", "png:-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(bytes).ok()?;
        let out = child.wait_with_output().ok()?;
        if out.status.success() && !out.stdout.is_empty() {
            return Some(out.stdout);
        }
    }
    None
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
    crossterm::terminal::disable_raw_mode()?;
    let _ = crossterm::execute!(terminal.backend_mut(), crossterm::event::PopKeyboardEnhancementFlags);
    crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture)?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
