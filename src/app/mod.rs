mod action;
mod actions;
mod actions_navigation;
mod app_audiobookshelf_service_completion;
mod app_emby_service_completion;
mod app_struct;
mod audio_subtitle_actions;
mod audiobookshelf_browse_actions;
mod audiobookshelf_service_actions;
mod bootstrap;
mod browse_level_actions;
mod cast_actions;
mod cast_reattach;
mod cast_status_actions;
mod construct;
mod consume_quit_actions;
mod context_menu_actions;
mod cw_library_tab_actions;
mod daemon_restart;
mod emby_service_actions;
mod feed_actions;
mod feed_parse;
mod feed_parse_date;
mod feed_tab_actions;
mod feeds_manage_actions;
mod home_actions;
pub(crate) mod images;
mod input;
mod input_browse_dispatch;
mod input_confirm_keys;
mod input_context_menu;
mod input_daemon_lost_keys;
mod input_feed_tab_keys;
mod input_feeds_manage_keys;
mod input_lib_keys;
mod input_mouse;
mod input_mouse_dispatch;
mod input_mouse_panels;
mod input_playlist_keys;
mod input_queue_keys;
mod input_remote_reanchor;
mod input_resolver;
mod input_search_sidebar_keys;
mod input_settings_keys;
pub(crate) mod layout;
mod lib_cursor_actions;
mod lib_event_actions;
mod library_browse_actions;
mod library_column_width;
mod library_load_actions;
mod library_position_state;
mod library_route;
mod library_search_actions;
mod music_actions;
mod music_grouping;
mod notify_actions;
pub(crate) mod palette;
mod panel_focus_state;
mod panel_targets;
mod playback_target;
mod playback_target_cast;
mod playback_target_local;
mod playback_target_remote;
mod player_event;
mod queue_actions;
mod queue_column_width;
mod queue_scope;
mod remote_slot_state;
pub mod render;
mod render_cadence;
mod resize;
mod run_loop_drains;
mod run_loop_events;
mod search_sidebar;
mod service_startup;
mod services_settings;
mod session_command_actions;
mod session_connect;
mod session_switch;
mod settings;
mod shared_sync;
mod shuffle_folder_actions;
mod types_audiobookshelf_browse;
mod types_browse;
mod types_cast;
mod types_confirm;
mod types_context_menu;
mod types_daemon_lost;
mod types_events;
mod types_feed;
mod types_feed_tab;
mod types_feeds_manage;
mod types_library_tab;
mod types_playback;
mod types_player_tab;
mod types_settings;
mod types_tab_selection;
pub(crate) mod ui_util;
mod visualizer;
mod visualizer_worker;
mod ws_event_actions;

pub use self::app_struct::App;
mod app_init;
use self::app_init::AppInit;
use self::bootstrap::{bootstrap_local_daemon_queue, bootstrap_unified_queue};
use self::notify_actions::ToastSeverity;
use self::resize::spawn_resize_worker;
use self::search_sidebar::SearchDrainOutcome;
use self::types_browse::{
    restore_library_position, AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel,
    LibSearch, SeriesDetail,
};
use self::types_confirm::{ConfirmAction, ConfirmModal};
use self::types_context_menu::{
    ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry, LibraryRoutePopup,
    LibraryRouteStage, MultiSelectKind, MultiSelectPopup,
};
use self::types_daemon_lost::DaemonLostModal;
use self::types_events::{LibEvent, ReconciliationCommand, SessionEvent};
use self::types_feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use self::types_library_tab::LibraryTab;
#[cfg(test)]
use self::types_playback::HomePane;
use self::types_playback::{
    CastPlaybackTarget, HomeLatestSource, LocalPlaybackTarget, PendingQueueAction, PlaybackState,
    PlaybackTarget, QueueScope, QueueScopeResolution, RemotePlaybackTarget, RemoteReanchorPopup,
    RemoteSlotState, SuspendedLocalSession, UndoEntry,
};
use self::types_player_tab::PlayerTab;
use self::types_settings::{PanelFocus, PanelMode, SettingKey, SETTING_SECTIONS};
#[cfg(test)]
use self::types_settings::{ServiceEntry, SettingsDestination, SERVICE_ENTRIES};
use self::types_tab_selection::TabSelection;
use mbv_core::api::EmbyClient;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
// Set only by SIGHUP or stdin POLLHUP (terminal vanished). Never set by q/SIGTERM.
// The watchdog's forced exit arms only on this flag so clean q-quits are never raced.
static TERMINAL_GONE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
type DirectConnectFn = fn(
    &mbv_core::remote_player::DaemonEndpoint,
) -> Result<
    (
        mbv_core::remote_player::RemotePlayer,
        mpsc::Receiver<PlayerEvent>,
    ),
    String,
