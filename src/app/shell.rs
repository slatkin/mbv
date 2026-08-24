//! Shell `Model` — the migration's mixed-framework home (design D2/D11/D13).
//!
//! The `Model` owns the legacy `App` (it draws the current UI and runs the
//! existing handlers directly) and the TuiRealm
//! `Application<ComponentId, Msg, UserEvent>`. `Model::run` is the moved body
//! of the former `App::run`: the event-poll section is replaced by
//! `application.tick(PollStrategy::Once(..))`, whose messages are folded back
//! into the existing `App` input handlers via the temporary `LegacyInput`
//! bridge. Every other part of the loop (receiver drains, periodic work,
//! render cadence via `wants_terminal_render`, teardown) is byte-for-byte the
//! legacy behaviour, only with `self.x` rewritten to `self.app.x`.
//!
//! TODO(migrate-tui-to-tuirealm): this whole module is the strangler-phase
//! shell. It shrinks as surfaces convert and is gone at the completion gate
//! (task 5.3/5.6), leaving only shell/runtime authority + the `Application`.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tuirealm::application::{Application, PollStrategy};
use tuirealm::listener::EventListenerCfg;

use super::components::{
    ComponentId, LegacyInput, LegacyTerminalEvent, Msg, OverlayId, PlaybackGatesComponent,
    ServiceRequest, ShellRequest, UserEvent,
};
use super::service_startup;
use super::{
    init_terminal, install_signal_handlers, restore_terminal, start_quit_watchdog, QUIT_REQUESTED,
};
use super::{App, IdleFeed, ToastSeverity};

/// How often the TuiRealm crossterm listener worker polls the terminal for
/// events. The listener's `poll` blocks for half of this; the worker cycle is
/// this long. Set to 8 ms so event latency matches the legacy loop's fastest
/// cadence (the visualizer's 8 ms poll). The main thread's per-iteration wait
/// is governed separately by the `PollStrategy::Once` timeout below, so this
/// only affects how promptly a buffered event reaches the channel — not the
/// render cadence.
const LEGACY_INPUT_LISTENER_INTERVAL: Duration = Duration::from_millis(8);
/// Upper bound on events the listener drains from crossterm in one worker
/// cycle. Generous so a burst (e.g. a mouse drag) is flushed into the channel
/// in one cycle; the main thread still processes at most one per tick via
/// `PollStrategy::Once`, matching the legacy one-event-per-iteration loop.
const LEGACY_INPUT_LISTENER_MAX_POLL: usize = 60;

/// Shell model holding the legacy `App` and the TuiRealm `Application`.
pub struct Model {
    pub app: App,
    pub(super) application: Application<ComponentId, Msg, UserEvent>,
}

impl Model {
    /// Construct the model, starting the TuiRealm crossterm listener and
    /// mounting the temporary `LegacyInput` bridge as the sole active
    /// component (checkpoint 1).
    pub fn new(app: App) -> Self {
        let listener_cfg = EventListenerCfg::default().crossterm_input_listener(
            LEGACY_INPUT_LISTENER_INTERVAL,
            LEGACY_INPUT_LISTENER_MAX_POLL,
        );
        let application = Application::init(listener_cfg);
        let mut model = Self { app, application };
        // Checkpoint 1: the only mounted component is the temporary
        // LegacyInput bridge, occupying the UiRoot slot. It is the active
        // component so every terminal event is forwarded to it.
        // TODO(migrate-tui-to-tuirealm): replace this mount with the real
        // UiRoot component as surfaces convert (task 5.2); remove LegacyInput
        // at task 5.3.
        model
            .application
            .mount(ComponentId::UiRoot, Box::new(LegacyInput), vec![])
            .expect("mount LegacyInput bridge");
        model
            .application
            .active(&ComponentId::UiRoot)
            .expect("activate LegacyInput bridge");
        // Home is mounted for the whole session but never made active: its
        // input stays on the legacy path, only its render is component-owned
        // (task 3.4; see `shell_home.rs`/`components::home`'s module docs).
        model.mount_home();
        model.mount_feeds();
        // Precedence-gate attribute carrier (see `components::playback_gates`
        // module docs) -- mounted for the whole session, never active/
        // subscribed, so a future `SubClause::HasAttrValue` guard on
        // `confirm_skip_intro`/`confirm_next_up` has real state to read.
        model
            .application
            .mount(
                ComponentId::Playback,
                Box::new(PlaybackGatesComponent::new()),
                vec![],
            )
            .expect("mount PlaybackGates");
        model
    }

