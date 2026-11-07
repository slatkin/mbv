use crate::app::BrowseEvent;
use crate::app::{ClientCore, TickOutcome};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::{
    IdleFeed, Model, PanelFocus, PollStrategy, QUIT_REQUESTED, init_terminal,
    install_signal_handlers, restore_terminal, start_quit_watchdog,
};
// The run-loop tests reach `App` through this module's scope.
#[cfg(test)]
use super::App;

/// Issue #940: a run-loop phase that takes at least this long gets a WARN
/// naming the phase that consumed it, so a stalled input window names its
/// blocker after the loop unblocks (a blocked phase cannot log mid-stall).
const SLOW_PHASE: Duration = Duration::from_secs(2);

fn warn_slow_phase(phase: &'static str, started: Instant) {
    let elapsed = started.elapsed();
    if elapsed >= SLOW_PHASE {
        tracing::warn!(
            name: "run_loop.slow_phase",
            target: "run_loop",
            { phase = phase, elapsed = ?elapsed },
            "run-loop phase exceeded the 2s watchdog bound"
        );
    }
}

impl Model {
    pub(crate) fn sync_mounted_surfaces(&mut self) {
        // Tasks 1.1/1.2: the draw-time state mutations run here, before any
        // draw. The launch-tab resolution and the stale-destination fallback
        // settle the active tab so every projection below -- and the frame --
        // sees the resolved tab (the sync pass no longer writes `self.tab`),
        // and the terminal-resize handling (queue-column clamp + prefs save,
        // and the mini-view focus hand-off on a real Resize event) leaves the
        // draw path, which now only reads geometry.
        self.sync_terminal_resize();
        self.app.resolve_launch_tab_on_sync();
        self.app.normalize_stale_browse_destination();
        // Apply App-owned effect handoffs to their mounted components.
        // `sync_home` was deleted:
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
        // Derived once per pass, before every workspace owner's push below.
        self.workspace_now_playing = super::now_playing::derive_workspace_now_playing(&self.app);
        self.sync_abs_book_now_playing();
        // The Music workspace re-projects every sync pass: navigation
        // landings, track-fetch arrivals, focus requests, and breakpoint
        // flips all land in App state, and the owner only picks them up
        // through this push (the re-anchor, deep-selection, and
        // track-focus one-shots are consumed here). Event writers push
        // too, but the sync pass is the backstop that direct App writers
        // (and the tests driving them) rely on.
        self.push_music_workspace_content();
        // The TV owner is installed/pushed before the panel's
        // owner-retention and active-pointer pass below.
        self.sync_tv_content();
        // The Library panel mounts with the library column and
        // drives its owner map (retention + the active pointer) before the
        // focus pass routes to the active surface.
        self.sync_library_panel();
        // Restore the selected destination's main Selector before
        // its library item, then consume the pending intent exactly once.
        self.reanchor_pending_launch_destination();
        // A landing that completed during this iteration's lib-event
        // drain owes the Series detail hand-off. Consume it here, after the
        // panel's active pointer follows the landed tab and before the hero
        // image / focus passes so they see the opened presentation.
        self.drain_series_navigation_handoff();
        // A navigated episode's workspace selection retries as its
        // series detail / season episodes drain (each retry may arm the next
        // season fetch; absence clears it silently).
        self.drain_pending_episode_selection();
        // The active owner's hero image projection —
        // the fetches and the cover-fit box re-encode — runs here, before the
        // draw, so painting reads projected state only.
        self.sync_library_hero_images();
        // Retire destination components whose Service library left the
        // catalog before the focus pass routes to the active destination.
        self.sync_active_destination();
        // Mouse eligibility is derived off the same active-destination
        // derivation, in the same pass, right after it — the focus step above
        // (design D3 of `extract-client-core`) has already applied the core
        // focus stack's derived focus to TuiRealm, and this pass's only other
        // TuiRealm focus caller is that step.
        self.sync_mouse_subscriptions();
    }

