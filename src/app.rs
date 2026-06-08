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
    widgets::{Block, BorderType, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs},
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
    MarkPlayed(String),
    MarkUnplayed(String),
    RemoveFromContinueWatching,
    RemoveFromPlaylist(usize),
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

mod palette {
    use ratatui::style::Color;
    pub const BASE:          Color = Color::Rgb(26,  26,  26);   // near-black, for text on colored bg
    pub const OVERLAY:       Color = Color::Rgb(63,  63,  63);   // gray, unfocused borders
    pub const MUTED:         Color = Color::Rgb(108, 108, 108);  // dim text, icons
    pub const SUBTLE:        Color = Color::Rgb(158, 158, 158);  // secondary text
    pub const TEXT:          Color = Color::Rgb(230, 230, 230);  // primary text
    pub const WHITE:         Color = Color::Rgb(253, 253, 253);  // near-white (#fdfdfd)
    pub const YELLOW:        Color = Color::Rgb(250, 220, 70);   // yellow — in-progress, paused
    pub const PINE:          Color = Color::Rgb(61,  139, 55);   // dark green — folders, watched
    pub const FOAM:          Color = Color::Rgb(0,   164, 220);  // emby blue — now-playing item
    pub const IRIS:          Color = Color::Rgb(82,  181, 75);   // emby green — active tab, focused
    pub const HIGHLIGHT_MED: Color = Color::Rgb(42,  42,  42);   // selection row bg (#2a2a2a)
    pub const STRIPE:        Color = Color::Rgb(58,  58,  58);   // zebra stripe row bg (#3a3a3a)
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
    home_panel_section_offset: usize,
    layout_lib_table_area: Rect,
    layout_breadcrumbs: Vec<(u16, u16, usize)>, // (x_start, x_end, target nav_stack len)
    last_click_time: Instant,
    last_click_pos: (u16, u16),
    last_drag_seek: Instant,
    layout_seekbar_area: Rect,
    layout_button_area: Rect,
    layout_tracks_area: Rect,
    layout_vol_area: Rect,
    layout_sub_area: Rect,
    layout_audio_area: Rect,
    confirm_remove_idx: Option<usize>, // playlist index pending removal confirmation
    confirm_clear_playlist: bool,
    next_up_item: Option<MediaItem>,
    playlist_card_view: bool,
    home_card_view: bool,
    last_played_item_id: Option<String>,
    layout_carousel_slots: [(Option<usize>, Rect); 3],
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
    help_scroll: u16,
    show_log_tab: bool,
    lib_tx: mpsc::Sender<LibEvent>,
    lib_rx: mpsc::Receiver<LibEvent>,
    force_clear: bool,
    last_lib_tab: usize,
    lib_picker_open: bool,
    lib_picker_cursor: usize,
}

const HOME_MIN_SECTION_H: u16 = 6; // 2 border rows + 4 content rows

impl App {
    pub fn new(client: EmbyClient) -> Self {
        let (player_tx, player_rx) = mpsc::channel();
        let (ws_tx, ws_rx) = mpsc::channel();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (card_image_tx, card_image_rx) = mpsc::channel::<(String, Option<Vec<u8>>)>();
        let server_url = client.config.server_url.clone();
        let token = client.token.clone();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let show_log_tab = client.config.show_log_tab;
        let ws_url = client.ws_url();
        let log = AppLog::new(if show_log_tab { 5000 } else { 0 });
        let ws_send_tx = crate::ws::start(ws_url, ws_tx, log.clone());
        let always_play_next = client.config.always_play_next;
        let raw_player = Player::new(server_url, token, client.config.show_audio_window, client.config.use_mpv_config, client.config.no_scripts, always_play_next, player_tx, Some(ws_send_tx));
        let player_status = raw_player.status.clone();
        let player_cmd_tx = raw_player.cmd_tx.clone();
        crate::mpris::start(player_status, move |cmd| {
            if let Some(tx) = player_cmd_tx.lock().unwrap().as_ref() {
                let _ = tx.send(cmd);
            }
        });
        let player = PlayerProxy::local(raw_player, always_play_next);
        App {
            client: Arc::new(Mutex::new(client)),
            player,
            player_rx,
            ws_rx,
            tab_idx: 0,
            hidden_libraries,
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
            layout_audio_area: Rect::default(),
            confirm_remove_idx: None,
            confirm_clear_playlist: false,
            next_up_item: None,
            playlist_card_view: Self::load_playlist_card_view(),
            home_card_view: Self::load_home_card_view(),
            last_played_item_id: None,
            layout_carousel_slots: [(None, Rect::default()); 3],
            last_carousel_click_slot: None,
            last_carousel_click_time: Instant::now() - Duration::from_secs(1),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            card_image_tx,
            card_image_rx,
            image_picker: None,
            show_help: false,
            help_scroll: 0,
            show_log_tab,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            force_clear: false,
            last_lib_tab: 2,
            lib_picker_open: false,
            lib_picker_cursor: 0,
        }
    }

