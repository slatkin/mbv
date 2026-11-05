//! The GUI Client's half of the shared tick (design D7 of
//! `add-gui-front-end`).
//!
//! [`ClientCore::tick`] runs the App-side drain sequence one time for every
//! Client and calls into this host for the surface phases that interleave it.
//! The surface-neutral body of each phase is the Client core's drain
//! ([`mbv::App`]); what stays here is what only a Client can know. The first
//! GUI slice presents the Queue, the Queue playback panel, and the Status bar
//! (rows 6.4 and 6.5), so every projection this host would make is a no-op
//! with the row that fills it named.

use std::fmt;
use std::time::Instant;

use mbv::{App, ClientLifecycle, ClientTickHost, HomeEventDrain, PlayerEventDrain, TickOutcome};
use mbv_ui_msg::{Msg, ServiceRequest};

/// The `App` the Client core drives, and the Client-owned parts of the tick.
/// The GUI holds no interaction state here: cursor, selection, marks, and
/// viewport live in Surface state (design D5).
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
        // The rebuilt Home content a completion carries has no surface in the
        // first slice; rows 6.4 and 6.5 present the Queue, Queue playback, and
        // Status bar.
        self.app.drain_startup_workers().events
    }

    fn reproject_audiobookshelf_content(&mut self) {
        // Rows 6.4 and 6.5 own every GUI projection; the first slice presents
        // no Audiobookshelf surface.
    }

    fn drain_suspended_home_events(&mut self) -> bool {
        let mut had_events = false;
        loop {
            match self.app.drain_home_event() {
                HomeEventDrain::Empty => return had_events,
                HomeEventDrain::Consumed => had_events = true,
                // A GUI Client holds no terminal and no panel to swap.
                HomeEventDrain::Client(
                    ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit,
                ) => {
                    had_events = true;
                }
            }
        }
    }

    fn drain_player_events(&mut self, input_received: &mut bool) -> bool {
        match self.app.drain_player_event() {
            PlayerEventDrain::Empty => false,
            // A GUI Client holds no terminal and no panel to swap.
            PlayerEventDrain::Client(ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit) => {
                *input_received = true;
                false
            }
            PlayerEventDrain::Handled { restart } => {
                // Rows 6.4 and 6.5 project the Queue, the Queue playback panel,
                // and the Status bar from this event's App state.
                *input_received = true;
                restart
            }
        }
    }

    fn drain_lib_events(&mut self) -> bool {
        let mut had_events = false;
        // The core's drain applies the App-owned half of every library event;
        // the library content state it does not keep belongs to rows that
        // present a library surface.
        while self.app.drain_lib_event() {
            had_events = true;
        }
        had_events
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
        // Terminal image protocols are a TUI Client concern; the GUI paints
        // with the egui `Painter` (rows 6.4 and 6.5).
        false
    }

    fn drain_transport_events(&mut self) -> bool {
        // The core drains it; rows 6.4 and 6.5 project the state it changes.
        self.app.drain_transport_events()
    }

    fn drain_ws_events(&mut self) -> bool {
        let mut had_events = false;
        // The core handles each event; the content it changes belongs to rows
        // that present a library surface.
        while self.app.drain_ws_event() {
            had_events = true;
        }
        had_events
    }

    fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        let mut had_events = false;
        while self.app.drain_audiobookshelf_socket_event() {
            had_events = true;
        }
        had_events
    }

    fn drain_idle_feed(&mut self) -> bool {
        // The core drains it; the idle feed has no surface in the first slice.
        self.app.drain_idle_feed()
    }

    fn run_periodic_maintenance(&mut self) {
        self.app.run_periodic_maintenance();
    }

    fn tick_input(&mut self, outcome: &mut TickOutcome) {
        // Row 6.3 fills this: egui key and pointer events translated to the
        // neutral types and routed through the shared router.
        let _ = outcome;
    }
}