    /// The run loop — the moved body of the former `App::run`.
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = init_terminal()?;
        terminal.clear()?;

        // Initialise image picker after terminal is in raw mode. The
        // halfblock picker backs the dimmed-backdrop fallback: while a modal
        // dims the backdrop, images re-encode to halfblocks so the dim
        // applies uniformly (#451).
        self.app.image_picker = Some(self.app.build_image_picker());
        self.app.halfblock_picker = Some(ratatui_image::picker::Picker::halfblocks());

        // Don't clobber a still-live flash message (e.g. try_auto_reconnect's
        // outcome, set during App::new) -- only show "Loading..." if there's
        // no pending flash, mirroring the render loop's own expiry check.
        let has_live_flash = self.app.status_expires.is_some_and(|t| t > Instant::now());
        if !has_live_flash {
            self.app.status = self
                .app
                .emby_client()
                .map(|_| "Loading...".into())
                .unwrap_or_else(|| {
                    service_startup::startup_status(self.app.emby_runtime.state).into()
                });
        }
        self.app.home_loading = true;
        terminal.draw(|f| self.app.render(f))?;

        // Only start the configured Remote Service after the first TUI frame
        // has been rendered. The selected Player owner and UI therefore never
        // wait for Emby setup, authentication, or connectivity.
        if let Some((config, generation)) = self.app.emby_startup_request.take() {
            self.app.emby_startup_rx = Some(service_startup::start(config, generation));
        }
        if let Some((config, generation)) = self.app.audiobookshelf_startup_request.take() {
            self.app.audiobookshelf_startup_rx = Some(service_startup::start_audiobookshelf(
                config,
                generation,
                service_startup::AudiobookshelfCompletionKind::Startup,
            ));
        }

        // Auto-fetch configured feeds asynchronously so the Feeds tab and the
        // Home "Feeds" pill are populated shortly after startup instead
        // of staying empty until the user presses the manual refresh key.
        self.app.start_feed_fetch();

        if let Some(client) = self.app.emby_client() {
            client.lock().unwrap().register_capabilities();
        }

        // Home populates from whatever Services are available at this moment;
        // Emby's independent startup connection merges its portion in when it
        // completes (apply_emby_bootstrap), so no Emby gate is needed here
        // (#543 Part 1).
        match self.app.fetch_home() {
            Ok(()) => {
                let has_live_flash = self.app.status_expires.is_some_and(|t| t > Instant::now());
                if !has_live_flash {
                    self.app.status.clear();
                }
            }
            Err(e) => self
                .app
                .flash(format!("Couldn't load home: {e}"), ToastSeverity::Warning),
        }
        self.app.home_loading = false;
        self.app.maybe_restore_queue_state();

        // Initialize idle feed if configured
        if self.app.config.lock().unwrap().idle_feed_rss_url.is_empty() {
            // No RSS URL configured, skip idle feed
        } else {
            let (items_tx, items_rx) = std::sync::mpsc::channel();
            self.app.idle_feed = Some(IdleFeed {
                items: Vec::new(),
                current_index: 0,
                last_rotation: Instant::now(),
                last_fetch: Instant::now(),
                items_tx,
                items_rx,
            });
            self.app.spawn_idle_feed_fetch();
        }

        terminal.draw(|f| self.app.render(f))?;

        install_signal_handlers();
        let quit_timeout = Duration::from_secs(self.app.config.lock().unwrap().quit_timeout_secs);
        start_quit_watchdog(self.app.player.quit_handle(), quit_timeout);

        let mut last_render = Instant::now() - Duration::from_secs(2);