    /// Terminal-resize side effects, applied in the sync pass before any
    /// draw. A size drift against the size this pass last handled runs
    /// the former draw-time mutations: the queue-column clamp + prefs save
    /// (this also picks up the startup draw's size normalization and any
    /// direct-frame normalization). The card-image clear those mutations used
    /// to include is gone -- invariant 17. The mini-view focus hand-off runs
    /// only when the Resize observer armed it -- the real terminal-resize
    /// event, whose pre-resize width the marker still holds.
    pub(super) fn sync_terminal_resize(&mut self) {
        // A pinned width change is one `pinwin set-size` request; the new size
        // reaches mbv only as the terminal's own `Resize` observer event, which
        // this pass adopts like any other resize (invariant 17).
        let size = (self.app.terminal_width, self.app.terminal_height);
        let resize_event = std::mem::take(&mut self.pending_terminal_resize);
        if self.handled_terminal_size == size && !resize_event {
            return;
        }
        let was_wide = self.handled_terminal_size.0 >= mbv_render::layout::MINI_VIEW_THRESHOLD;
        self.handled_terminal_size = size;
        // Invariant 17: the image cache survives a resize. The painters'
        // protocol resize path re-encodes from the decoded sources at the new
        // geometry; wiping the cache here (a second clear next to the
        // observer's) only forced HTTP refetches and a longer placeholder
        // window on every pinned width change.
        // Crossing into mini view on a real resize hands focus to the queue;
        // the stored wide focus is untouched.
        if resize_event && was_wide && size.0 < mbv_render::layout::MINI_VIEW_THRESHOLD {
            self.app.mini_view_focus = PanelFocus::Queue;
        }
        if self.app.clamp_queue_column_width() {
            self.app.save_prefs();
        }
    }

