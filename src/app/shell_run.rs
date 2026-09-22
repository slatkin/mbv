use super::*;
use crate::app::images::SERIES_IMAGE_CACHE_KEY_INFIX;
use crate::app::PanelFocus;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

impl Model {
    pub(crate) fn sync_mounted_surfaces(&mut self) {
        // Tasks 1.1/1.2: the draw-time state mutations run here, before any
        // draw. The saved-tab resolution and the stale-destination fallback
        // settle the active tab so every projection below -- and the frame --
        // sees the resolved tab (the sync pass no longer writes `self.tab`),
        // and the terminal-resize handling (card-image clear, queue-column
        // clamp + prefs save, and the mini-view focus hand-off on a real
        // Resize event) leaves the draw path, which now only reads geometry.
        self.sync_terminal_resize();
        self.app.resolve_library_tab_pending();
        self.app.normalize_stale_browse_destination();
        // Apply App-owned effect handoffs to their mounted components.
        // `sync_home` was deleted (task 5.3d, sync_home mirror deletion):
        // Home content/focus is projected event-driven by
        // `push_home_content` at the seams above.
        self.sync_modal_requests();
        self.sync_sidebar_overlays();
        self.sync_library_playback_panel();
        self.sync_feeds();
        self.sync_audiobookshelf_book();
        self.sync_queue();
        self.sync_queue_boundary();
        self.sync_tab_panel();
        self.sync_status_bar_panel();
        self.sync_queue_card_geometry();
        self.sync_queue_playback_panel();
        // The Music workspace re-projects every sync pass: navigation
        // landings, track-fetch arrivals, focus requests, and breakpoint
        // flips all land in App state, and the owner only picks them up
        // through this push (the re-anchor, deep-selection, and
        // track-focus one-shots are consumed here). Event writers push
        // too, but the sync pass is the backstop that direct App writers
        // (and the tests driving them) rely on.
        self.push_music_workspace_content();
        // Task 8.4: the TV owner is installed/pushed before the panel's
        // owner-retention and active-pointer pass below.
        self.sync_tv_content();
        // Task 5.9: the Library panel mounts with the library column and
        // drives its owner map (retention + the active pointer) before the
        // focus pass routes to the active surface.
        self.sync_library_panel();
        // Task 3.2: restore the selected destination's main Selector before
        // its library item, then consume the pending intent exactly once.
        self.reanchor_pending_launch_destination();
        // Task 3.1: a landing that completed during this iteration's lib-event
        // drain owes the Series detail hand-off. Consume it here, after the
        // panel's active pointer follows the landed tab and before the hero
        // image / focus passes so they see the opened presentation.
        self.drain_series_navigation_handoff();
        // Task 6.1: a navigated episode's workspace selection retries as its
        // series detail / season episodes drain (each retry may arm the next
        // season fetch; absence clears it silently).
        self.drain_pending_episode_selection();
        // Task 5.10 (design D9): the active owner's hero image projection —
        // the fetches and the cover-fit box re-encode — runs here, before the
        // draw, so painting reads projected state only.
        self.sync_library_hero_images();
        // Retire destination components whose Service library left the
        // catalog before the focus pass routes to the active destination
        // (keep-destination-components-mounted tasks 1.3).
        self.sync_active_destination();
        // ADR 0024 D2: mouse eligibility is derived off the same
        // active-destination derivation, in the same pass, right after it.
        self.sync_mouse_subscriptions();
    }

    /// Terminal-resize side effects, applied in the sync pass before any draw
    /// (task 1.2). A size drift against the size this pass last handled runs
    /// the former draw-time mutations: the card-image state clear and the
    /// queue-column clamp + prefs save (this also picks up the startup draw's
    /// size normalization and any direct-frame normalization). The mini-view
    /// focus hand-off runs only when the Resize observer armed it -- the real
    /// terminal-resize event, whose pre-resize width the marker still holds.
    pub(super) fn sync_terminal_resize(&mut self) {
        let size = (self.app.terminal_width, self.app.terminal_height);
        let resize_event = std::mem::take(&mut self.pending_terminal_resize);
        if self.handled_terminal_size == size && !resize_event {
            return;
        }
        let was_wide = self.handled_terminal_size.0 >= crate::app::MINI_VIEW_THRESHOLD;
        self.handled_terminal_size = size;
        self.app.card_image_states.clear();
        self.app.card_image_loading.clear();
        // Crossing into mini view on a real resize hands focus to the queue;
        // the stored wide focus is untouched.
        if resize_event && was_wide && size.0 < crate::app::MINI_VIEW_THRESHOLD {
            self.app.mini_view_focus = PanelFocus::Queue;
        }
        if self.app.clamp_queue_column_width() {
            self.app.save_prefs();
        }
    }