        'outer: loop {
            let mut had_events = false;
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }

            if let Some(worker) = self.app.emby_startup_rx.take() {
                match worker.rx.try_recv() {
                    Ok(completion) => {
                        had_events = true;
                        self.app.apply_emby_completion(completion);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.app.emby_startup_rx = Some(worker);
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.app
                            .handle_emby_startup_worker_disconnect(worker.generation);
                        had_events = true;
                    }
                }
            }
            if let Some(rx) = self.app.emby_setup_rx.take() {
                match rx.try_recv() {
                    Ok(completion) => {
                        had_events = true;
                        self.app.apply_emby_setup_completion(completion);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.app.emby_setup_rx = Some(rx);
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.app.handle_emby_setup_worker_disconnect();
                        had_events = true;
                    }
                }
            }
            had_events |= self.app.drain_audiobookshelf_events();
            if let Ok(ev) = self.app.player_rx.try_recv() {
                had_events = true;
                if self.app.handle_player_event(ev) {
                    continue 'outer;
                }
            }

            had_events |= self.app.drain_notif_actions();

            while let Ok(ev) = self.app.lib_rx.try_recv() {
                had_events = true;
                self.app.handle_lib_event(ev);
            }

            // Search results drain: the shell drains `search_rx` and writes
            // each result into the `SearchSidebarComponent` via downcast
            // (task 3.2). The debounce is component-owned and driven by
            // `UserEvent::Clock`; when the deadline passes the component
            // emits `Msg::Service(SearchQuery)`, which the shell handles by
            // spawning the search.
            had_events |= self.drain_search_results();

            had_events |= self.app.drain_session_events();

            had_events |= self.app.drain_cast_events();

            had_events |= self.app.drain_shared_events();

            had_events |= self.app.drain_feed_tab_results();

            had_events |= self.app.drain_feed_add_results();

            while let Ok((item_id, img_opt)) = self.app.card_image_rx.try_recv() {
                had_events = true;
                self.app.card_image_loading.remove(&item_id);
                self.app.image_fetches_active = self.app.image_fetches_active.saturating_sub(1);
                let entry = self.app.build_cached_image(&item_id, img_opt);
                if entry.img.is_some() {
                    self.app.image_lru.retain(|k| k != &item_id);
                    self.app.image_lru.push_back(item_id.clone());
                    while self.app.image_lru.len() > self.app.image_cache_size_total {
                        if let Some(evict) = self.app.image_lru.pop_front() {
                            self.app.card_image_states.remove(&evict);
                        }
                    }
                }
                self.app.card_image_states.insert(item_id, entry);
            }
            self.app.drain_image_fetches();

            // Apply completed off-thread resize+encode results (#164). A
            // response for an evicted/replaced/absent key is silently
            // dropped here; `update_resized_protocol` also guards on
            // ThreadProtocol's internal id, so a stale response racing a
            // newer resize request for the same (still-present) key is a
            // no-op too.
            while let Ok((key, response)) = self.app.resize_response_rx.try_recv() {
                had_events = true;
                // Responses are tagged with the per-suffix mem-key
                // ("bare@suffix"); route them into the matching protocol of
                // the bare-key cache entry.
                if let Some((bare_key, suffix)) = key.rsplit_once('@') {
                    if let Some(entry) = self.app.card_image_states.get_mut(bare_key) {
                        if let Some(state) = entry.protocols.get_mut(suffix) {
                            state.update_resized_protocol(response);
                        }
                    }
                }
            }

            while let Ok(ev) = self.app.ws_rx.try_recv() {
                had_events = true;
                self.app.handle_ws_event(ev);
            }

            while let Ok(ev) = self.app.audiobookshelf_socket_rx.try_recv() {
                had_events = true;
                self.app.handle_audiobookshelf_socket_event(ev);
            }

            // Drain idle feed items
            if let Some(ref mut idle_feed) = self.app.idle_feed {
                while let Ok(items) = idle_feed.items_rx.try_recv() {
                    had_events = true;
                    idle_feed.items = items;
                    idle_feed.current_index = 0;
                }
                // Re-fetch every 30 minutes
                if idle_feed.last_fetch.elapsed() >= Duration::from_secs(1800) {
                    idle_feed.last_fetch = Instant::now();
                    self.app.spawn_idle_feed_fetch();
                }
            }

            self.app.sync_visualizer();

            if let Some(at) = self.app.settings_save_at {
                if Instant::now() >= at {
                    let cfg = self.app.config.lock().unwrap().clone();
                    crate::config::save_config_with_ui(&cfg, &self.app.ui_config_snapshot());
                    self.app.settings_save_at = None;
                }
            }

            // Periodic session poll when connected to a remote session
            if self.app.connected_session_id.is_some()
                && self.app.last_session_poll.elapsed() >= Duration::from_secs(1)
                && !self.app.sessions_loading
            {
                self.app.spawn_sessions_load();
            }

            // Periodic status poll while attached to a cast target (6.1). The
            // keep-alive heartbeat this poll's background thread sends is not
            // optional -- see `CAST_STATUS_POLL_INTERVAL`'s doc comment.
            if self.app.cast_attachment.is_some()
                && self.app.last_cast_poll.elapsed()
                    >= super::cast_status_actions::CAST_STATUS_POLL_INTERVAL
                && !self.app.cast_status_loading
            {
                self.app.spawn_cast_status_poll();
            }

            // Keep this session visible to other Emby clients
            if let Some(ref tx) = self.app.ws_send_tx {
                if self.app.last_keepalive.elapsed() >= Duration::from_secs(30) {
                    let _ = tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
                    self.app.last_keepalive = Instant::now();
                }
            }
            if self.app.ws_send_tx.is_some()
                && self.app.last_capabilities.elapsed() >= Duration::from_secs(600)
            {
                if let Some(client) = self.app.emby_snapshot() {
                    std::thread::spawn(move || client.register_capabilities());
                }
                self.app.last_capabilities = Instant::now();
            }

            // Terminal event poll is now driven by TuiRealm. `tick` polls the
            // crossterm listener (a background worker) for one event within
            // the same timeout the legacy loop used (8 ms with the visualizer,
            // 50 ms otherwise), then forwards it to the active `LegacyInput`
            // bridge, which translates it into a `Msg::Legacy`. A non-empty
            // result means an event was processed → mark the frame dirty via
            // the existing `had_events` path (design D12). When the terminal
            // closes (SIGHUP), the listener's failed poll/read surfaces as a
            // tick error; breaking here lets the post-loop cleanup run
            // (player.stop + join) so the mpv window closes — same contract as
            // the legacy direct `event::poll`/`event::read` path.
            let poll_timeout = if self.app.visualizer.is_some() {
                Duration::from_millis(8)
            } else {
                Duration::from_millis(50)
            };
            let messages = match self.application.tick(PollStrategy::Once(poll_timeout)) {
                Ok(msgs) => msgs,
                Err(_) => break,
            };
            if !messages.is_empty() {
                had_events = true;
                // `PollStrategy::Once` delivers at most one terminal event per
                // tick, so this runs 0 or 1 times; `quit` handles the legacy
                // `handle_key`-returns-true loop break without a labelled
                // break inside the fold.
                let mut quit = false;
                for msg in messages {
                    match msg {
                        Msg::Legacy(LegacyTerminalEvent::Key(key)) => {
                            // F1 opens the Help overlay unless a blocking
                            // overlay is active (those swallow it). Once
                            // Help is mounted it is the active component, so
                            // F1 arrives as `Msg::Shell(DismissHelp)` instead.
                            if key.code == crossterm::event::KeyCode::F(1)
                                && !self
                                    .application
                                    .mounted(&ComponentId::Overlay(OverlayId::Help))
                                && !self.is_blocking_overlay_open()
                            {
                                self.mount_help();
                            } else if self.app.handle_key(key) {
                                quit = true;
                            }
                        }
                        Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)) => {
                            self.app.handle_mouse(mouse);
                        }
                        Msg::Legacy(LegacyTerminalEvent::Resize) => {
                            self.app.force_clear = true;
                            self.app.card_image_states.clear();
                            self.app.card_image_loading.clear();
                        }
                        Msg::Legacy(LegacyTerminalEvent::FocusGained) => {
                            self.app.note_focus_gained();
                        }
                        Msg::Legacy(LegacyTerminalEvent::FocusLost) => {
                            self.app.note_focus_lost();
                        }
                        // Non-Press keys (collapsed to `Event::None` by the
                        // adapter) and `Paste` (ignored by the legacy
                        // `_ => {}` arm): no-op, but `had_events` is already
                        // set, preserving the legacy "poll-ready ⇒ dirty"
                        // behaviour (design D12).
                        Msg::Legacy(LegacyTerminalEvent::NoOp) => {}
                        // Help overlay cross-boundary requests (design D4).
                        Msg::Shell(ShellRequest::Quit) => quit = true,
                        Msg::Shell(ShellRequest::DismissHelp) => self.umount_help(),
                        Msg::Shell(ShellRequest::OpenSettings) => {
                            self.app.show_sessions = false;
                            self.umount_help();
                            self.app.show_settings = true;
                        }
                        Msg::Shell(ShellRequest::OpenSessions) => {
                            self.umount_help();
                            self.app.show_sessions = true;
                            self.app.spawn_sessions_load();
                            self.app.spawn_cast_discovery();
                        }
                        Msg::Shell(ShellRequest::OpenPlaylists) => {
                            self.app.show_sessions = false;
                            self.umount_help();
                            self.app.open_playlists_panel();
                        }
                        // Confirm modal: forward the key to the existing
                        // `handle_key_confirm_modal` handler. The shell owns
                        // `ConfirmAction` dispatch (task 2.2).
                        Msg::Shell(ShellRequest::ConfirmKey(key)) => {
                            self.app.handle_key_confirm_modal(key);
                        }
                        // Daemon-lost modal: forward the key to the existing
                        // `handle_key_daemon_lost_modal` handler. The shell
                        // owns restart/quit (process-lifecycle effects,
                        // task 2.3).
                        Msg::Shell(ShellRequest::DaemonLostKey(key)) => {
                            self.app.handle_key_daemon_lost_modal(key);
                        }
                        // Remote-reanchor popup: forward the key to the
                        // existing `handle_key_remote_reanchor` handler. The
                        // shell owns cursor/targets and reconciliation
                        // (task 2.4).
                        Msg::Shell(ShellRequest::RemoteReanchorKey(key)) => {
                            self.app.handle_key_remote_reanchor(key);
                        }
                        // Context menu: forward the key to the existing
                        // `handle_key_context_menu` handler. The shell owns
                        // cursor navigation and action execution (task 2.5).
                        Msg::Shell(ShellRequest::ContextMenuKey(key)) => {
                            self.app.handle_key_context_menu(key);
                        }
                        // Search sidebar: dismiss (Esc/Backspace-on-empty).
                        // The component owns the state; the shell clears the
                        // open flag, which `sync_search_sidebar` picks up to
                        // unmount (task 3.2).
                        Msg::Shell(ShellRequest::DismissSearch) => {
                            self.app.search_sidebar_open = false;
                        }
                        // Search sidebar: activate result (Enter). The
                        // component owns the cursor/results; the shell owns
                        // the library tabs and navigation spawn (task 3.2).
                        Msg::Shell(ShellRequest::SearchActivate { id, item_type }) => {
                            self.app.activate_search_result(id, item_type);
                        }
                        Msg::Shell(ShellRequest::DismissSessions) => {
                            self.app.show_sessions = false;
                        }
                        Msg::Shell(ShellRequest::RefreshSessions) => {
                            self.app.spawn_sessions_load();
                            self.app.spawn_cast_discovery();
                        }
                        Msg::Shell(ShellRequest::SelectSession(index)) => {
                            if let Some(target) = self.app.panel_targets.get(index).cloned() {
                                self.app.select_panel_target(target);
                            }
                        }
                        Msg::Shell(ShellRequest::DetachSessions) => {
                            self.app.disconnect_remote();
                            if self.app.is_cast_attached() {
                                self.app.detach_cast();
                                self.app.flash(
                                    "Detached from cast target".to_string(),
                                    ToastSeverity::Success,
                                );
                            }
                            self.app.show_sessions = false;
                        }
                        Msg::Shell(ShellRequest::RefreshFeeds) => {
                            self.app.refresh_feeds();
                        }
                        Msg::Shell(ShellRequest::FeedsPlay(cursor)) => {
                            self.app.feed_tab.cursor = cursor;
                            let _ = self
                                .app
                                .handle_feed_tab_key(crossterm::event::KeyEvent::new(
                                    crossterm::event::KeyCode::Enter,
                                    crossterm::event::KeyModifiers::NONE,
                                ));
                        }
                        Msg::Shell(ShellRequest::FeedsEnqueue(cursor)) => {
                            self.app.feed_tab.cursor = cursor;
                            let _ = self
                                .app
                                .handle_feed_tab_key(crossterm::event::KeyEvent::new(
                                    crossterm::event::KeyCode::Char('e'),
                                    crossterm::event::KeyModifiers::NONE,
                                ));
                        }
                        Msg::Shell(ShellRequest::PlaybackPromptKey(key)) => {
                            if self.app.skip_intro_end_ticks.is_some() {
                                self.app.handle_key_confirm_skip_intro(key);
                            } else if self.app.next_up_item.is_some() {
                                self.app.handle_key_confirm_next_up(key);
                            }
                        }
                        // Search sidebar: debounce deadline passed. The
                        // component owns the debounce; the shell owns the
                        // Emby client and spawns the search thread (task 3.2).
                        Msg::Service(ServiceRequest::SearchQuery(query)) => {
                            if let Some(client) = self.app.emby_snapshot() {
                                self.app.spawn_search_sidebar_query(client, query);
                            }
                        }
                        // No other Msg variants are produced yet.
                        _ => {}
                    }
                }
                if quit {
                    break 'outer;
                }
            }

            // Sync the Confirm modal mount state with `App::confirm_modal`
            // after processing tick messages. Mounts when an action sets the
            // modal; unmounts when `handle_key_confirm_modal` clears it
            // (task 2.2).
            self.sync_confirm_modal();
            self.sync_daemon_lost_modal();
            self.sync_remote_reanchor_popup();
            self.sync_context_menu();
            self.sync_search_sidebar();
            self.sync_sessions();
            self.sync_home();
            self.sync_feeds();
            self.sync_playback_prompt();
            self.sync_precedence_gates();

            self.app.expire_music_grouping_candidates();
            self.app.sync_volume_from_player();
            self.app.flush_library_position_if_idle();

            // Advance idle feed rotation
            self.app.advance_idle_feed_rotation();

            // See `render_interval`'s doc comment for the fast/slow cadence rules.
            let render_interval = self.app.render_interval();
            if self
                .app
                .wants_terminal_render(had_events, last_render, render_interval)
            {
                if self.app.last_throbber_advance.elapsed() >= std::time::Duration::from_millis(300)
                {
                    let playback = self.app.effective_playback_state();
                    if playback.active && !self.app.playback_transport_paused() {
                        self.app.now_playing_throbber_index =
                            self.app.now_playing_throbber_index.wrapping_add(1);
                    }
                    self.app.last_throbber_advance = std::time::Instant::now();
                }
                if self.app.force_clear {
                    self.app.force_clear = false;
                    if let Err(e) = terminal.clear() {
                        log::error!(
                            target: "run_loop",
                            "terminal.clear() failed: {e:?} (kind={:?})",
                            e.kind()
                        );
                        return Err(e.into());
                    }
                }
                if self.app.visualizer.is_some() {
                    self.app.sync_visualizer();
                }
                if let Err(e) = terminal.draw(|f| {
                    self.app.render(f);
                    self.render_home_component(f);
                    self.render_feeds_component(f);
                    self.render_playback_prompt(f);
                    self.render_help_overlay(f);
                    self.render_confirm_overlay(f);
                    self.render_daemon_lost_overlay(f);
                    self.render_remote_reanchor_overlay(f);
                    self.render_context_menu_overlay(f);
                    self.render_search_overlay(f);
                    self.render_sessions_overlay(f);
                }) {
                    log::error!(
                        target: "run_loop",
                        "terminal.draw() failed: {e:?} (kind={:?})",
                        e.kind()
                    );
                    return Err(e.into());
                }
                last_render = Instant::now();
            }
        }

        self.app.teardown(quit_timeout);
        let _ = restore_terminal(terminal); // ignore errors — terminal may be gone (SIGHUP)
                                            // Printed only after the terminal is restored (task 7.2): anything
                                            // written while still in the alternate screen would never be
                                            // visible once it's left.
        if let Some(msg) = self.app.pending_exit_message.take() {
            println!("{msg}");
        }
        Ok(())
    }
}
