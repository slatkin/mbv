use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::{
    init_terminal, install_signal_handlers, restore_terminal, service_startup, start_quit_watchdog,
    IdleFeed, Model, Msg, PanelFocus, PollStrategy, QUIT_REQUESTED,
};
// The run-loop tests reach `App` through this module's scope.
#[cfg(test)]
use super::App;
use crate::app::images::SERIES_IMAGE_CACHE_KEY_INFIX;

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
        // Flat TV episodes use the existing Library Hero overlay only in
        // compact mini view; this must run after the panel's active owner
        // hand-off above.
        self.sync_tv_mini_view_hero();
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
        self.app.images.card_image_states.clear();
        self.app.images.card_image_loading.clear();
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
                    self.render_tab_panel_at(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::Library(area) => {
                    self.render_library_panel_at(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::LibraryPlayback(area) => {
                    self.render_library_playback_panel_at(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::Queue(area) => {
                    self.render_queue_panel_at(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::QueuePlayback(area) => {
                    self.render_queue_playback_panel(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::StatusBar(area) => {
                    self.render_status_bar_panel_at(f, area);
                }
                crate::app::render::arrangements::chrome::PanelPlacement::QueueBoundary(area) => {
                    self.render_queue_boundary_at(f, area);
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
    fn evict_excess_card_images(&mut self) {
        while self.app.images.image_lru.len() > self.app.images.cache_size_total {
            let Some(evict) = self.app.images.image_lru.pop_front() else {
                break;
            };
            self.app.images.card_image_states.remove(&evict);
        }
    }

    pub(in crate::app) fn drain_card_image_completions(&mut self) -> bool {
        let mut series_image_changed = false;
        let mut drained = false;
        while let Ok((cache_key, img_opt)) = self.app.images.card_image_rx.try_recv() {
            drained = true;
            series_image_changed |= cache_key.contains(SERIES_IMAGE_CACHE_KEY_INFIX);
            self.app.images.card_image_loading.remove(&cache_key);
            self.app.images.image_fetches_active =
                self.app.images.image_fetches_active.saturating_sub(1);
            let entry = self.app.images.build_cached_image(
                &cache_key,
                img_opt,
                self.app.current_protocol_suffix(),
            );
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
                self.app.images.image_lru.retain(|k| k != &cache_key);
                self.app.images.image_lru.push_back(cache_key.clone());
                self.evict_excess_card_images();
            }
            self.app.images.card_image_states.insert(cache_key, entry);
            if let Some(event) = artist_completion {
                let _ = self.app.channels.lib_tx.send(event);
            }
        }
        if series_image_changed {
            self.push_tv_workspace_content();
        }
        drained
    }

    pub(in crate::app) fn handle_restored_library_position_event(
        &mut self,
        ev: super::super::LibEvent,
    ) {
        let lib_idx = match &ev {
            super::super::LibEvent::RestoreLibraryPosition { lib_idx, .. } => *lib_idx,
            _ => unreachable!("restore handler called with a different library event"),
        };
        self.app.handle_lib_event(ev);
        let latest = self.app.libs.get(lib_idx).and_then(|lib| {
            let level = lib.nav_stack.last()?;
            (lib.library.collection_type == "tvshows"
                && lib.nav_stack.len() == 1
                && lib.tv_content_mode == Some(mbv_core::config::TvContentMode::Latest))
            .then(|| {
                (
                    lib.library.id.clone(),
                    lib.library.name.clone(),
                    level
                        .items
                        .iter()
                        .cloned()
                        .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                        .collect(),
                )
            })
        });
        if let Some((library_id, title, items)) = latest {
            let items = self
                .tv_latest_snapshots
                .get(&library_id)
                .map_or(items, |snapshot| snapshot.items.clone());
            self.update_emby_latest_snapshot(&library_id, title, items);
        }
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
            self.app.status = self.app.emby_client().map_or_else(
                || service_startup::startup_status(self.app.emby_runtime.state).into(),
                |_| "Loading...".into(),
            );
        }
        self.home_content.loading = true;
        terminal.draw(|f| self.draw_frame(f, false, false))?;

        self.spawn_startup_services();

        // Home populates now; Emby's startup merges its portion later (#543).
        self.fetch_home_at_startup();
        self.app.restore_queue_state();

        self.init_idle_feed();

        terminal.draw(|f| self.draw_frame(f, false, false))?;

        install_signal_handlers();
        let quit_timeout = Duration::from_secs(self.app.config.lock().unwrap().quit_timeout_secs);
        start_quit_watchdog(self.app.player.quit_handle(), quit_timeout);

        let mut last_render = Instant::now()
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(Instant::now);

        'outer: loop {
            let mut had_events = false;
            let mut music_resize = false;
            let mut tv_resize = false;
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }

            had_events |= self.drain_startup_workers();
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
            if self.drain_player_events(&mut had_events) {
                continue 'outer;
            }
            if self.drain_iteration_work(&mut had_events, &mut music_resize, &mut tv_resize) {
                break 'outer;
            }

            if self.finish_run_iteration(
                &mut terminal,
                had_events,
                &mut last_render,
                music_resize,
                tv_resize,
            )? {
                break 'outer;
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

    fn finish_run_iteration(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        had_events: bool,
        last_render: &mut Instant,
        mut music_resize: bool,
        mut tv_resize: bool,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Apply the Settings panel's live mouse-capture flip before the next draw.
        if let Some(enabled) = self.app.mouse_capture_pending.take() {
            let _ = crate::app::set_mouse_capture(terminal.backend_mut(), enabled);
        }
        // Drain deferred component intents after primary messages, including
        // ticks with no primary component message.
        if self.drain_deferred_library_message(&mut music_resize, &mut tv_resize) {
            return Ok(true);
        }

        // Keep in sync with tests/tick_integration/harness.rs, the other caller of this shared pass.
        self.sync_mounted_surfaces();
        self.app.expire_music_grouping_candidates();
        self.app.sync_volume_from_player();
        self.app.advance_idle_feed_rotation();
        self.draw_frame_if_due(had_events, last_render, music_resize, tv_resize, terminal)?;
        Ok(false)
    }

    fn drain_iteration_work(
        &mut self,
        had_events: &mut bool,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        *had_events |= self.app.drain_notif_actions();
        *had_events |= self.drain_lib_events();

        // Search results drain: the shell drains `search_rx` and writes each
        // result into the SearchSidebarComponent. The debounce is component-
        // owned; the shell fires the wall clock via the sweep below (#609)
        // and routes any emitted request through the normal service handler.
        *had_events |= self.drain_search_results();

        // The shell supplies the wall-clock tick directly once per loop.
        if let Some(Msg::Service(request)) = self.tick_search_clock(Instant::now()) {
            *had_events = true;
            self.handle_service_request(request);
        }
        *had_events |= self.tick_inline_search_clock(Instant::now());

        *had_events |= self.app.drain_session_events();
        *had_events |= self.app.expire_bare_transition(Instant::now());
        *had_events |= self.app.drain_cast_events();

        // Feed results update their embedded destination owner.
        if self.app.drain_feed_tab_results() {
            *had_events = true;
            self.push_active_emby_library_owner_content();
        }

        *had_events |= self.drain_feed_add_results();
        *had_events |= self.drain_card_image_completions();
        self.app.drain_image_fetches();
        *had_events |= self.drain_resize_responses();
        *had_events |= self.drain_ws_events();
        *had_events |= self.drain_audiobookshelf_socket_events();
        *had_events |= self.drain_idle_feed();
        self.app.sync_visualizer();
        self.run_periodic_maintenance();
        self.tick_terminal_messages(had_events, music_resize, tv_resize)
    }
}

mod drains;
#[cfg(test)]
mod tests;
