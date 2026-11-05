//! The Client core tick (design D7 of `add-gui-front-end`): the `App`-side
//! phases of the run loop — runtime and channel drains, Owner snapshot
//! adoption, effects, and expiries — sequenced one time for every Client.
//!
//! `ClientCore` holds no terminal, `TuiRealm`, or GUI-toolkit type. Each
//! Client supplies its own half of the tick through [`ClientTickHost`]: the
//! TUI Client's `Model` delegates to the shell drains and projections in
//! `shell::run`; the GUI Client implements the same phases over its own
//! surfaces.

use std::time::Instant;

use mbv_ui_msg::{Msg, ServiceRequest};

use super::App;

/// What one [`ClientCore::tick`] hands back to the Client's run step. Each
/// field names what the tick observed, not how a Client uses it.
#[derive(Debug, Default, Clone, Copy)]
pub struct TickOutcome {
    /// A worker, a channel, or the Client's own input delivered something
    /// this tick. The Client uses it to decide whether a repaint is due.
    pub input_received: bool,
    /// Playback asked the loop to restart the iteration immediately,
    /// skipping the rest of this tick's work: the Owner snapshot the event
    /// carried already landed, and the next iteration re-reads it.
    pub restart_requested: bool,
    /// The Client's input phase asked to end the run — a quit request or a
    /// lost input source.
    pub quit_requested: bool,
    /// The workspaces whose layouts went stale this tick — a resize or a
    /// content change invalidated them.
    pub workspace_relayout: WorkspaceRelayout,
}

/// Per-workspace layout-staleness flags carried by [`TickOutcome`].
#[derive(Debug, Default, Clone, Copy)]
pub struct WorkspaceRelayout {
    /// The Music workspace's layout is stale for this frame.
    pub music: bool,
    /// The TV workspace's layout is stale for this frame.
    pub tv: bool,
}

/// The Client half of the shared tick: the surface and input work that
/// interleaves the `App`-side drains. [`ClientCore::tick`] calls these in
/// run-loop order; each Client implements them over its own surfaces.
///
/// The TUI Client implements it for `Model` (see `shell::run::tick_host`),
/// delegating to the existing shell drains and projections. A Client that
/// does not present a surface still drains that surface's events and
/// answers with a no-op projection — the `App`-side sequence runs once, in
/// the core.
pub trait ClientTickHost {
    /// The shared runtime state the drains read and mutate.
    fn app_mut(&mut self) -> &mut App;

    /// Drain the startup-worker completion channels; `true` when a
    /// completion or a worker disconnect landed.
    fn drain_startup_workers(&mut self) -> bool;

    /// Re-project the surfaces an Audiobookshelf event drain can change.
    /// Runs only when the drain reported an event.
    fn reproject_audiobookshelf_content(&mut self);

    /// Drain the suspended home link's events — including its Owner
    /// snapshot adoption — and report whether any arrived.
    fn drain_suspended_home_events(&mut self) -> bool;

    /// Drain one Owner player event — the snapshot-adoption phase — and
    /// report whether playback asked the loop to restart the iteration.
    fn drain_player_events(&mut self, input_received: &mut bool) -> bool;

    /// Drain pending library events and re-project the owners each event
    /// touches; `true` when any event arrived.
    fn drain_lib_events(&mut self) -> bool;

    /// Drain search-worker results into the search surface; `true` when
    /// any arrived.
    fn drain_search_results(&mut self) -> bool;

    /// Fire the wall-clock `now` into the mounted search control; returns
    /// the `Msg` it emits when its debounce deadline passed.
    fn tick_search_clock(&mut self, now: Instant) -> Option<Msg>;

    /// Route the `ServiceRequest` a search-clock deadline emitted.
    fn handle_service_request(&mut self, request: ServiceRequest);

    /// Fire the wall-clock `now` into the Inline Search control; `true`
    /// when the sweep emitted work.
    fn tick_inline_search_clock(&mut self, now: Instant) -> bool;

    /// Re-project the surfaces a feed-tab refresh changes.
    fn reproject_feed_content(&mut self);

    /// Drain feed-add results; `true` when any arrived.
    fn drain_feed_add_results(&mut self) -> bool;

    /// Drain card-image completions into the image cache; `true` when any
    /// arrived.
    fn drain_card_image_completions(&mut self) -> bool;

    /// Drain image-resize worker responses; `true` when any arrived.
    fn drain_resize_responses(&mut self) -> bool;

    /// Drain ctrl transport commands; `true` when any arrived.
    fn drain_transport_events(&mut self) -> bool;

    /// Drain Emby websocket events; `true` when any arrived.
    fn drain_ws_events(&mut self) -> bool;