>;

#[cfg(test)]
static DIRECT_CONNECT_OVERRIDE: Mutex<Option<DirectConnectFn>> = Mutex::new(None);

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

// Test seam for `App::connect_cast_receiver`'s resolve-and-connect step
// (7.3/7.5), mirroring the overrides above: lets tests substitute a fake
// worker instead of a real mDNS browse + `CastClient::connect`, without
// making the reattach/attach-on-selection call sites themselves
// test-aware.
#[cfg(test)]
type CastConnectFn = fn(&str, Duration) -> Result<mpsc::Sender<types_cast::CastJob>, String>;
#[cfg(test)]
static CAST_CONNECT_OVERRIDE: Mutex<Option<CastConnectFn>> = Mutex::new(None);
#[cfg(test)]
static CAST_CONNECT_TEST_LOCK: Mutex<()> = Mutex::new(());

pub(super) const LEFT_WIDTH_DEFAULT: u16 = 40;
pub(super) const LEFT_WIDTH_STEP: u16 = 5;
/// The single wide/narrow breakpoint. Minimum list-pane / Home-pane width at
/// which the view switches to a two-column layout. Every screen and
/// arrangement reads this one constant instead of testing width itself; the
/// library list's column count derives from it (`library_column_count`), not
/// the other way around.
pub(super) const TWO_COLUMN_THRESHOLD: u16 = 82;
/// The narrow-terminal breakpoint below which the Power View uses the
/// two-state "mini view" (`x` toggles library-only <-> queue-only) instead of
/// the three-state both/queue-only/library-only cycle. Independent of and
/// unrelated to `TWO_COLUMN_THRESHOLD` (82), which governs the library
/// panel's internal list-column layout (see design.md).
pub(super) const MINI_VIEW_THRESHOLD: u16 = 80;
/// Left margin for the tab row. The control pill used to live here (hence
/// the old, larger reservation); it now renders in the status bar (see
/// `render_status_bar`) and the tabs are left-aligned flush with the left
/// edge instead.
pub(super) const TABBAR_LEFT_RESERVE: u16 = 0;