    /// The sole frame orchestrator (D3): root placement publication, mounted
    /// component views, deferred protocol-image paint, and overlay stack.
    pub(in crate::app) fn draw_frame(
        &mut self,
        f: &mut ratatui::Frame,
        _music_resize: bool,
        _tv_resize: bool,
    ) {
        // The legacy base frame reads the blocking-overlay state for its dim
        // backdrop and stay-alive indicator; that fact now lives in TuiRealm
        // mount state, so the shell computes it once per frame (the deleted
        // App-level `blocking_overlay_active` adapter, task 5.3d).
        self.app.dim_backdrop_active = self.blocking_overlay_active();
        self.app.compose_root_frame(f);
        // Root composition is data-driven: this is the sole panel paint loop.
        // The queue playback placement is deliberately handled by its panel
        // method; its transport geometry was projected during sync from the
        // prior-paint card checkpoint, so this draw path remains read-only.
        for placement in self.app.layout.root_frame.placements() {
            let Some(placement) = placement else { continue };
            match placement {
                crate::app::render::arrangements::chrome::PanelPlacement::Tab(area) => {
                    self.render_tab_panel_at(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::Library(area) => {
                    self.render_library_panel_at(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::LibraryPlayback(area) => {
                    self.render_library_playback_panel_at(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::Queue(area) => {
                    self.render_queue_panel_at(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::QueuePlayback(area) => {
                    self.render_queue_playback_panel(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::StatusBar(area) => {
                    self.render_status_bar_panel_at(f, area)
                }
                crate::app::render::arrangements::chrome::PanelPlacement::QueueBoundary(area) => {
                    self.render_queue_boundary_at(f, area)
                }
            }
        }
        self.render_overlay_stack(f);
    }

    /// Drain completed card-image fetches into the render cache. Returns whether
    /// any completion arrived.
    ///
    /// A completion under a Series (`:ser:` family) key re-projects TV workspace
    /// content: the Wide push reserved its painted key and painted the
    /// placeholder, and the cached entry only reaches the screen through that
    /// re-push. The infix is the only Series-family marker, so both live chains
    /// (Wide's Thumb-first, narrow's `Primary`) gate without a suffix list that
    /// can drift from the key the painter builds.
    pub(in crate::app) fn drain_card_image_completions(&mut self) -> bool {
        let mut series_image_changed = false;
        let mut drained = false;
        while let Ok((cache_key, img_opt)) = self.app.card_image_rx.try_recv() {
            drained = true;
            series_image_changed |= cache_key.contains(SERIES_IMAGE_CACHE_KEY_INFIX);
            self.app.card_image_loading.remove(&cache_key);
            self.app.image_fetches_active = self.app.image_fetches_active.saturating_sub(1);
            let entry = self.app.build_cached_image(&cache_key, img_opt);
            // Artist artwork (task 6.2, design D7): a fetch reserved through
            // the typed artist request identity completes as its own typed
            // event; the generic image cache stays provider/generic.
            let artist_completion =
                self.app
                    .artist_artwork_requests
                    .remove(&cache_key)
                    .map(|identity| crate::app::LibEvent::ArtistArtworkFetched {
                        destination: identity.destination,
                        generation: mbv_core::service_runtime::SetupGeneration::new(
                            identity.generation,
                        ),
                        artist_id: identity.artist_id,
                        revision: identity.revision,
                        cache_key: cache_key.clone(),
                        available: entry.img.is_some(),
                    });
            if entry.img.is_some() {
                self.app.image_lru.retain(|k| k != &cache_key);
                self.app.image_lru.push_back(cache_key.clone());
                while self.app.image_lru.len() > self.app.image_cache_size_total {
                    if let Some(evict) = self.app.image_lru.pop_front() {
                        self.app.card_image_states.remove(&evict);
                    }
                }
            }
            self.app.card_image_states.insert(cache_key, entry);
            if let Some(event) = artist_completion {
                let _ = self.app.lib_tx.send(event);
            }
        }
        if series_image_changed {
            self.push_tv_workspace_content();
        }
        drained
    }

    /// The run loop — the moved body of the former `App::run`.
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mouse_support = self.app.config.lock().unwrap().mouse_support;
        let mut terminal = init_terminal(mouse_support)?;
        terminal.clear()?;

        // Image pickers are initialised in `main` before `Model::new` starts
        // the TuiRealm listener — see `App::init_image_pickers` (#654).

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
        self.home_content.loading = true;
        terminal.draw(|f| self.draw_frame(f, false, false))?;

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

        // Home populates now; Emby's startup merges its portion later (#543).
        self.fetch_home_at_startup();
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

        terminal.draw(|f| self.draw_frame(f, false, false))?;

        install_signal_handlers();
        let quit_timeout = Duration::from_secs(self.app.config.lock().unwrap().quit_timeout_secs);
        start_quit_watchdog(self.app.player.quit_handle(), quit_timeout);

        let mut last_render = Instant::now() - Duration::from_secs(2);

        'outer: loop {
            let mut had_events = false;
            let mut music_resize = false;
            let mut tv_resize = false;
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }

            if let Some(worker) = self.app.emby_startup_rx.take() {
                match worker.rx.try_recv() {
                    Ok(completion) => {
                        had_events = true;
                        // Emby bootstrap wrote Home content; assign + re-project
                        // (5.3d); stale/error return None.
                        self.apply_emby_completion_drain(completion);
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
                        // Emby setup drain re-bootstraps Home content; assign +
                        // re-project (5.3d); stale/decline return None.
                        self.apply_emby_setup_completion_drain(completion);
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
            let drained_abs_events = self.app.drain_audiobookshelf_events();
            had_events |= drained_abs_events;
            // ABS startup/refresh reset the browse state and reconcile
            // per-episode progress; re-project the active podcast browser
            // (task 5.3d.11 U6). Only project when the drain actually reported
            // an event.
            if drained_abs_events {
                self.push_audiobookshelf_podcast_content();
                self.push_audiobookshelf_book_content();
                self.push_music_workspace_content();
            }
            if let Ok(ev) = self.app.player_rx.try_recv() {
                had_events = true;
                let restart = self.app.handle_player_event(ev);
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
                if restart {
                    continue 'outer;
                }
            }

            had_events |= self.app.drain_notif_actions();

            while let Ok(ev) = self.app.lib_rx.try_recv() {
                had_events = true;
                match ev {
                    // Recursive album activation used to write `Some(0)` on
                    // the deleted inline track-focus field directly; the
                    // component owns the cursor now, so the shell delivers
                    // the same trigger as a one-shot request consumed at the
                    // next sync (wide only -- narrow keeps track focus off).
                    super::super::LibEvent::RecursiveAlbumActivated {
                        library_id,
                        nav_stack,
                    } => {
                        self.on_recursive_album_activated(library_id, nav_stack);
                    }
                    // Position restore used to clear the deleted track-focus
                    // field; route the same reset to the component at the
                    // next sync.
                    super::super::LibEvent::RestoreLibraryPosition { .. } => {
                        self.app.handle_lib_event(ev);
                        self.music_track_focus_request = Some(MusicTrackFocusRequest::Clear);
                        // Saved position restored into the nav stack; re-anchor
                        // the workspace cursor to it at this event rather than
                        // by an equality test on the next content push.
                        self.music_workspace_reanchor = true;
                        self.push_inline_search_content();
                    }
                    // App-internal Home writers deliver content/section deltas
                    // to assign/merge into Model-owned `home_content` (5.3d).
                    super::super::LibEvent::HomeContentRefreshed(content) => {
                        self.assign_home_content(*content)
                    }
                    super::super::LibEvent::SeriesDetailFetched { .. } => {
                        self.app.handle_lib_event(ev);
                        self.push_tv_workspace_content();
                    }
                    // Artist detail completions (tasks 6.1/6.2): the App arms
                    // own the stale-guard and cache writes; the drain tail's
                    // Music re-push projects whatever is now current. Kept as
                    // explicit arms so the variants are never wildcard-hidden.
                    super::super::LibEvent::ArtistTracksFetched { .. }
                    | super::super::LibEvent::ArtistArtworkFetched { .. } => {
                        self.app.handle_lib_event(ev);
                    }
                    super::super::LibEvent::HomeContentCleared => self.clear_home_content(),
                    super::super::LibEvent::AudiobookshelfLatestRebuilt(sections) => {
                        self.merge_home_abs_sections(sections)
                    }
                    super::super::LibEvent::FeedsLatestRebuilt(sections) => {
                        self.merge_home_feeds_sections(sections)
                    }
                    ev => self.handle_inline_search_lib_event(ev),
                }
                // Every lib event re-projects Home and the podcast browser
                // (idempotent; 5.3d.11 U6): lib events deliver ShowsFetched /
                // DetailFetched async completions, RestoreLibraryPosition
                // saved-position restore, and audio progress reconciles.
                self.push_home_content();
                self.push_audiobookshelf_podcast_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_active_emby_library_owner_content();
                // ABS book async completions (BooksFetched / BookDetailFetched)
                // and saved-position restore arrive via lib events; re-project (5.3d).
                self.push_audiobookshelf_book_content();
                self.push_music_workspace_content();
                self.push_tv_workspace_content();
            }

            // Search results drain: the shell drains `search_rx` and writes
            // each result into the `SearchSidebarComponent` via downcast
            // (task 3.2). The debounce is component-owned; the shell fires
            // the wall clock via the sweep below (#609) and routes any
            // emitted `Msg::Service(SearchQuery)` through the same
            // service-request handler the keyboard path uses.
            had_events |= self.drain_search_results();

            // Search debounce sweep (#609): production never wired a
            // `UserEvent::Clock` publisher, so the shell supplies the
            // wall-clock tick directly via `tick_search_clock` once per
            // main-loop iteration. The component owns the deadline and
            // emits `Msg::Service(SearchQuery)` when it passes; the shell
            // dispatches it through `handle_service_request` (the same
            // path the keyboard arm routes Service requests through).
            if let Some(Msg::Service(request)) = self.tick_search_clock(Instant::now()) {
                had_events = true;
                self.handle_service_request(request);
            }

            // Inline Search debounce sweep: same shell-supplied wall clock,
            // pumped into the embedded control of the active session. A
            // fired debounce re-scored the results and needs a redraw.
            had_events |= self.tick_inline_search_clock(Instant::now());

            had_events |= self.app.drain_session_events();

            had_events |= self.app.expire_bare_transition(Instant::now());

            had_events |= self.app.drain_cast_events();

            // Feed results rebuild Home's Feeds pill via `FeedsLatestRebuilt`,
            // drained next loop pass; re-project for the other inputs (5.3d).
            if self.app.drain_feed_tab_results() {
                had_events = true;
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_active_emby_library_owner_content();
            }

            had_events |= self.drain_feed_add_results();

            had_events |= self.drain_card_image_completions();
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
                // `UserDataChanged` refetches Home inside the handler; re-project (5.3d).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_active_emby_library_owner_content();
                self.push_music_workspace_content();
            }

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
                    >= super::super::cast_status_actions::CAST_STATUS_POLL_INTERVAL
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
            let messages = match self.application.tick(PollStrategy::Once(poll_timeout)) {
                Ok(msgs) => msgs,
                Err(_) => break,
            };
            // ADR 0024: fold the mouse-derived messages (one per eligible
            // subscribed component) down to at most one before the keyboard
            // router fold and `handle_terminal_message` dispatch. A keyboard
            // tick passes through untouched.
            let messages = fold_mouse_messages(messages);
            if !messages.is_empty() {
                had_events = true;
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
                if let RouterOutcome::Command(command) | RouterOutcome::PrefixDispatch(command) =
                    &router
                {
                    quit |= self.dispatch_router_command(command.clone());
                }
                // A deferred candidate fires on an unhandled press; its
                // commands never quit, so it does not feed the loop's quit
                // flag.
                self.apply_deferred_candidate(&router, diagnostic.leaf_disposition == "consumed");
                for msg in messages {
                    if self.handle_terminal_message(msg, &mut music_resize, &mut tv_resize) {
                        quit = true;
                    }
                }
                if quit {
                    break 'outer;
                }
            }

            // Apply the Settings panel's live mouse-capture flip: the toggle
            // arm only records the intent; the capture sequence goes out on
            // the session stdout here, before the next draw.
            if let Some(enabled) = self.app.mouse_capture_pending.take() {
                let _ = crate::app::set_mouse_capture(terminal.backend_mut(), enabled);
            }

            // Drain deferred component intents after this tick's primary
            // messages, including ticks with no primary component message.
            if self.drain_deferred_library_message(&mut music_resize, &mut tv_resize) {
                break 'outer;
            }

            // Keep in sync with tests_tick_harness.rs, the other caller of this shared pass.
            self.sync_mounted_surfaces();

            self.app.expire_music_grouping_candidates();
            self.app.sync_volume_from_player();
            // Advance idle feed rotation
            self.app.advance_idle_feed_rotation();

            // See `render_interval`'s doc comment for the fast/slow cadence rules.
            let render_interval = self.app.render_interval();
            if self
                .app
                .wants_terminal_render(had_events, last_render, render_interval)
            {
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
                last_render = Instant::now();
            }
        }

        self.teardown(quit_timeout);
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

#[cfg(test)]
#[path = "shell_run_tests.rs"]
mod tests;
