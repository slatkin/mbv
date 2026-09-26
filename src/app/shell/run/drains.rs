//! Per-tick drains and the terminal-message tick, extracted from `Model::run`
//! (issue #800). Behaviour-preserving extractions; `run` keeps the ordering.

use super::super::{
    arbitrate_key, fold_mouse_messages, service_startup, Model, MusicTrackFocusRequest,
    RouterOutcome,
};
use super::{Duration, IdleFeed, Instant, PollStrategy};
use crate::app::dispatch::session::player_event::PlayerEventFlow;

/// Outcome of one `drain_worker` step.
enum WorkerDrain {
    Completed,
    Empty,
    Disconnected,
}

impl Model {
    /// One `take/try_recv/match` step shared by the startup-worker channels:
    /// a completion runs `on_completion`, `Empty` leaves the receiver for the
    /// caller to put back into its slot, and a disconnected channel runs
    /// `on_disconnect` (slot stays empty).
    fn drain_worker<R>(
        &mut self,
        rx: &mut std::sync::mpsc::Receiver<R>,
        on_completion: impl FnOnce(&mut Self, R),
        on_disconnect: impl FnOnce(&mut Self),
    ) -> WorkerDrain {
        match rx.try_recv() {
            Ok(completion) => {
                on_completion(self, completion);
                WorkerDrain::Completed
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => WorkerDrain::Empty,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                on_disconnect(self);
                WorkerDrain::Disconnected
            }
        }
    }

    /// Drain the Emby startup and Emby setup worker channels. Returns whether
    /// any event was drained.
    pub(super) fn drain_startup_workers(&mut self) -> bool {
        let mut had_events = false;
        if let Some(mut worker) = self.app.emby_startup_rx.take() {
            let generation = worker.generation;
            match self.drain_worker(
                &mut worker.rx,
                // Emby bootstrap wrote Home content; assign + re-project
                // (5.3d); stale/error return None.
                super::super::Model::apply_emby_completion_drain,
                |model| model.app.handle_emby_startup_worker_disconnect(generation),
            ) {
                WorkerDrain::Completed | WorkerDrain::Disconnected => had_events = true,
                WorkerDrain::Empty => self.app.emby_startup_rx = Some(worker),
            }
        }
        if let Some(mut rx) = self.app.emby_setup_rx.take() {
            match self.drain_worker(
                &mut rx,
                // Emby setup drain re-bootstraps Home content; assign +
                // re-project (5.3d); stale/decline return None.
                super::super::Model::apply_emby_setup_completion_drain,
                |model| model.app.handle_emby_setup_worker_disconnect(),
            ) {
                WorkerDrain::Completed | WorkerDrain::Disconnected => had_events = true,
                WorkerDrain::Empty => self.app.emby_setup_rx = Some(rx),
            }
        }
        had_events
    }

    /// Drain one player event. Returns whether playback requested a restart
    /// (the caller `continue`s the run loop).
    pub(super) fn drain_player_events(&mut self, had_events: &mut bool) -> bool {
        let Some(ev) = self.app.player_rx.try_recv().ok() else {
            return false;
        };
        *had_events = true;
        let flow = self.app.handle_player_event(ev);
        // Playback completion refetches Home; re-project (task 5.3d, sync_home
        // mirror deletion).
        self.push_home_content();
        // Emby browser content may have changed (5.3d.15/M2).
        self.push_active_emby_library_owner_content();
        // Player events can reconcile ABS podcast progress; re-project (5.3d.11 U6).
        self.push_audiobookshelf_podcast_content();
        // Player events can reconcile ABS book progress; re-project (5.3d).
        self.push_audiobookshelf_book_content();
        self.push_music_workspace_content();
        flow == PlayerEventFlow::RestartLoop
    }