    /// Drain Audiobookshelf socket events; `true` when any arrived.
    fn drain_audiobookshelf_socket_events(&mut self) -> bool;

    /// Drain idle-feed items and run its refetch cadence; `true` when an
    /// item batch arrived.
    fn drain_idle_feed(&mut self) -> bool;

    /// Periodic maintenance between ticks: the delayed settings save,
    /// session polls, the websocket keepalive, and the capability refresh.
    fn run_periodic_maintenance(&mut self);

    /// The Client's input phase: poll the raw input source, resolve the
    /// event, and dispatch the resulting messages. Sets `input_received`
    /// when an event arrived, `quit_requested` when the input source asked
    /// to end the run, and the workspace relayout flags the dispatched
    /// messages armed.
    ///
    /// The TUI Client runs the `TuiRealm` listener poll here.
    fn tick_input(&mut self, outcome: &mut TickOutcome);
}

/// The shared run-loop sequencer every Client drives once per iteration.
/// It holds no `App` or surface state — the [`ClientTickHost`] supplies
/// those — so the same `tick` serves the TUI Client's shell and the GUI
/// Client's eframe `update`.
pub struct ClientCore {
    /// Invoked after a tick that observed new input, so a sleeping Client
    /// wakes instead of waiting out its idle cadence. `install_waker`
    /// replaces it; the TUI Client keeps the no-op default — its listener
    /// poll already bounds input latency.
    waker: Box<dyn Fn() + Send + Sync>,
}

impl ClientCore {
    /// A core whose waker does nothing — the TUI Client's choice.
    pub fn new() -> Self {
        Self {
            waker: Box::new(|| {}),
        }
    }

    /// Install the callback `tick` invokes when worker or channel input
    /// arrived this iteration. The GUI Client installs
    /// `egui::Context::request_repaint`.
    pub fn install_waker(&mut self, waker: impl Fn() + Send + Sync + 'static) {
        self.waker = Box::new(waker);
    }

    /// Run one iteration of the shared phases in run-loop order. `now` is
    /// the wall clock the phase clocks fire with.
    pub fn tick(&self, host: &mut dyn ClientTickHost, now: Instant) -> TickOutcome {
        let mut outcome = TickOutcome::default();

        outcome.input_received |= host.drain_startup_workers();
        let drained_audiobookshelf = host.app_mut().drain_audiobookshelf_events();
        outcome.input_received |= drained_audiobookshelf;
        if drained_audiobookshelf {
            host.reproject_audiobookshelf_content();
        }
        outcome.input_received |= host.drain_suspended_home_events();
        if host.drain_player_events(&mut outcome.input_received) {
            outcome.restart_requested = true;
        } else {
            // Pending queue transfer settlement: the same tick phase that
            // adopted Owner snapshots above is the first point where the
            // target's post-replacement state is observable.
            outcome.input_received |= host.app_mut().settle_pending_queue_transfer();

            outcome.input_received |= host.app_mut().drain_notif_actions();
            outcome.input_received |= host.drain_lib_events();
            outcome.input_received |= host.drain_search_results();
            // The search debounce is control-owned; the core fires the wall
            // clock once per iteration and routes an emitted request
            // through the service handler.
            if let Some(Msg::Service(request)) = host.tick_search_clock(now) {
                outcome.input_received = true;
                host.handle_service_request(request);
            }
            outcome.input_received |= host.tick_inline_search_clock(now);
            outcome.input_received |= host.app_mut().drain_session_events();
            outcome.input_received |= host.app_mut().drain_cast_events();
            outcome.input_received |= host.app_mut().drain_pinned_requests();
            if host.app_mut().drain_feed_tab_results() {
                outcome.input_received = true;
                host.reproject_feed_content();
            }
            outcome.input_received |= host.drain_feed_add_results();
            outcome.input_received |= host.drain_card_image_completions();
            host.app_mut().drain_image_fetches();
            outcome.input_received |= host.drain_resize_responses();
            outcome.input_received |= host.drain_transport_events();
            outcome.input_received |= host.drain_ws_events();
            outcome.input_received |= host.drain_audiobookshelf_socket_events();
            outcome.input_received |= host.drain_idle_feed();
            host.app_mut().sync_visualizer();
            host.run_periodic_maintenance();

            // The Client's input phase sits inside the drain sequence, at
            // the position it held in the legacy loop: after the channel
            // drains and maintenance, before the finish phase.
            host.tick_input(&mut outcome);

            host.app_mut().expire_music_grouping_candidates();
            host.app_mut().sync_volume_from_player();
            host.app_mut().advance_idle_feed_rotation();
        }
        if outcome.input_received {
            (self.waker)();
        }
        outcome
    }
}

impl Default for ClientCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
