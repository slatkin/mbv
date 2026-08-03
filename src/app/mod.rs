mod action;
mod actions;
mod app_struct;
mod artist_header_actions;
mod audio_subtitle_actions;
mod bootstrap;
mod browse_level_actions;
mod construct;
mod consume_quit_actions;
mod context_menu_actions;
mod daemon_restart;
mod feed_actions;
mod feed_parse;
pub(crate) mod images;
mod input;
mod input_confirm_keys;
mod input_context_menu;
mod input_daemon_lost_keys;
mod input_home_search_keys;
mod input_lib_power_keys;
mod input_mouse;
mod input_mouse_dispatch;
mod input_mouse_panels;
mod input_playlist_keys;
mod input_queue_keys;
mod input_resolver;
mod input_settings_keys;
pub(crate) mod layout;
mod lib_cursor_actions;
mod lib_event_actions;
mod library_browse_actions;
mod library_load_actions;
mod library_position_state;
mod library_route;
mod library_search_actions;
mod music_actions;
mod music_grouping;
mod notify_actions;
pub(crate) mod palette;
mod panel_focus_state;
mod playback_target;
mod playback_target_local;
mod playback_target_remote;
mod player_event;
mod power_cw_library_tab_actions;
mod power_home_actions;
mod queue_actions;
mod queue_column_width;
mod queue_scope;
mod remote_slot_state;
pub mod render;
mod render_cadence;
mod resize;
mod run_loop_drains;
mod run_loop_events;
mod search;
mod session_command_actions;
mod session_connect;
mod settings;
mod shuffle_folder_actions;
mod types_browse;
mod types_confirm;
mod types_context_menu;
mod types_daemon_lost;
mod types_events;
mod types_feed;
mod types_library_tab;
mod types_playback;
mod types_player_tab;
mod types_settings;
pub(crate) mod ui_util;
mod visualizer;
mod ws_event_actions;

pub use self::app_struct::App;
use self::app_struct::AppInit;
use self::bootstrap::bootstrap_local_daemon_queue;
use self::resize::spawn_resize_worker;
use self::search::SearchSubsystem;
use self::types_browse::{
    restore_library_position, AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel,
    LibSearch, SeriesDetail,
};
use self::types_confirm::{ConfirmAction, ConfirmModal};
use self::types_context_menu::{
    ContextAction, ContextMenu, ContextMenuEntry, LibraryRoutePopup, LibraryRouteStage,
    MultiSelectKind, MultiSelectPopup,
};
use self::types_daemon_lost::DaemonLostModal;
use self::types_events::{LibEvent, SessionEvent};
use self::types_feed::{
    FeedHomeVideoGroup, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
};
use self::types_library_tab::LibraryTab;
#[cfg(test)]
use self::types_playback::HomePane;
use self::types_playback::{
    ArtistHeaderSelection, LocalPlaybackTarget, PendingQueueAction, PlaybackState, PlaybackTarget,
    QueueScope, QueueScopeResolution, RemotePlaybackTarget, RemoteSlotState, SuspendedLocalSession,
    UndoEntry,
};
use self::types_player_tab::PlayerTab;
use self::types_settings::{PanelFocus, SettingKey, SETTING_SECTIONS};
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

#[cfg(test)]
use mbv_core::api::MediaItem;
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
        self.maybe_restore_queue_state();

        // Initialize idle feed if configured
        if self
            .client
            .lock()
            .unwrap()
            .config
            .idle_feed_rss_url
            .is_empty()
        {
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
        let quit_timeout =
            Duration::from_secs(self.client.lock().unwrap().config.quit_timeout_secs);
        start_quit_watchdog(self.player.quit_handle(), quit_timeout);

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

            self.sync_volume_from_player();

            // Advance idle feed rotation
            self.advance_idle_feed_rotation();

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
                                            // Printed only after the terminal is restored (task 7.2): anything
                                            // written while still in the alternate screen would never be
                                            // visible once it's left.
        if let Some(msg) = self.pending_exit_message.take() {
            println!("{msg}");
        }
        Ok(())
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
#[path = "tests_library_position_activation.rs"]
mod tests_library_position_activation;

#[cfg(test)]
#[path = "tests_library_position_restore.rs"]
mod tests_library_position_restore;

#[cfg(test)]
#[path = "tests_panel_focus.rs"]
mod tests_panel_focus;

#[cfg(test)]
#[path = "tests_lifecycle.rs"]
mod tests_lifecycle;

#[cfg(test)]
#[path = "tests_session_connect.rs"]
mod tests_session_connect;

#[cfg(test)]
#[path = "tests_daemon_bootstrap.rs"]
mod tests_daemon_bootstrap;

#[cfg(test)]
#[path = "tests_auto_reconnect.rs"]
mod tests_auto_reconnect;

#[cfg(test)]
#[path = "tests_library_route.rs"]
mod tests_library_route;

#[cfg(test)]
#[path = "tests_feed_group_nav.rs"]
mod tests_feed_group_nav;

#[cfg(test)]
#[path = "tests_feed_group_loading.rs"]
mod tests_feed_group_loading;

#[cfg(test)]
#[path = "tests_podcast.rs"]
mod tests_podcast;

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
#[path = "tests_queue_reorder.rs"]
mod tests_queue_reorder;

#[cfg(test)]
#[path = "tests_route_state.rs"]
mod tests_route_state;

#[cfg(test)]
#[path = "tests_status_bar.rs"]
mod tests_status_bar;

#[cfg(test)]
#[path = "tests_music_grouping.rs"]
mod tests_music_grouping;