extern "C" fn handle_quit_signal(signum: i32) {
    let name = match signum {
        1 => "SIGHUP",
        15 => "SIGTERM",
        _ => "unknown",
    };
    // SAFETY: log::info is not async-signal-safe, but we only reach this
    // from SIGTERM/SIGHUP where the process is about to exit anyway;
    // a worst-case torn write is acceptable for diagnostics.
    eprintln!("mbv: received {name} (signal {signum}), requesting quit");
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

#[cfg(test)]
use mbv_core::api::EmbyItem;
#[cfg(test)]
use mbv_core::playback_queue::RemoveSlotResult;
#[cfg(test)]
use mbv_core::player::PlayerEvent;

const PAGE_SIZE: usize = 100;
const PREFETCH_AHEAD: usize = 25;
const SESSIONS_PANEL_W: u16 = 40;
const HELP_PANEL_W: u16 = 40;
const SETTINGS_PANEL_W: u16 = 40;
const PLAYLISTS_PANEL_W: u16 = 40;
const SEARCH_PANEL_W: u16 = 40;
impl App {
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode. The
        // halfblock picker backs the dimmed-backdrop fallback: while a modal
        // dims the backdrop, images re-encode to halfblocks so the dim
        // applies uniformly (#451).
        self.image_picker = Some(self.build_image_picker());
        self.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());

        // Don't clobber a still-live flash message (e.g. try_auto_reconnect's
        // outcome, set during App::new) -- only show "Loading..." if there's
        // no pending flash, mirroring the render loop's own expiry check.
        let has_live_flash = self.status_expires.is_some_and(|t| t > Instant::now());
        if !has_live_flash {
            self.status = self
                .emby_client()
                .map(|_| "Loading...".into())
                .unwrap_or_else(|| service_startup::startup_status(self.emby_runtime.state).into());
        }
        self.home_loading = true;
        terminal.draw(|f| self.render(f))?;

        // Only start the configured Remote Service after the first TUI frame
        // has been rendered. The selected Player owner and UI therefore never
        // wait for Emby setup, authentication, or connectivity.
        if let Some((config, generation)) = self.emby_startup_request.take() {
            self.emby_startup_rx = Some(service_startup::start(config, generation));
        }
        if let Some((config, generation)) = self.audiobookshelf_startup_request.take() {
            self.audiobookshelf_startup_rx = Some(service_startup::start_audiobookshelf(
                config,
                generation,
                service_startup::AudiobookshelfCompletionKind::Startup,
            ));
        }

        // Auto-fetch configured feeds asynchronously so the Feeds tab and the
        // Home "Feeds" pill are populated shortly after startup instead
        // of staying empty until the user presses the manual refresh key.
        self.start_feed_fetch();

        if let Some(client) = self.emby_client() {
            client.lock().unwrap().register_capabilities();
        }

        // Home populates from whatever Services are available at this moment;
        // Emby's independent startup connection merges its portion in when it
        // completes (apply_emby_bootstrap), so no Emby gate is needed here
        // (#543 Part 1).
        match self.fetch_home() {
            Ok(()) => {
                let has_live_flash = self.status_expires.is_some_and(|t| t > Instant::now());
                if !has_live_flash {
                    self.status.clear();
                }
            }
            Err(e) => self.flash(format!("Couldn't load home: {e}"), ToastSeverity::Warning),
        }
        self.home_loading = false;
        self.maybe_restore_queue_state();

        // Initialize idle feed if configured
        if self.config.lock().unwrap().idle_feed_rss_url.is_empty() {
            // No RSS URL configured, skip idle feed
        } else {
            let (items_tx, items_rx) = std::sync::mpsc::channel();
            self.idle_feed = Some(IdleFeed {
                items: Vec::new(),
                current_index: 0,
                last_rotation: Instant::now(),
                last_fetch: Instant::now(),
                items_tx,
                items_rx,
            });
            self.spawn_idle_feed_fetch();
        }

        terminal.draw(|f| self.render(f))?;

        install_signal_handlers();
        let quit_timeout = Duration::from_secs(self.config.lock().unwrap().quit_timeout_secs);
        start_quit_watchdog(self.player.quit_handle(), quit_timeout);

        let mut last_render = Instant::now() - Duration::from_secs(2);

        'outer: loop {
            let mut had_events = false;
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }

            if let Some(worker) = self.emby_startup_rx.take() {
                match worker.rx.try_recv() {
                    Ok(completion) => {
                        had_events = true;
                        self.apply_emby_completion(completion);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.emby_startup_rx = Some(worker);
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.handle_emby_startup_worker_disconnect(worker.generation);
                        had_events = true;
                    }
                }
            }
            if let Some(rx) = self.emby_setup_rx.take() {
                match rx.try_recv() {
                    Ok(completion) => {
                        had_events = true;
                        self.apply_emby_setup_completion(completion);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.emby_setup_rx = Some(rx);
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.handle_emby_setup_worker_disconnect();
                        had_events = true;
                    }
                }
            }
            had_events |= self.drain_audiobookshelf_events();
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

            had_events |= self.maybe_flush_search_debounce();

            had_events |= self.drain_search_results();

            had_events |= self.drain_session_events();

            had_events |= self.drain_cast_events();

            had_events |= self.drain_shared_events();

            had_events |= self.drain_feed_tab_results();

            had_events |= self.drain_feed_add_results();

            while let Ok((item_id, img_opt)) = self.card_image_rx.try_recv() {
                had_events = true;
                self.card_image_loading.remove(&item_id);
                self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
                let entry = self.build_cached_image(&item_id, img_opt);
                if entry.img.is_some() {
                    self.image_lru.retain(|k| k != &item_id);
                    self.image_lru.push_back(item_id.clone());
                    while self.image_lru.len() > self.image_cache_size_total {
                        if let Some(evict) = self.image_lru.pop_front() {
                            self.card_image_states.remove(&evict);
                        }
                    }
                }
                self.card_image_states.insert(item_id, entry);
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
                // Responses are tagged with the per-suffix mem-key
                // ("bare@suffix"); route them into the matching protocol of
                // the bare-key cache entry.
                if let Some((bare_key, suffix)) = key.rsplit_once('@') {
                    if let Some(entry) = self.card_image_states.get_mut(bare_key) {
                        if let Some(state) = entry.protocols.get_mut(suffix) {
                            state.update_resized_protocol(response);
                        }
                    }
                }
            }

            while let Ok(ev) = self.ws_rx.try_recv() {
                had_events = true;
                self.handle_ws_event(ev);
            }

            while let Ok(ev) = self.audiobookshelf_socket_rx.try_recv() {
                had_events = true;
                self.handle_audiobookshelf_socket_event(ev);
            }

            // Drain idle feed items
            if let Some(ref mut idle_feed) = self.idle_feed {
                while let Ok(items) = idle_feed.items_rx.try_recv() {
                    had_events = true;
                    idle_feed.items = items;
                    idle_feed.current_index = 0;
                }
                // Re-fetch every 30 minutes
                if idle_feed.last_fetch.elapsed() >= Duration::from_secs(1800) {
                    idle_feed.last_fetch = Instant::now();
                    self.spawn_idle_feed_fetch();
                }
            }

            self.sync_visualizer();

            if let Some(at) = self.settings_save_at {
                if Instant::now() >= at {
                    let cfg = self.config.lock().unwrap().clone();
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

            // Periodic status poll while attached to a cast target (6.1). The
            // keep-alive heartbeat this poll's background thread sends is not
            // optional -- see `CAST_STATUS_POLL_INTERVAL`'s doc comment.
            if self.cast_attachment.is_some()
                && self.last_cast_poll.elapsed()
                    >= self::cast_status_actions::CAST_STATUS_POLL_INTERVAL
                && !self.cast_status_loading
            {
                self.spawn_cast_status_poll();
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
                if let Some(client) = self.emby_snapshot() {
                    std::thread::spawn(move || client.register_capabilities());
                }
                self.last_capabilities = Instant::now();
            }

            // Break instead of propagating I/O errors: when the terminal closes
            // (SIGHUP), poll/read fail because the fd is gone. Breaking lets the
            // post-loop cleanup run (player.stop + join) so the mpv window closes.
            let poll_timeout = if self.visualizer.is_some() {
                Duration::from_millis(8)
            } else {
                Duration::from_millis(50)
            };
            let poll_ready = match event::poll(poll_timeout) {
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
                    // only. Fixes a resize-corruption bug: ratatui's diffing
                    // alone left stale content on-screen after a size
                    // change, since raw image escape sequences aren't
                    // tracked in its buffer.
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

            self.expire_music_grouping_candidates();
            self.sync_volume_from_player();
            self.flush_library_position_if_idle();

            // Advance idle feed rotation
            self.advance_idle_feed_rotation();

            // See `render_interval`'s doc comment for the fast/slow cadence rules.
            let render_interval = self.render_interval();
            if self.wants_terminal_render(had_events, last_render, render_interval) {
                if self.last_throbber_advance.elapsed() >= std::time::Duration::from_millis(300) {
                    let playback = self.effective_playback_state();
                    if playback.active && !self.playback_transport_paused() {
                        self.now_playing_throbber_index =
                            self.now_playing_throbber_index.wrapping_add(1);
                    }
                    self.last_throbber_advance = std::time::Instant::now();
                }
                if self.force_clear {
                    self.force_clear = false;
                    if let Err(e) = terminal.clear() {
                        log::error!(target: "run_loop", "terminal.clear() failed: {e:?} (kind={:?})", e.kind());
                        return Err(e.into());
                    }
                }
                if self.visualizer.is_some() {
                    self.sync_visualizer();
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
                                            // Printed only after the terminal is restored (task 7.2): anything
                                            // written while still in the alternate screen would never be
                                            // visible once it's left.
        if let Some(msg) = self.pending_exit_message.take() {
            println!("{msg}");
        }
        Ok(())
    }

    pub(super) fn spawn_search_sidebar_query(&self, client: EmbyClient, query: String) {
        let tx = self.search_tx.clone();
        std::thread::spawn(move || {
            let result = client.search_items(&query, 100);
            let _ = tx.send((query, result));
        });
    }

    pub(super) fn drain_search_sidebar_results(&mut self, outcome: &mut SearchDrainOutcome) {
        while let Ok((query, result)) = self.search_rx.try_recv() {
            outcome.received += 1;
            if let Some(sidebar) = self.search_sidebar.as_mut() {
                sidebar.apply_drain(&query, result);
            }
        }
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

include!("app_test_modules.rs");