    pub fn new_remote(
        client: EmbyClient,
        remote: crate::remote_player::RemotePlayer,
        player_rx: mpsc::Receiver<PlayerEvent>,
    ) -> Self {
        let (_, ws_rx) = mpsc::channel::<crate::ws::WsEvent>();
        let (lib_tx, lib_rx) = mpsc::channel();
        let (card_image_tx, card_image_rx) = mpsc::channel::<(String, Option<Vec<u8>>)>();
        let hidden_libraries = client.config.hidden_libraries.clone();
        let show_log_tab = client.config.show_log_tab;
        let always_play_next = client.config.always_play_next;
        let log = AppLog::new(if show_log_tab { 5000 } else { 0 });
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
            layout_audio_area: Rect::default(),
            confirm_remove_idx: None,
            confirm_clear_playlist: false,
            next_up_item: None,
            playlist_card_view: Self::load_playlist_card_view(),
            home_card_view: Self::load_home_card_view(),
            last_played_item_id: None,
            layout_carousel_slots: [(None, Rect::default()); 3],
            last_carousel_click_slot: None,
            last_carousel_click_time: Instant::now() - Duration::from_secs(1),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            card_image_tx,
            card_image_rx,
            image_picker: None,
            show_help: false,
            help_scroll: 0,
            show_log_tab,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            force_clear: false,
            last_lib_tab: 2,
            lib_picker_open: false,
            lib_picker_cursor: 0,
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode.
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.client.lock().unwrap().config.card_image_protocol.clone();
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
                    PlayerEvent::Stopped { idx, position_ticks } => {
                        if self.player.is_remote_disconnected() {
                            self.next_up_item = None;
                            self.status = "Daemon disconnected — playback stopped".into();
                            self.refresh_after_stop();
                            continue;
                        }
                        if let Some(item) = self.player_tab.items.get_mut(idx) {
                            if position_ticks > 0 {
                                item.playback_position_ticks = position_ticks;
                            }
                            self.last_played_item_id = Some(item.id.clone());
                        }
                        self.next_up_item = None;
                        self.status.clear();
                        self.refresh_after_stop();
                    }
                    PlayerEvent::TrackChanged(idx) => {
                        self.player_tab.playlist_cursor = idx;
                        if let Some(item) = self.player_tab.items.get(idx) {
                            self.last_played_item_id = Some(item.id.clone());
                        }
                    }
                    PlayerEvent::PlaylistNextUp { next_idx } => {
                        if let Some(item) = self.player_tab.items.get(next_idx) {
                            let item_id = item.id.clone();
                            let title   = item.display_name();
                            self.next_up_item = Some(item.clone());
                            // Daemon sends NextUpShow to mpv directly; only send from local player.
                            if !self.player.is_remote() {
                                self.player.send_command(PlayerCommand::NextUpShow { item_id, title });
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
                }
            }

            while let Ok(ev) = self.lib_rx.try_recv() {
                self.handle_lib_event(ev);
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
        if !self.player.is_remote() {
            self.player.stop();
        }
        self.player.join();
        self.save_playlist();
        restore_terminal(terminal)?;
        Ok(())
    }

    fn tab_count(&self) -> usize { 2 + self.libs.len() }
    fn log_tab_idx(&self) -> usize { 2 + self.libs.len() }

    fn open_lib_picker(&mut self) {
        if self.libs.len() > 1 {
            self.lib_picker_cursor = self.tab_idx.saturating_sub(2).min(self.libs.len().saturating_sub(1));
            self.lib_picker_open = true;
        }
    }

    fn set_tab_by_number(&mut self, c: char) {
        match c {
            '1' => self.set_tab(0),
            '2' => self.set_tab(1),
            '3' => { if !self.libs.is_empty() { self.set_tab(self.last_lib_tab); } }
            _ => {}
        }
    }

    fn next_logical_tab(&self) -> usize {
        if self.tab_idx == 0 { 1 }
        else if self.tab_idx == 1 { if self.libs.is_empty() { 0 } else { self.last_lib_tab } }
        else { 0 }
    }

    fn prev_logical_tab(&self) -> usize {
        if self.tab_idx == 0 { if self.libs.is_empty() { 1 } else { self.last_lib_tab } }
        else if self.tab_idx == 1 { 0 }
        else { 1 }
    }

    // ── key handling ────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.show_help {
            match key.code {
                KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
                KeyCode::Esc | KeyCode::F(1) => { self.show_help = false; }
                KeyCode::Up       => { self.help_scroll = self.help_scroll.saturating_sub(1); }
                KeyCode::Down     => { self.help_scroll += 1; }
                KeyCode::PageUp   => { self.help_scroll = self.help_scroll.saturating_sub(10); }
                KeyCode::PageDown => { self.help_scroll += 10; }
                KeyCode::Home     => { self.help_scroll = 0; }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::F(1) {
            self.show_help = true;
            return false;
        }
        // Global c: clear playlist (not when typing in library search)
        let in_lib_search = self.tab_idx > 1
            && self.tab_idx != self.log_tab_idx()
            && self.libs.get(self.tab_idx - 2).is_some_and(|l| l.search.is_some());
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
                KeyCode::Esc => { self.context_menu = None; }
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
                        let action = m.actions.get(m.cursor).cloned();
                        self.execute_context_action(action);
                    }
                }
                _ => {}
            }
            return false;
        }
        if self.lib_picker_open {
            match key.code {
                KeyCode::Up => {
                    if self.lib_picker_cursor > 0 { self.lib_picker_cursor -= 1; }
                }
                KeyCode::Down => {
                    if self.lib_picker_cursor + 1 < self.libs.len() { self.lib_picker_cursor += 1; }
                }
                KeyCode::Enter => {
                    let idx = 2 + self.lib_picker_cursor;
                    self.lib_picker_open = false;
                    self.set_tab(idx);
                }
                KeyCode::Esc | KeyCode::Char('\\') => { self.lib_picker_open = false; }
                _ => {}
            }
            return false;
        }
        if self.show_log_tab && key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::ALT) {
            self.tab_idx = self.log_tab_idx();
            return false;
        }
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT) {
            self.enqueue_selected();
            return false;
        }
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.force_clear = true;
            return false;
        }
        if self.tab_idx == 0 { return self.handle_combined_key(key); }
        if self.tab_idx == 1 { return self.handle_playlist_key(key); }
        if self.tab_idx == self.log_tab_idx() { return self.handle_log_key(key); }
        let lib_idx = self.tab_idx - 2;
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // When search is active, most keys feed the query
        if self.libs[lib_idx].search.is_some() {
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
                KeyCode::Char('s') if alt => self.shuffle_play(),
                KeyCode::Char('o') if alt => self.open_context_menu(),
                KeyCode::Char(c) if !alt => {
                    self.libs[lib_idx].search.as_mut().unwrap().query.push(c);
                    self.update_lib_search(lib_idx);
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => { if !self.player.is_remote() { self.player.stop(); } return true; }
            KeyCode::Tab => { self.set_tab(self.next_logical_tab()); }
            KeyCode::BackTab => { self.set_tab(self.prev_logical_tab()); }
            KeyCode::Char('\\') => { self.open_lib_picker(); }
            KeyCode::Esc | KeyCode::Backspace => self.go_back(),
            KeyCode::Up       => self.move_lib_cursor(-1),
            KeyCode::Down     => self.move_lib_cursor(1),
            KeyCode::PageUp   => { let p = self.lib_page_size(); self.move_lib_cursor(-(p as i64)); }
            KeyCode::PageDown => { let p = self.lib_page_size(); self.move_lib_cursor(p as i64); }
            KeyCode::Home     => self.jump_lib_cursor(false),
            KeyCode::End      => self.jump_lib_cursor(true),
            KeyCode::Enter => self.select(),
            KeyCode::Char('w') if alt => self.toggle_watched(),
            KeyCode::Char('s') if alt => self.shuffle_play(),
            KeyCode::Char('o') if alt => self.open_context_menu(),
            KeyCode::Char(c @ '1'..='3') => { self.set_tab_by_number(c); }
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
            KeyCode::Tab => { self.set_tab(self.next_logical_tab()); return false; }
            KeyCode::BackTab => { self.set_tab(self.prev_logical_tab()); return false; }
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
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.home_card_view = !self.home_card_view;
                self.save_home_card_view();
                if !self.card_image_states.is_empty() { self.force_clear = true; }
                return false;
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.open_context_menu(); return false;
            }
            KeyCode::Char(c @ '1'..='3') => { self.set_tab_by_number(c); return false; }
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
            KeyCode::Enter => self.select_home(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::ALT) => self.toggle_watched_home(),
            _ => {}
        }
        false
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
        let active = self.player.status.lock().unwrap().active;
        if !active { return None; }
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter if alt => { self.player.stop(); Some(false) }
            KeyCode::Char(' ') => { self.player.send_command(PlayerCommand::TogglePause); Some(false) }
            KeyCode::Left  if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(-5.0)); Some(false) }
            KeyCode::Right if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(5.0));  Some(false) }
            KeyCode::Char('-') => {
                let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                self.player.send_command(PlayerCommand::SetVolume(v));
                Some(false)
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let st = self.player.status.lock().unwrap();
                let v = (st.volume + 5).min(st.volume_max);
                drop(st);
                self.player.send_command(PlayerCommand::SetVolume(v));
                Some(false)
            }
            KeyCode::Char('a') if alt => { self.cycle_audio(); Some(false) }
            KeyCode::Char('z') if alt => { self.cycle_sub();   Some(false) }
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
            KeyCode::Tab => { self.set_tab(self.next_logical_tab()); }
            KeyCode::BackTab => { self.set_tab(self.prev_logical_tab()); }
            KeyCode::Up | KeyCode::Left
                if self.player_tab.playlist_cursor > 0 && (key.code == KeyCode::Up || self.playlist_card_view) => {
                    self.player_tab.playlist_cursor -= 1;
                }
            KeyCode::Down | KeyCode::Right
                if self.player_tab.playlist_cursor + 1 < self.player_tab.items.len()
                && (key.code == KeyCode::Down || self.playlist_card_view) => {
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
                    let active = self.player.status.lock().unwrap().active;
                    if active {
                        self.player.send_command(PlayerCommand::JumpTo(t));
                    } else if !self.player_tab.items.is_empty() {
                        let items = self.player_tab.items.clone();
                        let c = Arc::new(self.client.lock().unwrap().clone());
                        self.player.play_playlist(items, t, c, self.log.clone());
                    }
                }
            }
            KeyCode::Delete => {
                let t = self.player_tab.playlist_cursor;
                if t < self.player_tab.items.len() { self.remove_from_playlist(t); }
            }
            KeyCode::Char(c @ '1'..='3') => { self.set_tab_by_number(c); }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.open_context_menu();
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.playlist_card_view = !self.playlist_card_view;
                self.save_playlist_card_view();
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
            KeyCode::Char(c @ '1'..='3') => { self.set_tab_by_number(c); }
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

    fn tab_title_widths(&self) -> Vec<u16> {
        let lib_name = self.libs.get(self.tab_idx.saturating_sub(2))
            .map(|l| l.library.name.as_str())
            .unwrap_or("Libraries");
        vec![
            "Home".chars().count() as u16,
            "Queue".chars().count() as u16,
            lib_name.chars().count() as u16,
        ]
    }

    fn tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout_tabs_area;
        if col < area.x || col >= area.x + area.width { return None; }
        let rel = col - area.x;
        let widths = self.tab_title_widths();
        // ratatui Tabs renders: pad(1) + title(w) + pad(1) + divider(3) per tab
        let pad = 1u16;
        let div_w = 3u16; // " • "
        let mut x = 0u16;
        for (i, &w) in widths.iter().enumerate() {
            let is_last = i + 1 == widths.len();
            let end = x + pad + w + pad + if is_last { 0 } else { div_w };
            if rel < end { return Some(i); }
            x = end;
        }
        None
    }

    fn handle_button_click(&mut self, btn: usize) {
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
                items.push("Mark Watched");
                actions.push(ContextAction::MarkPlayed(item.id.clone()));
                items.push("Mark Unwatched");
                actions.push(ContextAction::MarkUnplayed(item.id.clone()));
            } else {
                items.push("Play");
                actions.push(ContextAction::Play);
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
            let lib_idx = self.tab_idx - 2;
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

    fn load_playlist_card_view() -> bool {
        Self::load_prefs()["playlist_card_view"].as_bool().unwrap_or(false)
    }

    fn load_home_card_view() -> bool {
        Self::load_prefs()["home_card_view"].as_bool().unwrap_or(false)
    }

    fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        let v = serde_json::json!({
            "playlist_card_view": self.playlist_card_view,
            "home_card_view": self.home_card_view,
        });
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }

    fn save_playlist_card_view(&self) { self.save_prefs(); }
    fn save_home_card_view(&self) { self.save_prefs(); }

    fn save_playlist(&self) {
        let ids: Vec<&str> = self.player_tab.items.iter().map(|i| i.id.as_str()).collect();
        let payload = serde_json::json!({
            "ids": ids,
            "cursor": self.player_tab.playlist_cursor,
            "last_played_item_id": self.last_played_item_id,
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

        // Support both new format {"ids":[...],"cursor":N} and old format [...]
        let (ids, saved_cursor, last_played_item_id): (Vec<String>, usize, Option<String>) = if v.is_array() {
            let ids = serde_json::from_value(v).unwrap_or_default();
            (ids, 0, None)
        } else {
            let ids = v["ids"].as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let cursor = v["cursor"].as_u64().unwrap_or(0) as usize;
            let last_id = v["last_played_item_id"].as_str().map(String::from);
            (ids, cursor, last_id)
        };

        if ids.is_empty() { return }
        let client = self.client.lock().unwrap();
        if let Ok(mut items) = client.get_items_by_ids(&ids) {
            // preserve original order (get_items_by_ids may reorder)
            items.sort_by_key(|item| ids.iter().position(|id| id == &item.id).unwrap_or(usize::MAX));
            drop(client);
            let cursor = saved_cursor.min(items.len().saturating_sub(1));
            self.last_played_item_id = last_played_item_id;
            self.player_tab.playlist_cursor = cursor;
            self.player_tab.items = items;
        }
    }

    fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout_seekbar_area;
        if bar.width == 0 { return; }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 { return; }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
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
                let lib = &mut self.libs[self.tab_idx - 2];
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
                if let Some(visual_idx) = self.tab_idx_at(col) {
                    match visual_idx {
                        0 => self.set_tab(0),
                        1 => self.set_tab(1),
                        2 => {
                            if self.tab_idx >= 2 {
                                // already on Libraries — open picker to switch
                                self.open_lib_picker();
                            } else if !self.libs.is_empty() {
                                self.set_tab(self.last_lib_tab);
                            }
                        }
                        _ => {}
                    }
                }
                return;
            }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollDown) { 1 } else { -1 };
                if self.layout_vol_area.contains((col, row).into()) {
                    let st = self.player.status.lock().unwrap();
                    let v = (st.volume as i64 - delta * 5).clamp(0, st.volume_max);
                    drop(st);
                    self.player.send_command(PlayerCommand::SetVolume(v));
                    return;
                }
                if self.tab_idx == 0 {
                    if self.home_rect.contains((col, row).into()) {
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
                                self.execute_context_action(action);
                            } else {
                                self.context_menu = None;
                            }
                            return;
                        }
                    }
                    self.context_menu = None;
                    return;
                }

                let now = Instant::now();

                // Carousel card clicks — own double-click tracking independent of position-exact is_double
                if self.tab_idx == 1 && self.playlist_card_view {
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
                                            self.player.play_playlist(items, *item_idx, c, self.log.clone());
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
                            self.player.send_command(PlayerCommand::JumpTo(t));
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
                if self.layout_sub_area.contains((col, row).into()) {
                    self.cycle_sub();
                    return;
                }
                if self.layout_audio_area.contains((col, row).into()) {
                    self.cycle_audio();
                    return;
                }
                if self.layout_vol_area.contains((col, row).into()) {
                    let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                    self.player.send_command(PlayerCommand::SetVolume(v));
                    return;
                }

                // Breadcrumb click: navigate back to that depth
                if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
                    let crumbs = self.layout_breadcrumbs.clone();
                    for (x_start, x_end, target_depth) in crumbs {
                        if col >= x_start && col < x_end {
                            let lib = &mut self.libs[self.tab_idx - 2];
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
                    let st = self.player.status.lock().unwrap();
                    let v = (st.volume + 5).min(st.volume_max);
                    drop(st);
                    self.player.send_command(PlayerCommand::SetVolume(v));
                    return;
                }
                if self.click_set_cursor(col, row) {
                    self.open_context_menu_at(col, row);
                }
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
        let lib = &mut self.libs[self.tab_idx - 2];
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
        let lib = &mut self.libs[self.tab_idx - 2];
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

        let n_sections = 1 + self.home.latest.len();
        let visible = if (n_sections as u16) * HOME_MIN_SECTION_H <= panel_h {
            n_sections
        } else {
            ((panel_h / HOME_MIN_SECTION_H) as usize).max(1)
        };

        let sec = self.home.section;
        if sec < self.home_panel_section_offset {
            self.home_panel_section_offset = sec;
        } else if sec >= self.home_panel_section_offset + visible {
            self.home_panel_section_offset = sec + 1 - visible;
        }
        let max_offset = n_sections.saturating_sub(visible);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
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
        let lib = self.libs.get(self.tab_idx - 2)?;
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

    fn cycle_audio(&mut self) {
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        if tracks.is_empty() { return; }
        let cur = tracks.iter().position(|(id, _)| *id == current_id).unwrap_or(0);
        let next = (cur + 1) % tracks.len();
        self.player.send_command(PlayerCommand::SetAudio(tracks[next].0));
    }

    fn cycle_sub(&mut self) {
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.sub_tracks.clone(), s.sub_id)
        };
        // Off + English text subs only
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter()
            .filter(|(_, l)| crate::player::is_english(l))
            .map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        self.player.send_command(PlayerCommand::SetSub(entries[next]));
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
        if !item.series_id.is_empty() && self.player.always_play_next {
            let c = self.client.lock().unwrap();
            let episodes = c.get_episodes_from(&item.series_id, &item.id, &self.log);
            drop(c);
            if episodes.len() > 1 {
                let c = Arc::new(self.client.lock().unwrap().clone());
                self.player_tab.items = episodes.clone();
                self.player_tab.playlist_cursor = 0;
                self.flash_status(label);
                self.player.play_playlist(episodes, 0, c, self.log.clone());
                return;
            }
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        self.player_tab.items = vec![item.clone()];
        self.player_tab.playlist_cursor = 0;
        self.flash_status(label);
        self.player.play(&item, c, self.log.clone());
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
            let lib_idx = self.tab_idx - 2;
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
            self.play_item(fresh);
        }
    }

    fn go_back(&mut self) {
        if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib = &mut self.libs[self.tab_idx - 2];
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
                else if self.tab_idx == 1 { self.player.send_command(PlayerCommand::JumpTo(self.player_tab.playlist_cursor)); }
                else { self.select(); }
            }
            Some(ContextAction::PlayFolder(id)) => self.play_folder(&id),
            Some(ContextAction::ShuffleFolder(id)) => self.shuffle_folder(&id),
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
        if item.is_folder { return; }
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
        if item.is_folder { return; }
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
        let lib_idx = self.tab_idx - 2;
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.loading = true;
            let parent_id = lvl.parent_id.clone();
            let item_types = lvl.item_types.clone();
            let unplayed_only = lvl.unplayed_only;
            self.spawn_refresh(lib_idx, parent_id, item_types, unplayed_only);
        }
    }

    fn shuffle_play(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let lib_idx = self.tab_idx - 2;
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
                self.player.play_playlist(items, 0, c, self.log.clone());
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
                self.player.play_playlist(items, 0, c, self.log.clone());
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
                self.player.play_playlist(items, 0, c, self.log.clone());
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
        if idx >= 2 && idx != self.log_tab_idx() {
            self.last_lib_tab = idx;
        }
        if self.tab_idx == 0 {
            self.home.section = 0;
            let _ = self.fetch_home();
        } else {
            self.ensure_library_loaded();
        }
    }

    fn ensure_library_loaded(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let idx = self.tab_idx - 2;
        if self.libs[idx].nav_stack.is_empty() {
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let (item_types, unplayed_only) = match self.libs[idx].library.collection_type.as_str() {
                "movies"               => (Some("Movie".to_string()), false),
                "channels"|"homevideos"=> (Some("Video".to_string()), true),
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
        for v in &user_views {
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
                    self.player.play(&item, c, self.log.clone());
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
                        self.player.play(&start_item, c, self.log.clone());
                    } else {
                        let mut items_with_pos = items.clone();
                        if start_position_ticks > 0 {
                            items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                        }
                        self.player.play_playlist(items_with_pos, start_idx, c, self.log.clone());
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
        if self.show_help { self.render_help_screen(f); return; }
        let area = f.area();
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;

        let active = self.player.status.lock().unwrap().active;
        let controls_h: u16 = if active { 3 } else { 0 };
        let sep_h: u16 = if active { 1 } else { 0 };

        let [tabs_area, _gap_area, main_area, sep_area, status_area, controls_area] = Layout::vertical([
            Constraint::Length(1),            // tabs
            Constraint::Length(1),            // spacer
            Constraint::Min(0),               // main content
            Constraint::Length(sep_h),        // separator
            Constraint::Length(1),            // status line (now playing)
            Constraint::Length(controls_h),   // playback controls (global, when active)
        ]).areas(area);

        self.layout_tabs_area = tabs_area;

        // Tab bar: always 3 logical tabs — Home, Queue, Libraries (current lib name)
        let lib_label = if self.tab_idx >= 2 && self.tab_idx != self.log_tab_idx() {
            self.libs.get(self.tab_idx - 2).map(|l| l.library.name.clone())
        } else {
            self.libs.get(self.last_lib_tab.saturating_sub(2)).map(|l| l.library.name.clone())
        }.unwrap_or_else(|| "Libraries".to_string());
        let tab_titles = vec![Span::raw("Home"), Span::raw("Queue"), Span::raw(lib_label)];
        let tab_select = if self.tab_idx == self.log_tab_idx() { usize::MAX }
            else if self.tab_idx >= 2 { 2 }
            else { self.tab_idx };
        f.render_widget(
            Tabs::new(tab_titles)
                .select(tab_select)
                .divider(Span::styled(" • ", Style::default().fg(palette::IRIS)))
                .highlight_style(Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
            tabs_area,
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
        // Any explicit status (flash or persistent prompt) beats now_playing
        let (status_text, status_color) = if !self.status.is_empty() {
            let color = if self.status_expires.is_some() { palette::YELLOW } else { palette::YELLOW };
            (Some(self.status.as_str()), color)
        } else {
            (now_playing.as_deref(), palette::FOAM)
        };
        if let Some(text) = status_text {
            f.render_widget(
                Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                status_area,
            );
        }

        if active {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "\u{2500}".repeat(area.width as usize),
                    Style::default().fg(palette::MUTED),
                )),
                sep_area,
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
            self.render_library(f, main_area, self.tab_idx - 2);
        }

        self.render_context_menu(f);
        self.render_lib_picker(f);
    }

    fn render_lib_picker(&mut self, f: &mut ratatui::Frame) {
        if !self.lib_picker_open || self.libs.is_empty() { return; }
        let area = f.area();
        let w = (area.width / 2).max(30).min(area.width);
        let h = (self.libs.len() as u16 + 2).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(w)) / 2;
        let y = (area.height.saturating_sub(h)) / 2;
        let popup = Rect { x, y, width: w, height: h };
        f.render_widget(ratatui::widgets::Clear, popup);
        let block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title(" Libraries ")
            .border_style(Style::default().fg(palette::IRIS));
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        let visible_h = inner.height as usize;
        let scroll = if self.lib_picker_cursor >= visible_h {
            self.lib_picker_cursor + 1 - visible_h
        } else {
            0
        };
        for (row, lib) in self.libs.iter().enumerate().skip(scroll).take(visible_h) {
            let y = inner.y + (row - scroll) as u16;
            let selected = row == self.lib_picker_cursor;
            let style = if selected {
                Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT)
            };
            let prefix = if selected { "> " } else { "  " };
            f.render_widget(
                Paragraph::new(format!("{}{}", prefix, lib.library.name)).style(style),
                Rect { x: inner.x, y, width: inner.width, height: 1 },
            );
        }
    }

    fn render_help_screen(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();

        let [header_area, sep_area, content_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ]).areas(area);

        // Header bar: title left, hint right
        let title = " ⌨  Keyboard Shortcuts";
        let hint  = "↑↓ / mouse · Esc to close ";
        let pad = (area.width as usize).saturating_sub(title.len() + hint.len());
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(title, Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                Span::raw(" ".repeat(pad)),
                Span::styled(hint, Style::default().fg(palette::MUTED)),
            ])),
            header_area,
        );

        // Separator
        f.render_widget(
            Paragraph::new(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(palette::OVERLAY),
            )),
            sep_area,
        );

        // Build content lines
        let w = content_area.width as usize;
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
            mk("Tab",              "Cycle menu"),
            mk("1 – 9",            "Jump to tab"),
            mk("↑ / ↓",            "Move cursor"),
            mk("PgUp / PgDn",      "Page scroll"),
            mk("Home / End",       "First / last"),
            mk("Enter",            "Select / Play / Open"),
            mk("Alt+Q",            "Add item(s) to Queue"),
            mk("Alt+O",            "Context menu"),
            mk("c",                "Clear Queue (confirms)"),
            mk("q",                "Quit"),
        ];
        lines.extend(vec![

            blank(),
            section("PLAYBACK"),
            mk("Space",            "Pause / Resume"),
            mk("Alt+← / →",       "Seek ±5 seconds"),
            mk("Alt+Enter",        "Stop"),
            mk("- / +",            "Volume down / up"),
            mk("Alt+A",            "Cycle audio track"),
            mk("Alt+Z",            "Enable subtitles"),

            blank(),
            section("QUEUE"),
            mk(".",                "Jump to playing item"),
            mk("Del",              "Remove from Queue"),
            mk("Alt+V",            "Toggle view"),

            blank(),
            section("HOME"),
            mk("Alt+↑ / ↓",        "Switch sections"),
            mk("Alt+W",            "Toggle watched"),

            blank(),
            section("LIBRARY"),
            mk("Esc / Backspace",  "Go back"),
            mk("/",                "Search library"),
            mk("Alt+W",            "Toggle watched"),
            mk("Alt+S",            "Shuffle and play selection"),

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
        let visible = content_area.height as usize;
        self.help_scroll = self.help_scroll.min(total.saturating_sub(visible) as u16);

        f.render_widget(
            Paragraph::new(lines)
                .scroll((self.help_scroll, 0)),
            content_area,
        );
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

        let (position_ticks, runtime_ticks, paused, volume, volume_max,
             audio_tracks, sub_tracks, audio_id, sub_id) = {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused, s.volume, s.volume_max,
             s.audio_tracks.clone(), s.sub_tracks.clone(), s.audio_id, s.sub_id)
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

        let audio_label = if audio_tracks.is_empty() {
            "\u{2014}".to_string()
        } else {
            let pos = audio_tracks.iter().position(|(id, _)| *id == audio_id).unwrap_or(0);
            let label = audio_tracks.iter().find(|(id, _)| *id == audio_id)
                .map(|(_, l)| l.as_str()).unwrap_or("\u{2014}");
            format!("{} ({}/{})", label, pos + 1, audio_tracks.len())
        };
        let sub_label = if sub_id == 0 || sub_tracks.is_empty() { "Off" } else { "On" };
        let vol_chars = ['▁','▂','▃','▄','▅','▆','▇','█'];
        let vol_icon = vol_chars[(volume as usize * 8 / (volume_max as usize + 1)).min(7)];

        const BTNS_W: u16 = 30; // 6 buttons × 5
        let btn_x = area.x + area.width.saturating_sub(BTNS_W) / 2;

        // Each info group gets 1 char padding on each side; groups separated by 2 spaces
        let vol_pct = format!("{volume}%");
        let g1_inner = format!("Subs: \u{2261} {}", sub_label);        // Subs: ≡ sub
        let g2_inner = format!("\u{266a} {}", audio_label);          // ♪ audio
        let g3_inner = format!("{vol_icon} {vol_pct}");               // ▄ vol%
        let pad = 1u16;
        let gap = 2u16;
        let g1_w = g1_inner.chars().count() as u16 + pad * 2;
        let g2_w = g2_inner.chars().count() as u16 + pad * 2;
        let g3_w = g3_inner.chars().count() as u16 + pad * 2;
        // Row 1: audio + volume left-justified; subs right-justified
        let row_y = area.y + 1;
        let g1_x = area.x;
        let g3_x = area.x + g1_w + gap;
        let g2_x = (area.x + area.width).saturating_sub(g2_w);
        let vol_x = g3_x + pad; // for click detection

        self.layout_seekbar_area = Rect { x: area.x, y: area.y,     width: area.width, height: 1 };
        self.layout_button_area  = Rect { x: btn_x,  y: area.y + 2, width: BTNS_W,     height: 1 };
        self.layout_tracks_area  = Rect { x: area.x, y: row_y,      width: area.width, height: 1 };
        self.layout_vol_area     = Rect { x: vol_x,  y: row_y,      width: g3_w,       height: 1 };
        self.layout_sub_area     = Rect { x: g1_x,   y: row_y,      width: g1_w,       height: 1 };
        self.layout_audio_area   = Rect { x: g2_x,   y: row_y,      width: g2_w,       height: 1 };

        let btn_row = Rect { x: area.x, y: area.y + 2, width: area.width, height: 1 };
        let _chip_bg = Style::default().bg(Color::Rgb(38, 38, 52));

        // Row 0 — Gauge with timestamp label
        let ratio = if runtime_ticks > 0 {
            (position_ticks as f64 / runtime_ticks as f64).clamp(0.0, 1.0)
        } else { 0.0 };
        let seek_label = Span::styled(
            format!("{} / {}", pos_str, dur_str),
            Style::default().fg(palette::TEXT).add_modifier(Modifier::BOLD),
        );
        f.render_widget(
            Gauge::default()
                .ratio(ratio)
                .label(seek_label)
                .gauge_style(Style::default().fg(palette::IRIS).bg(palette::OVERLAY))
                .use_unicode(true),
            Rect { x: area.x, y: area.y, width: area.width, height: 1 },
        );

        // Row 1 — subs + volume (left), audio (right)
        let text_style = btn_style;
        let p = " ".repeat(pad as usize);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(&p),
                Span::styled("Subs: ", text_style),
                Span::styled("\u{2261} ", Style::default().fg(palette::TEXT)),
                Span::styled(sub_label, text_style),
                Span::raw(&p),
            ])),
            Rect { x: g1_x, y: row_y, width: g1_w, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(&p),
                Span::styled("\u{266a} ", Style::default().fg(palette::IRIS)),
                Span::styled(audio_label, text_style),
                Span::raw(&p),
            ])),
            Rect { x: g2_x, y: row_y, width: g2_w, height: 1 },
        );

        let (vol_icon_style, vol_text_style) = if volume > 100 {
            (Style::default().fg(Color::Red), Style::default().fg(Color::Yellow))
        } else {
            (Style::default().fg(palette::YELLOW), text_style)
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(&p),
                Span::styled(format!("{vol_icon} "), vol_icon_style),
                Span::styled(vol_pct, vol_text_style),
                Span::raw(&p),
            ])),
            Rect { x: g3_x, y: row_y, width: g3_w, height: 1 },
        );

        // Row 2 — buttons, centered
        f.render_widget(
            Paragraph::new(Line::from(btn_spans)).alignment(Alignment::Center),
            btn_row,
        );
    }

    fn render_combined(&mut self, f: &mut ratatui::Frame, area: Rect) {
        self.home_rect = area;
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

        if self.playlist_card_view {
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

        let block = Block::default()
            .borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Span::styled("Queue",
                Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        self.layout_playlist_inner = inner;
        f.render_widget(block, area);

        if self.player_tab.items.is_empty() {
            f.render_widget(
                Paragraph::new("Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        let cursor = self.player_tab.playlist_cursor;

        // Split off one row for the btop-style rule header.
        let [header_line, table_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
        ]).areas(inner);

        // Rule header: ─ fill in OVERLAY, column labels in IRIS.
        // Column layout: [1, Min(10), 7, 10, 1] spacing 1 → fixed = 23, title = w-23.
        // Span breakdown (total = w):  2 + 7 + (w-30) + 1 + 7 + 1 + 10 + 2 = w
        {
            let label = Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD);
            let w = header_line.width as usize;
            let fill = w.saturating_sub(29);
            let header_spans = Line::from(vec![
                Span::raw("  "),
                Span::styled("Title",              label),
                Span::raw(" ".repeat(fill + 2)),
                Span::styled("Length",             label),
                Span::raw(" "),
                Span::styled("  Progress",         label),
            ]);
            f.render_widget(Paragraph::new(header_spans), header_line);
        }

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let stripe_bg = if i % 2 == 1 { palette::STRIPE } else { Color::Reset };
            let row_style = if i == current_idx && active {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD).bg(stripe_bg)
            } else if stripe_bg != Color::Reset {
                Style::default().fg(palette::WHITE).bg(stripe_bg)
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
            let length = if len_secs > 0 { format!("{:>7}", fmt_duration(len_secs)) } else { format!("{:>7}", "—") };
            let (pos_ticks, rt_ticks) = if i == current_idx && active {
                (live_pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            let progress_cell = if pos_ticks > 0 && rt_ticks > 0 {
                const BAR_W: usize = 8;
                let filled = (((pos_ticks as f64 / rt_ticks as f64) * BAR_W as f64)
                    .round() as usize)
                    .min(BAR_W);
                let bar_color = if i == current_idx && active { palette::FOAM } else { palette::YELLOW };
                Cell::from(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("━".repeat(filled),         Style::default().fg(bar_color)),
                    Span::styled("─".repeat(BAR_W - filled), Style::default().fg(palette::WHITE)),
                ])).style(Style::default())
            } else {
                Cell::from("")
            };

            Row::new([
                indicator,
                Cell::from(title),
                Cell::from(length),
                progress_cell,
                Cell::from(""),
            ]).style(row_style)
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .column_spacing(1)
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
        // Logo and Backdrop images live on the series item, not the episode.
        let urls: Vec<String> = types.iter().map(|t| {
            let src = match *t {
                "Logo" | "Backdrop" if !series_id.is_empty() => &series_id,
                _ => &item_id,
            };
            match *t {
                "Backdrop" => format!("{}/Items/{}/Images/Backdrop/0?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                "Logo"     => format!("{}/Items/{}/Images/Logo?maxHeight=400&quality=80&api_key={}", server_url, src, token),
                _          => format!("{}/Items/{}/Images/Primary?maxHeight=400&quality=80&api_key={}", server_url, src, token),
            }
        }).collect();
        let tx = self.card_image_tx.clone();
        let log = self.log.clone();
        std::thread::spawn(move || {
            let bytes = urls.iter().find_map(|url| {
                ureq::get(url).call().ok().and_then(|r| {
                    let mut buf = Vec::new();
                    r.into_reader().read_to_end(&mut buf).ok()?;
                    Some(buf)
                })
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

    fn evict_card_images(&mut self) {
        let mut valid: std::collections::HashSet<String> = self.player_tab.items.iter()
            .flat_map(|item| [format!("{}:A", item.id), format!("{}:S", item.id)])
            .collect();
        for lib in &self.libs {
            if let Some(lvl) = lib.nav_stack.last() {
                if let Some(item) = lvl.items.get(lvl.cursor) {
                    if item.item_type == "Movie" {
                        valid.insert(format!("{}:lib", item.id));
                    }
                }
            }
        }
        self.card_image_states.retain(|k, _| valid.contains(k));
        self.card_image_loading.retain(|k| valid.contains(k));
    }

    fn render_playlist_cards(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
        let center_h   = if cards_h < 12 { cards_h } else { ((cards_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        let side_h     = ((center_h as u32 * 4 / 5) as u16).max(3);
        let side_v_pad = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        // Below this width threshold, hide side cards and give all space to center.
        const SIDE_HIDE_W: u16 = 60;
        let show_sides = area.width >= SIDE_HIDE_W;

        // Width split: gap | side 30% | gap | center 40% | gap | side 30% | gap
        // Four equal gaps consumed from total width before distributing to panels.
        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w  = area.width.saturating_sub(GAP * 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = area.x + GAP;
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
                Rect { x: x_left,   y: area.y + side_v_pad, width: side_w,   height: side_h   },
                false,
            ),
            (
                Some(cursor),
                Rect { x: x_center, y: area.y + center_v_pad, width: center_w, height: center_h },
                true,
            ),
            (
                if show_sides && cursor + 1 < n { Some(cursor + 1) } else { None },
                Rect { x: x_right,  y: area.y + side_v_pad, width: side_w,   height: side_h   },
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
            self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);

            let ep_tag = if is_ep { format!("S{:02}E{:02}", season, episode) } else { String::new() };
            self.render_card_slot(f, *card_rect, *is_center, selected, now_playing,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played);
        }

        // Prefetch images for the three items before and after the cursor.
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

        // Count just below the center card, matching the home tab's title_y position.
        let count_y = (area.y + center_v_pad + center_h + 1).min(area.bottom().saturating_sub(1));
        if count_y < area.bottom() {
            f.render_widget(
                Paragraph::new(format!("{}/{}", cursor + 1, n))
                    .style(Style::default().fg(palette::MUTED))
                    .alignment(Alignment::Center),
                Rect { x: area.x, y: count_y, width: area.width, height: 1 },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_card_slot(
        &mut self,
        f: &mut ratatui::Frame,
        card_rect: Rect,
        is_center: bool,
        selected: bool,
        now_playing: bool,
        cache_key: &str,
        name: &str,
        series: &str,
        ep_tag: &str,
        runtime: i64,
        pos_ticks: i64,
        rt_ticks: i64,
        played: bool,
    ) {
        let border_fg = if selected { palette::IRIS } else { palette::WHITE };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_fg));
        let inner = block.inner(card_rect);
        f.render_widget(block, card_rect);

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

        // "Now Playing" header at top (2 rows).
        let mut header_rows = 0u16;
        if now_playing {
            put(f, inner.y, Paragraph::new("Now Playing")
                .style(Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center));
            let rule_w = (inner.width as u32 * 4 / 5) as usize;
            let pad    = (inner.width as usize).saturating_sub(rule_w) / 2;
            let rule   = format!("{}{}", " ".repeat(pad), "─".repeat(rule_w));
            put(f, inner.y + 1, Paragraph::new(rule).style(Style::default().fg(palette::IRIS)));
            header_rows = 2;
        }

        // Text rows pinned to the bottom, scaled to available height:
        //   >=10 rows inner: title(2) + series(1) + progress(2) = 5
        //   >=7  rows inner: title(2) + series(1) = 3  (drop progress bar)
        //   <7   rows inner: title(1) only = 1
        let text_rows = if inner.height >= 10 { 5u16 }
                        else if inner.height >= 7 { 3 }
                        else { 1 };
        let img_top    = inner.y + header_rows;
        let img_bottom = inner.bottom().saturating_sub(text_rows);
        let img_h      = img_bottom.saturating_sub(img_top);

        // Render image if available and there is space.
        let mut text_y = img_bottom; // text always pinned to bottom
        if img_h >= 2 {
            if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                if is_center {
                    let avail = ratatui::layout::Size { width: inner.width.saturating_sub(2), height: img_h };
                    let actual = state.size_for(ratatui_image::Resize::Fit(None), avail);
                    let img_x = inner.x + (inner.width.saturating_sub(actual.width)) / 2;
                    let img_y = img_top + (img_h.saturating_sub(actual.height)) / 2;
                    let img_rect = Rect { x: img_x, y: img_y, width: actual.width, height: actual.height };
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Fit(None)),
                        img_rect, state,
                    );
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
                }
                text_y = img_bottom;
            }
        }

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
                Span::styled("━".repeat(filled),         Style::default().fg(if now_playing { palette::FOAM } else { palette::YELLOW })),
                Span::styled("─".repeat(bar_w - filled), Style::default().fg(palette::WHITE)),
            ])));
            text_y += 1;
            put(f, text_y, Paragraph::new(format!("{} / {}", fmt_m(pos_ticks), fmt_m(rt_ticks)))
                .style(Style::default().fg(palette::MUTED))
                .alignment(Alignment::Center));
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
        let center_h   = if cards_h < 12 { cards_h } else { ((cards_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        // Place title and count just below the center card, with a one-line gap.
        let title_y  = (cards_area.y + center_v_pad + center_h + 1).min(area.bottom().saturating_sub(2));
        let gutter_y = (title_y + 1).min(area.bottom().saturating_sub(1));
        let side_h     = ((center_h as u32 * 4 / 5) as u16).max(3);
        let side_v_pad = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        const SIDE_HIDE_W: u16 = 60;
        let show_sides = cards_area.width >= SIDE_HIDE_W;

        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w  = cards_area.width.saturating_sub(GAP * 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = cards_area.x + GAP;
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
                Rect { x: x_left,   y: cards_area.y + side_v_pad, width: side_w,   height: side_h   },
                false,
            ),
            (
                Some(cursor),
                Rect { x: x_center, y: cards_area.y + center_v_pad, width: center_w, height: center_h },
                true,
            ),
            (
                if show_sides && cursor + 1 < n { Some(cursor + 1) } else { None },
                Rect { x: x_right,  y: cards_area.y + side_v_pad, width: side_w,   height: side_h   },
                false,
            ),
        ];

        let prefetch_start = cursor.saturating_sub(3);
        let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
        for pi in prefetch_start..=prefetch_end {
            let item = &items[pi];
            let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
            self.fetch_card_image(format!("{}:A", item_id.clone()), item_id.clone(), series_id.clone(), &["Primary", "Backdrop", "Logo"]);
            if pi != cursor {
                self.fetch_card_image(format!("{}:S", item_id), item_id, series_id, &["Logo", "Primary", "Backdrop"]);
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
                (format!("{}:A", item_id), &["Primary", "Backdrop", "Logo"])
            } else {
                (format!("{}:S", item_id), &["Logo", "Primary", "Backdrop"])
            };
            self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);

            self.render_card_slot(f, *card_rect, *is_center, selected, false,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played);
        }

        // Title, then count below it.
        let title_line = Line::from(
            Span::styled(sec_title, Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD))
        );
        f.render_widget(
            Paragraph::new(title_line).alignment(Alignment::Center),
            Rect { x: area.x, y: title_y, width: area.width, height: 1 },
        );
        if area.height >= 2 {
            f.render_widget(
                Paragraph::new(format!("{}/{}", cursor + 1, n))
                    .style(Style::default().fg(palette::MUTED))
                    .alignment(Alignment::Center),
                Rect { x: area.x, y: gutter_y, width: area.width, height: 1 },
            );
        }
    }

    fn render_home_panel(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let home_focused = true;
        let n_sections = 1 + self.home.latest.len();
        if n_sections == 0 { return; }

        let visible = if (n_sections as u16) * HOME_MIN_SECTION_H <= area.height {
            n_sections
        } else {
            ((area.height / HOME_MIN_SECTION_H) as usize).max(1)
        };

        let max_offset = n_sections.saturating_sub(visible);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
        let offset = self.home_panel_section_offset;
        let render_count = visible.min(n_sections - offset);

        let scrollable = n_sections > visible;
        let layout_area = if scrollable && area.width > 2 {
            Rect { width: area.width - 2, ..area }
        } else {
            area
        };

        let constraints: Vec<Constraint> = (0..render_count)
            .map(|_| Constraint::Ratio(1, render_count as u32))
            .collect();
        let section_areas = Layout::vertical(constraints).split(layout_area);

        // layout_section_areas is indexed by logical section; off-screen sections get Rect::default()
        let mut areas: Vec<Rect> = vec![Rect::default(); n_sections];
        for i in 0..render_count {
            areas[offset + i] = section_areas[i];
        }
        self.layout_section_areas = areas;

        let mut scrolls = vec![0usize; n_sections];

        if offset == 0 && render_count > 0 {
            let cont_focused = home_focused && self.home.section == 0;
            scrolls[0] = self.render_home_section(
                f, section_areas[0], "Continue Watching",
                &self.home.continue_items, self.home.continue_cursor, cont_focused, true,
            );
        }

        // Collect to avoid holding &self.home.latest across render calls that need &mut self
        let latest_data: Vec<(String, Vec<MediaItem>, usize)> = self.home.latest
            .iter()
            .map(|(t, _, items, c)| (t.clone(), items.clone(), *c))
            .collect();

        for render_pos in 0..render_count {
            let logical = offset + render_pos;
            if logical == 0 { continue; }
            let latest_idx = logical - 1;
            if let Some((title, items, cursor)) = latest_data.get(latest_idx) {
                let focused = home_focused && self.home.section == logical;
                scrolls[logical] = self.render_home_section(
                    f, section_areas[render_pos], title,
                    items, *cursor, focused, false,
                );
            }
        }
        self.layout_home_scrolls = scrolls;

        if scrollable {
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None),
                area,
                &mut sb_state,
            );
        }
    }

    fn render_home_section(
        &self, f: &mut ratatui::Frame, area: Rect,
        title: &str, items: &[MediaItem], cursor: usize, focused: bool,
        continue_style: bool,
    ) -> usize {
        let border_style = if focused { Style::default().fg(palette::IRIS) } else { Style::default().fg(palette::OVERLAY) };
        let title_style = if focused {
            Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)
        };
        let block = Block::default()
            .borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(Span::styled(title, title_style))
            .title_alignment(Alignment::Center);
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
            } else { li }
        }).collect();

        let mut state = ListState::default();
        if focused { state.select(Some(cursor)); }
        f.render_stateful_widget(List::new(list_items), inner, &mut state);
        state.offset()
    }

    fn render_library(&mut self, f: &mut ratatui::Frame, area: Rect, lib_idx: usize) {
        let is_loading = self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(true);
        if is_loading && self.libs[lib_idx].search.is_none() {
            let lib_name = self.libs[lib_idx].library.name.clone();
            let block = Block::default()
                .borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS))
                .title(Span::styled(lib_name, Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)))
                .title_alignment(Alignment::Center);
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

        if let Some(s) = &self.libs[lib_idx].search {
            self.layout_breadcrumbs.clear();
            let display = format!("{}█", s.query);
            let block = Block::default()
                .borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS))
                .title(Span::styled(display, Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)))
                .title_alignment(Alignment::Center);
            let inner = block.inner(area);
            f.render_widget(block, area);
            self.render_library_table(f, inner, lib_idx);
            return;
        }

        // Build breadcrumb spans and record click regions
        let lib = &self.libs[lib_idx];
        let skip = if lib.nav_stack.first().map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
        // crumb 0 = library name (target depth 1), then each nav_stack part above skip
        let mut crumb_names: Vec<(&str, usize)> = vec![(&lib.library.name, 1)];
        for (i, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
            crumb_names.push((lvl.title.as_str(), i + 1));
        }

        let sep = " > ";
        let crumb_style = Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD);
        let is_deep = crumb_names.len() > 1;

        // Compute total title width so we can derive the centered x offset for click regions.
        let total_title_w: u16 = crumb_names.iter().enumerate().map(|(ci, (name, _))| {
            let w = name.chars().count() as u16;
            if ci + 1 < crumb_names.len() { w + sep.len() as u16 } else { w }
        }).sum();
        let title_x = area.x + area.width.saturating_sub(total_title_w) / 2;

        let mut x = title_x;
        let mut crumb_spans: Vec<Span> = Vec::new();
        let mut new_breadcrumbs: Vec<(u16, u16, usize)> = Vec::new();
        for (ci, (name, target_depth)) in crumb_names.iter().enumerate() {
            let is_last = ci + 1 == crumb_names.len();
            let w = name.chars().count() as u16;
            new_breadcrumbs.push((x, x + w, *target_depth));
            crumb_spans.push(Span::styled(*name, crumb_style));
            x += w;
            if !is_last {
                crumb_spans.push(Span::styled(sep, crumb_style));
                x += sep.len() as u16;
            }
        }
        self.layout_breadcrumbs = if is_deep { new_breadcrumbs } else { Vec::new() };

        let block = Block::default()
            .borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Line::from(crumb_spans))
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        f.render_widget(block, area);
        self.render_library_table(f, inner, lib_idx);
    }

    fn render_library_table(&mut self, f: &mut ratatui::Frame, area: Rect, lib_idx: usize) {
        self.layout_lib_table_area = area;
        const LIB_IMG_W: u16 = 20;
        const LIB_EP_IMG_W: u16 = 32;

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

        // Row heights: 2 base (+ wrapped overview for episodes) + 1 separator.
        // Selected row also gets +2 padding (top + bottom).
        let episode_content_w = area.width.saturating_sub(1) as usize;
        let episode_selected_content_w = area.width.saturating_sub(1 + LIB_EP_IMG_W) as usize;
        let all_heights: Vec<u16> = display_items.iter().enumerate().map(|(i, (_, item))| {
            let base: u16 = if item.item_type == "Episode" {
                let ew = if i == cursor { episode_selected_content_w } else { episode_content_w };
                let ov_lines = if item.overview.is_empty() { 0 }
                    else { wrap(&item.overview, ew.max(1)).len().min(4) as u16 };
                let seekbar: u16 = if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 { 2 } else { 0 };
                2 + ov_lines + seekbar
            } else { 2 };
            let padding: u16 = if i == cursor { 2 } else { 0 };
            base + padding + 1 // +1 for separator line at bottom
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
        if let Some((_, item)) = display_items.get(cursor) {
            let cache_key = format!("{}:lib", item.id);
            match item.item_type.as_str() {
                "Movie" | "Series" => {
                    self.fetch_card_image(cache_key, item.id.clone(), String::new(), &["Logo", "Primary"]);
                }
                "Season" => {
                    self.fetch_card_image(cache_key, item.id.clone(), item.series_id.clone(), &["Logo", "Primary"]);
                }
                "Episode" => {
                    self.fetch_card_image(cache_key, item.id.clone(), String::new(), &["Primary"]);
                }

                _ => {}
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
            let show_img = selected && matches!(item.item_type.as_str(), "Movie" | "Series" | "Season" | "Episode");
            let row_rect = Rect { x: area.x, y: row_y, width: area.width, height: row_h };

            // Content area excludes the separator line at the bottom of the row.
            // For selected rows it is further inset by 1 top and 1 bottom for padding.
            let content_area = Rect { height: row_h.saturating_sub(1), ..row_rect };
            let padded_area = if selected {
                Rect { y: content_area.y + 1, height: content_area.height.saturating_sub(2), ..content_area }
            } else { content_area };

            // Compute actual image size first so we can size the column correctly.
            // Image sits at the far right; text fills everything to its left.
            let cache_key = format!("{}:lib", item.id);
            let img_actual = if show_img {
                if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                    let (img_w, img_h) = if item.item_type == "Episode" {
                        (LIB_EP_IMG_W, 4u16.min(padded_area.height))
                    } else {
                        (LIB_IMG_W, padded_area.height)
                    };
                    let avail = ratatui::layout::Size { width: img_w, height: img_h };
                    Some(state.size_for(ratatui_image::Resize::Fit(None), avail))
                } else { None }
            } else { None };

            // Split padded_area: indicator | text | gap | [image]
            let (ind_rect, text_rect, img_rect_opt) = if let Some(actual) = img_actual {
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

            let text_color = palette::TEXT;

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

            // Build metadata line (line 2)
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
                    // Movie and other non-folder types
                    let mut spans: Vec<Span> = Vec::new();
                    if item.played {
                        spans.push(Span::styled("\u{f00c} ", Style::default().fg(palette::PINE)));
                    }
                    let mut parts: Vec<String> = Vec::new();
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    let dur_s = item.runtime_ticks / crate::api::TICKS_PER_SECOND;
                    if dur_s > 0 {
                        let h = dur_s / 3600; let m = (dur_s % 3600) / 60;
                        parts.push(if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") });
                    }
                    if !parts.is_empty() {
                        spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
                    }
                    if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                        spans.push(Span::styled(format!("  {pct}%"), Style::default().fg(palette::SUBTLE)));
                    }
                    Line::from(spans)
                }
            };

            // Split text_rect vertically into lines
            let is_ep_in_progress = item.item_type == "Episode"
                && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0;
            let overview_lines: Vec<String> = if item.item_type == "Episode" && !item.overview.is_empty() {
                wrap(&item.overview, content_w.max(1))
                    .into_iter()
                    .take(4)
                    .map(|s| s.into_owned())
                    .collect()
            } else { Vec::new() };
            let seekbar_extra: usize = if is_ep_in_progress { 2 } else { 0 }; // spacer + bar
            let line_count = (2 + overview_lines.len() + seekbar_extra).min(text_rect.height as usize);
            if line_count == 0 { continue; }
            let constraints: Vec<Constraint> = (0..line_count).map(|_| Constraint::Length(1)).collect();
            let line_rects = Layout::vertical(constraints).split(text_rect);

            f.render_widget(
                Paragraph::new(Line::from(Span::styled(title_display, Style::default().fg(text_color)))),
                line_rects[0],
            );
            if line_count >= 2 {
                f.render_widget(Paragraph::new(meta_line), line_rects[1]);
            }
            for (j, ov_line) in overview_lines.iter().enumerate() {
                let idx = 2 + j;
                if idx >= line_count { break; }
                f.render_widget(
                    Paragraph::new(Span::styled(ov_line.as_str(), Style::default().fg(palette::MUTED))),
                    line_rects[idx],
                );
            }
            // Seekbar: spacer at line_count-2, bar at line_count-1
            if is_ep_in_progress && line_count >= 2 + seekbar_extra {
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

fn item_text_and_style(item: &MediaItem, selected: bool) -> (String, Style) {
    if item.is_folder {
        let text = if item.unplayed_item_count > 0 {
            format!("{} [{}]", item.display_name(), item.unplayed_item_count)
        } else {
            item.display_name()
        };
        // Series are content, not navigation folders — don't colour them green
        let is_series = item.item_type == "Series";
        let style = if selected { Style::default() }
            else if is_series  { Style::default().fg(palette::TEXT) }
            else               { Style::default().fg(palette::PINE) };
        return (text, style);
    }
    let mut suffix = String::new();
    if item.runtime_ticks > 0 {
        let s = item.runtime_seconds();
        let h = (s / 3600.0) as u64;
        let m = ((s % 3600.0) / 60.0) as u64;
        let dur = if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") };
        if item.playback_position_ticks > 0 {
            let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
            suffix = format!("  [{pct}%/{dur}]");
        } else {
            suffix = format!("  [{dur}]");
        }
    }
    let text = format!("{}{}", item.display_name(), suffix);
    let style = if selected { Style::default() }
        else if item.playback_position_ticks > 0 { Style::default().fg(palette::YELLOW) }
        else { Style::default().fg(palette::TEXT) };
    (text, style)
}

fn fmt_item_wrapped(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, style) = item_text_and_style(item, selected);
    let w = width.max(1);
    let lines: Vec<Line<'static>> = wrap(&full_text, w)
        .into_iter()
        .map(|s| Line::from(Span::styled(s.into_owned(), style)))
        .collect();
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

fn highlight_style(item: &MediaItem) -> Style {
    if item.is_folder && item.item_type != "Series" { Style::default().fg(palette::BASE).bg(palette::PINE) }
    else if item.playback_position_ticks > 0 { Style::default().fg(palette::BASE).bg(palette::YELLOW) }
    else                    { Style::default().fg(palette::TEXT).bg(palette::HIGHLIGHT_MED) }
}

fn fmt_item_continue(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, _) = item_text_and_style(item, selected);
    let w = width.max(1);
    let span_style = if selected { Style::default() }
        else if item.playback_position_ticks > 0 { Style::default().fg(palette::YELLOW) }
        else { Style::default().fg(palette::WHITE) };
    let mut lines: Vec<Line<'static>> = wrap(&full_text, w)
        .into_iter()
        .map(|s| Line::from(Span::styled(s.into_owned(), span_style)))
        .collect();
    if !selected && item.played {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(" ✓", Style::default().fg(palette::IRIS)));
        }
    }
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

fn highlight_style_continue(_item: &MediaItem) -> Style {
    Style::default().fg(palette::TEXT).bg(palette::HIGHLIGHT_MED)
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
            series_id: String::new(), series_name: String::new(),
            index_number: 0, parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
            production_year: 0, end_year: 0, overview: String::new(),
            premiere_date: String::new(), total_count: 0,
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
        assert_eq!(style.fg, Some(palette::TEXT));
    }

    #[test]
    fn item_text_played_movie_uses_default_color() {
        let mut item = make_item("Inception", "Movie");
        item.played = true;
        let (_, style) = item_text_and_style(&item, false);
        assert_eq!(style.fg, Some(palette::TEXT));
    }

    #[test]
    fn item_text_in_progress_shows_percentage() {
        let mut item = make_item("Inception", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200; // 2 hours
        item.playback_position_ticks = TICKS_PER_SECOND * 3600; // 1 hour in → 50%
        let (text, style) = item_text_and_style(&item, false);
        assert!(text.contains("50%"), "expected percentage in: {text}");
        assert_eq!(style.fg, Some(palette::YELLOW));
    }

    #[test]
    fn item_text_played_but_in_progress_shows_percentage() {
        let mut item = make_item("Inception", "Movie");
        item.runtime_ticks = TICKS_PER_SECOND * 7200;
        item.playback_position_ticks = TICKS_PER_SECOND * 3600;
        item.played = true;
        let (text, _) = item_text_and_style(&item, false);
        assert!(text.contains("50%"), "expected percentage in: {text}");
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
        // Series should use TEXT colour, not the PINE folder colour
        assert_eq!(style.fg, Some(palette::TEXT));
    }

    #[test]
    fn item_text_nav_folder_is_green() {
        let mut item = make_item("Folder", "Folder");
        item.is_folder = true;
        let (_, style) = item_text_and_style(&item, false);
        assert_eq!(style.fg, Some(palette::PINE));
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
            layout_audio_area: ratatui::layout::Rect::default(),
            confirm_remove_idx: None,
            confirm_clear_playlist: false,
            next_up_item: None,
            playlist_card_view: false,
            home_card_view: false,
            last_played_item_id: None,
            layout_carousel_slots: [(None, ratatui::layout::Rect::default()); 3],
            last_carousel_click_slot: None,
            last_carousel_click_time: std::time::Instant::now(),
            card_image_states: std::collections::HashMap::new(),
            card_image_loading: std::collections::HashSet::new(),
            card_image_tx,
            card_image_rx,
            image_picker: None,
            show_help: false,
            help_scroll: 0,
            show_log_tab: false,
            context_menu: None,
            context_menu_rect: None,
            lib_tx,
            lib_rx,
            force_clear: false,
            last_lib_tab: 2,
            lib_picker_open: false,
            lib_picker_cursor: 0,
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
        // panel_h = 24 - 3 = 21; visible = 21/6 = 3; sections=5 => offset must move
        app.terminal_height = 24;
        app.home.latest = sections(4); // 5 total sections
        app.home.section = 4; // last section
        app.home_panel_section_offset = 0;
        app.ensure_home_section_visible();
        // offset + visible > section, so offset = section + 1 - visible = 4 + 1 - 3 = 2
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
