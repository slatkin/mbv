//! The GUI Client's half of the shared tick (design D7 of
//! `add-gui-front-end`).
//!
//! [`ClientCore::tick`] runs the App-side drain sequence one time for every
//! Client and calls into this host for the surface phases that interleave it.
//! The first GUI slice presents the Queue, the Queue playback panel, and the
//! Status bar (rows 6.4 and 6.5), so every phase that feeds another surface
//! answers with a no-op projection; the comment names the row that fills the
//! phase it sits on.
//!
//! The drains these phases call are TUI-shell code today (`shell::run`), not
//! `App` entry points, so the GUI cannot reach them from outside the library
//! yet. Rows 6.4 and 6.5 need the ones the slice reads — see the unit
//! report's open question.

use std::fmt;
use std::time::Instant;

use mbv::{App, ClientTickHost, TickOutcome};
use mbv_ui_msg::{Msg, ServiceRequest};

/// The `App` the Client core drives, and the surface phases it calls back
/// into. The GUI holds no interaction state here: cursor, selection, marks,
/// and viewport live in Surface state (design D5).
pub(crate) struct GuiTickHost {
    app: App,
}

impl GuiTickHost {
    pub(crate) fn new(app: App) -> Self {
        Self { app }
    }
}

impl fmt::Debug for GuiTickHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `App` is not `Debug`; the host adds no state of its own to print.
        f.debug_struct("GuiTickHost").finish_non_exhaustive()
    }
}

impl ClientTickHost for GuiTickHost {
    fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    fn drain_startup_workers(&mut self) -> bool {
        // Library content is outside the first slice (rows 6.4 and 6.5
        // present the Queue, Queue playback, and Status bar).
        false
    }

    fn reproject_audiobookshelf_content(&mut self) {
        // Rows 6.4 and 6.5 own every GUI projection; the first slice presents
        // no Audiobookshelf surface.
    }

    fn drain_suspended_home_events(&mut self) -> bool {
        // Home is outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn drain_player_events(&mut self, input_received: &mut bool) -> bool {
        // Row 6.4 fills this: the Queue slice adopts Owner snapshots here.
        let _ = input_received;
        false
    }

    fn drain_lib_events(&mut self) -> bool {
        // Library content is outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn drain_search_results(&mut self) -> bool {
        // Search is outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn tick_search_clock(&mut self, now: Instant) -> Option<Msg> {
        // Search is outside the first slice (rows 6.4 and 6.5).
        let _ = now;
        None
    }

    fn handle_service_request(&mut self, request: ServiceRequest) {
        // Row 6.3 routes the keys that reach a service; the first slice
        // presents no search surface to emit one.
        let _ = request;
    }

    fn tick_inline_search_clock(&mut self, now: Instant) -> bool {
        // Inline search is outside the first slice (rows 6.4 and 6.5).
        let _ = now;
        false
    }

    fn reproject_feed_content(&mut self) {
        // Feeds are outside the first slice (rows 6.4 and 6.5).
    }

    fn drain_feed_add_results(&mut self) -> bool {
        // Feeds are outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn drain_card_image_completions(&mut self) -> bool {
        // Artwork is outside the first slice: rows 6.4 and 6.5 paint no
        // images.
        false
    }

    fn drain_resize_responses(&mut self) -> bool {
        // Artwork is outside the first slice: rows 6.4 and 6.5 paint no
        // images.
        false
    }

    fn drain_transport_events(&mut self) -> bool {
        // Row 6.4 fills this: the Queue slice reads queue and transport state
        // from the ctrl transport drain.
        false
    }

    fn drain_ws_events(&mut self) -> bool {
        // The Emby websocket feeds library and Home state, outside the first
        // slice (rows 6.4 and 6.5).
        false
    }

    fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        // Audiobookshelf state is outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn drain_idle_feed(&mut self) -> bool {
        // The idle feed is outside the first slice (rows 6.4 and 6.5).
        false
    }

    fn run_periodic_maintenance(&mut self) {
        // The session polls and the websocket keepalive are App-side work the
        // TUI shell owns today, beside the drains above.
    }

    fn tick_input(&mut self, outcome: &mut TickOutcome) {
        // Row 6.3 fills this: egui key and pointer events translated to the
        // neutral types and routed through the shared router.
        let _ = outcome;
    }
}