    /// Drain pending library events. Returns whether any event was drained.
    pub(super) fn drain_lib_events(&mut self) -> bool {
        let mut had_events = false;
        while let Ok(ev) = self.app.lib_rx.try_recv() {
            had_events = true;
            match ev {
                crate::app::LibEvent::EmbyLatestSnapshotFetched {
                    library_id,
                    title,
                    items,
                } => {
                    self.update_emby_latest_snapshot(
                        &library_id,
                        title,
                        items
                            .into_iter()
                            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                            .collect(),
                    );
                }
                crate::app::LibEvent::Loaded {
                    lib_idx,
                    parent_id,
                    level,
                } => {
                    let latest = self
                        .app
                        .libs
                        .get(lib_idx)
                        .filter(|lib| {
                            lib.library.collection_type == "tvshows"
                                && (lib.tv_content_mode
                                    == Some(mbv_core::config::TvContentMode::Latest)
                                    || level.tv_content_mode
                                        == Some(mbv_core::config::TvContentMode::Latest))
                        })
                        .map(|lib| {
                            (
                                lib.library.id.clone(),
                                lib.library.name.clone(),
                                level
                                    .items
                                    .iter()
                                    .cloned()
                                    .map(Box::new)
                                    .map(mbv_core::playback_queue::QueueItem::Emby)
                                    .collect(),
                            )
                        });
                    self.app.handle_lib_event(crate::app::LibEvent::Loaded {
                        lib_idx,
                        parent_id,
                        level,
                    });
                    if let Some((library_id, title, items)) = latest {
                        self.update_emby_latest_snapshot(&library_id, title, items);
                    }
                }
                // Recursive album activation used to write `Some(0)` on
                // the deleted inline track-focus field directly; the
                // component owns the cursor now, so the shell delivers
                // the same trigger as a one-shot request consumed at the
                // next sync (wide only -- narrow keeps track focus off).
                crate::app::LibEvent::RecursiveAlbumActivated {
                    library_id,
                    nav_stack,
                } => {
                    self.on_recursive_album_activated(library_id, nav_stack);
                }
                // Position restore used to clear the deleted track-focus
                // field; route the same reset to the component at the
                // next sync.
                crate::app::LibEvent::RestoreLibraryPosition { .. } => {
                    self.handle_restored_library_position_event(ev);
                    self.music_track_focus_request = Some(MusicTrackFocusRequest::Clear);
                    // Saved position restored into the nav stack; re-anchor
                    // the workspace cursor to it at this event rather than
                    // by an equality test on the next content push.
                    self.music_workspace_reanchor = true;
                    self.push_inline_search_content();
                }
                crate::app::LibEvent::HomeContentRefreshed(content) => {
                    self.assign_home_content(*content);
                }
                crate::app::LibEvent::SeriesDetailFetched { .. } => {
                    self.app.handle_lib_event(ev);
                    self.push_tv_workspace_content();
                }
                // Artist detail completions (tasks 6.1/6.2): the App arms
                // own the stale-guard and cache writes; the drain tail's
                // Music re-push projects whatever is now current. Kept as
                // explicit arms so the variants are never wildcard-hidden.
                crate::app::LibEvent::ArtistTracksFetched { .. }
                | crate::app::LibEvent::ArtistArtworkFetched { .. } => {
                    self.app.handle_lib_event(ev);
                }
                crate::app::LibEvent::HomeContentCleared => self.clear_home_content(),
                ev => self.handle_inline_search_lib_event(ev),
            }
            self.push_audiobookshelf_podcast_content();
            // Emby browser content may have changed (5.3d.15/M2).
            self.push_active_emby_library_owner_content();
            // ABS book async completions (BooksFetched / BookDetailFetched)
            // and saved-position restore arrive via lib events; re-project (5.3d).
            self.push_audiobookshelf_book_content();
            self.push_music_workspace_content();
            self.push_tv_workspace_content();
        }
        had_events
    }

    /// Drain idle-feed items and trigger the 30-minute refetch.
    pub(super) fn drain_idle_feed(&mut self) -> bool {
        let Some(ref mut idle_feed) = self.app.idle_feed else {
            return false;
        };
        let mut had_events = false;
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
        had_events
    }

    /// Drain resized-image worker responses into the matching protocol of
    /// the bare-key cache entry (#164). Returns whether any response was
    /// drained. A response for an evicted/replaced/absent key is silently
    /// dropped here; `update_resized_protocol` also guards on
    /// `ThreadProtocol`'s internal id, so a stale response racing a newer
    /// resize request for the same (still-present) key is a no-op too.
    pub(super) fn drain_resize_responses(&mut self) -> bool {
        let mut had_events = false;
        while let Ok((key, response)) = self.app.resize_response_rx.try_recv() {
            had_events = true;
            // Responses are tagged with the per-suffix mem-key
            // ("bare@suffix"); route them into the matching protocol of
            // the bare-key cache entry.
            let Some((bare_key, suffix)) = key.rsplit_once('@') else {
                continue;
            };
            let Some(entry) = self.app.card_image_states.get_mut(bare_key) else {
                continue;
            };
            if let Some(state) = entry.protocols.get_mut(suffix) {
                state.update_resized_protocol(response);
            }
        }
        had_events
    }

    /// Drain Emby websocket events. Returns whether any event was drained.
    pub(super) fn drain_ws_events(&mut self) -> bool {
        let mut had_events = false;
        while let Ok(ev) = self.app.ws_rx.try_recv() {
            had_events = true;
            self.app.handle_ws_event(ev);
            // `UserDataChanged` refetches Continue Watching inside the handler.
            self.push_home_content();
            // Emby browser content may have changed (5.3d.15/M2).
            self.push_active_emby_library_owner_content();
            self.push_music_workspace_content();
        }
        had_events
    }