    /// The sole frame orchestrator: root placement publication, mounted
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
        // App-level `blocking_overlay_active` adapter).
        self.app.dim_backdrop_active = self.blocking_overlay_active();
        self.app.compose_root_frame(f);
        // Root composition is data-driven: this is the sole panel paint loop.
        // The queue playback placement is deliberately handled by its panel
        // method; its transport geometry was projected during sync from the
        // prior-paint card checkpoint, so this draw path remains read-only.
        for placement in self.app.layout.root_frame.placements() {
            let Some(placement) = placement else { continue };
            match placement {
                mbv_render::arrangements::chrome::PanelPlacement::Tab(area) => {
                    self.render_tab_panel_at(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::Library(area) => {
                    self.render_library_panel_at(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::LibraryPlayback(area) => {
                    self.render_library_playback_panel_at(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::Queue(area) => {
                    self.render_queue_panel_at(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::QueuePlayback(area) => {
                    self.render_queue_playback_panel(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::StatusBar(area) => {
                    self.render_status_bar_panel_at(f, area);
                }
                mbv_render::arrangements::chrome::PanelPlacement::QueueBoundary(area) => {
                    self.render_queue_boundary_at(f, area);
                }
            }
        }
        self.render_overlay_stack(f);
    }

    /// Drain completed card-image fetches into the render cache, and re-project
    /// TV workspace content when a Series (`:ser:` family) key completed: the
    /// Wide push reserved its painted key and painted the placeholder, and the
    /// cached entry only reaches the screen through that re-push. The drain
    /// loop lives on the `App`, so both Clients run it once; the TUI is the
    /// Client that paints that workspace. Returns whether any completion
    /// arrived.
    pub(in crate::app) fn drain_card_image_completions(&mut self) -> bool {
        let drain = self.app.drain_card_image_completions();
        if drain.series_key_changed {
            self.push_tv_workspace_content();
        }
        drain.drained
    }

    pub(in crate::app) fn handle_restored_library_position_event(
        &mut self,
        ev: super::super::LibEvent,
    ) {
        let lib_idx = match &ev {
            super::super::LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
                lib_idx, ..
            }) => *lib_idx,
            _ => unreachable!("restore handler called with a different library event"),
        };
        self.app.handle_lib_event(ev);
        let latest = self.app.libs.get(lib_idx).and_then(|lib| {
            let level = lib.nav_stack.last()?;
            (lib.library.collection_type == "tvshows"
                && lib.nav_stack.len() == 1
                && lib.tv_content_mode == Some(mbv_queue::TvContentMode::Latest))
            .then(|| {
                (
                    lib.library.id.clone(),
                    level
                        .items
                        .iter()
                        .cloned()
                        .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                        .collect(),
                )
            })
        });
        if let Some((library_id, items)) = latest {
            let items = self
                .tv_latest_snapshots
                .get(&library_id)
                .map_or(items, |snapshot| snapshot.items.clone());
            self.update_emby_latest_snapshot(&library_id, items);
        }
    }

    /// The run loop — the moved body of the former `App::run`.
    ///
    /// Split so a failed start-up still restores the terminal: without the
    /// restore, an early error leaves the alternate screen and raw mode
    /// attached to the user's terminal, which presents as a silent black
    /// screen (and on a `--pin` launch the error line goes to the log, so
    /// nothing is visible at all).
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mouse_support = self.app.config.lock().unwrap().mouse_support;
        let mut terminal = init_terminal(mouse_support)?;
        let result = self.run_with_terminal(&mut terminal);
        if result.is_err() {
            let _ = restore_terminal(&mut terminal);
        }
        result
    }

    fn run_with_terminal(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // No `terminal.clear()` here. ratatui's `Terminal::clear` queries the
        // cursor position (`ESC[6n`) and waits for the terminal's reply on the
        // crossterm event source that the tuirealm input listener is already
        // polling concurrently (started in `Model::new`), so the reply is
        // sometimes consumed by the listener and the query times out after 2s,
        // killing the process before the first paint. The alternate screen is
        // already blank (`EnterAlternateScreen` plus the ratatui-image
        // capability probe's `ESC c` reset both precede this), and the first
        // draw below fully repaints because the back buffer starts empty.
        // Image pickers are initialised in `main` before `Model::new` starts
        // the TuiRealm listener — see `App::init_image_pickers` (#654).

        self.home_content.loading = true;
        terminal.draw(|f| self.draw_frame(f, false, false))?;

        self.spawn_startup_services();

        // Home populates now; Emby's startup merges its portion later (#543).
        self.fetch_home_at_startup();

        self.init_idle_feed();

        terminal.draw(|f| self.draw_frame(f, false, false))?;

        install_signal_handlers();
        let quit_timeout = Duration::from_secs(self.app.config.lock().unwrap().quit_timeout_secs);
        start_quit_watchdog();

        let mut last_render = Instant::now()
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(Instant::now);
        // Design D7 (`extract-client-core`): the Client core sequences the
        // App-side phases and shares the App's wake handle; the TUI Client
        // installs a no-op waker — the listener poll cadence already bounds
        // input latency.
        let client_core = ClientCore::from_app(&self.app);
        client_core.install_waker(|| {});

        'outer: loop {
            let drain_started = Instant::now();
            if QUIT_REQUESTED.load(Ordering::Relaxed) {
                break;
            }

            let mut outcome = ClientCore::tick(&mut *self, Instant::now());
            if outcome.restart_requested {
                continue 'outer;
            }
            if outcome.quit_requested {
                break 'outer;
            }
            // The drain phase includes the input tick, which logs its own
            // slow-phase WARN; a tick WARN plus this one pinpoints the tick.
            warn_slow_phase("drain", drain_started);

            if self.finish_run_iteration(terminal, &mut outcome, &mut last_render)? {
                break 'outer;
            }
        }

        self.teardown(quit_timeout);
        let _ = restore_terminal(terminal); // ignore errors — terminal may be gone (SIGHUP)
        // Printed only after the terminal is restored: anything
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
        outcome: &mut TickOutcome,
        last_render: &mut Instant,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Apply the Settings panel's live mouse-capture flip before the next draw.
        if let Some(enabled) = self.app.mouse_capture_pending.take() {
            let _ = crate::app::set_mouse_capture(terminal.backend_mut(), enabled);
        }
        // Drain deferred component intents after primary messages, including
        // ticks with no primary component message.
        if self.drain_deferred_library_message(
            &mut outcome.workspace_relayout.music,
            &mut outcome.workspace_relayout.tv,
        ) {
            return Ok(true);
        }

        // Keep in sync with tests/tick_integration/harness.rs, the other caller of this shared pass.
        let sync_started = Instant::now();
        self.sync_mounted_surfaces();
        warn_slow_phase("sync", sync_started);
        self.draw_frame_if_due(
            outcome.input_received,
            last_render,
            outcome.workspace_relayout.music,
            outcome.workspace_relayout.tv,
            terminal,
        )?;
        Ok(false)
    }
}

mod drains;
mod pin_swap;
#[cfg(test)]
mod tests;
mod tick_host;