    /// Drain Audiobookshelf socket events. Returns whether any event was
    /// drained.
    pub(super) fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        let mut had_events = false;
        while let Ok(ev) = self.app.audiobookshelf_socket_rx.try_recv() {
            had_events = true;
            self.app.handle_audiobookshelf_socket_event(ev);
            // Socket events reconcile ABS podcast episode progress;
            // re-project (5.3d.11 U6).
            self.push_audiobookshelf_podcast_content();
            // Socket events reconcile ABS book progress; re-project (5.3d).
            self.push_audiobookshelf_book_content();
            self.push_music_workspace_content();
        }
        had_events
    }

    /// Periodic maintenance between ticks: delayed settings save, session
    /// and cast status polls, websocket keepalive, capability refresh.
    pub(super) fn run_periodic_maintenance(&mut self) {
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
                >= crate::app::dispatch::cast_status::CAST_STATUS_POLL_INTERVAL
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
    }

    /// Poll the terminal, arbitrate the keyboard router, and dispatch the
    /// resulting messages. Returns whether the session asked to quit.
    pub(super) fn tick_terminal_messages(
        &mut self,
        had_events: &mut bool,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        // Terminal event poll is now driven by TuiRealm. `tick` polls the
        // crossterm listener (a background worker) for one event within
        // the same timeout the legacy loop used (8 ms with the visualizer,
        // 50 ms otherwise). UiRoot observes every event independently of
        // the active component's `Option<Msg>`; its typed event signal is
        // only handed to the legacy fallback when UiRoot has focus. This
        // preserves D12 redraws for local mutations without duplicating
        // legacy handling on converted surfaces. When the terminal closes
        // (SIGHUP), the listener's failed poll/read surfaces as a tick
        // error; breaking here lets post-loop cleanup run (player.stop +
        // join) — same contract as the legacy direct poll/read path.
        let poll_timeout = if self.app.visualizer.is_some() {
            Duration::from_millis(8)
        } else {
            Duration::from_millis(50)
        };
        let Ok(messages) = self.application.tick(PollStrategy::Once(poll_timeout)) else {
            return true;
        };
        // ADR 0024: fold the mouse-derived messages (one per eligible
        // subscribed component) down to at most one before the keyboard
        // router fold and `handle_terminal_message` dispatch. A keyboard
        // tick passes through untouched.
        let messages = fold_mouse_messages(messages);
        if messages.is_empty() {
            return false;
        }
        *had_events = true;
        // `PollStrategy::Once` delivers at most one terminal event per
        // tick, so this runs 0 or 1 times; `quit` handles the legacy
        // `handle_key`-returns-true loop break without a labelled
        // break inside the fold.
        let mut quit = false;
        // Snapshot focus before handling any messages. A legacy key can
        // mount or dismiss an overlay, changing focus before UiRoot's
        // observer message is folded; routing by the live focus then
        // double-delivers that same terminal event.
        let focused = self.application.focus().cloned();
        // ADR 0023: the Keyboard Router fold. `Application::tick`
        // returns the focused component's message first, then the
        // UiRoot observer's. With `PollStrategy::Once` there is at
        // most one terminal event per tick, so the leaf's request and
        // the router's resolution for the same chord arrive together.
        // The router's outcome selects between them: `Command` runs the
        // semantic command and discards the leaf's message, `Swallow`
        // runs nothing and discards it, `FallThrough` lets the leaf's
        // own request stand.
        let router = self.router_outcome(&messages);
        let (messages, diagnostic) = arbitrate_key(messages, focused.as_ref(), &router);
        // A prefix-namespace dispatch (design D6, task 6.2) runs the
        // mapped action and disarms, like an immediate `Command`.
        if let RouterOutcome::Command(command) | RouterOutcome::PrefixDispatch(command) = &router {
            quit |= self.dispatch_router_command(command);
        }
        // A deferred candidate fires on an unhandled press; its
        // commands never quit, so it does not feed the loop's quit
        // flag.
        self.apply_deferred_candidate(&router, diagnostic.leaf_disposition == "consumed");
        for msg in messages {
            if self.handle_terminal_message(msg, music_resize, tv_resize) {
                quit = true;
            }
        }
        quit
    }

    /// Render a frame when the render cadence allows it. Sets `last_render`
    /// after a successful draw. Terminal-clear or draw failures surface the
    /// original error (the run loop returns it).
    pub(super) fn draw_frame_if_due(
        &mut self,
        had_events: bool,
        last_render: &mut Instant,
        music_resize: bool,
        tv_resize: bool,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // See `render_interval`'s doc comment for the fast/slow cadence rules.
        let render_interval = self.app.render_interval();
        if !self
            .app
            .wants_terminal_render(had_events, *last_render, render_interval)
        {
            return Ok(());
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
        if let Err(e) = terminal.draw(|f| self.draw_frame(f, music_resize, tv_resize)) {
            log::error!(
                target: "run_loop",
                "terminal.draw() failed: {e:?} (kind={:?})",
                e.kind()
            );
            return Err(e.into());
        }
        *last_render = Instant::now();
        Ok(())
    }

    /// Start the configured Remote Service workers after the first TUI frame
    /// has rendered, kick off the startup feed fetch, and register client
    /// capabilities.
    pub(super) fn spawn_startup_services(&mut self) {
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
    }

    /// Create the idle-feed channel pair and start its first fetch when an
    /// RSS URL is configured.
    pub(super) fn init_idle_feed(&mut self) {
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
    }
}
