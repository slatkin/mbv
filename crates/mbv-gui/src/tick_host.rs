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

use eframe::egui;
use mbv::{
    App, ClientLifecycle, ClientTickHost, GuiInput, GuiSurfaceLeaf, HomeEventDrain,
    PlayerEventDrain, TickOutcome,
};
use mbv_surfaces::gesture::PointerGestureState;
use mbv_ui_msg::{Msg, Position, ServiceRequest};

use crate::input::{WheelAccumulator, neutral_input};
use crate::queue::QueuePanel;

/// The `App` the Client core drives, and the Client-owned parts of the tick.
/// The GUI holds no interaction state here: cursor, selection, marks, and
/// viewport live in Surface state (design D5) — the Queue panel's Surface
/// state is `queue`.
pub(crate) struct GuiTickHost {
    app: App,
    /// The egui context this Client reads its raw input from. It is the
    /// window's context, so the input phase reads the same frame's events
    /// the window paints with (design D7).
    ctx: egui::Context,
    /// The GUI's gesture recognizer: the Client core's pointer gesture
    /// policy over the egui logical points the translated pointer events
    /// carry (`mbv_ui_msg::Position`, whose `SamePlace` rule is the core's
    /// double-click slop). The Client-owned analog of the TUI edge's
    /// per-parent recognizer (`mbv_components::mouse::gesture`).
    gestures: PointerGestureState<Position>,
    /// The Queue panel: the Queue Surface state plus the painted-geometry
    /// report the pointer gestures resolve against (row 6.4).
    queue: QueuePanel,
    /// The wheel-delta accumulator (the row 6.3 review finding): pixel-unit
    /// wheel events fold into whole steps before the gesture policy sees
    /// them.
    wheel: WheelAccumulator,
}

impl GuiTickHost {
    pub(crate) fn new(app: App, ctx: egui::Context) -> Self {
        Self {
            app,
            ctx,
            gestures: PointerGestureState::new(),
            queue: QueuePanel::new(),
            wheel: WheelAccumulator::new(),
        }
    }

    /// Push the App-owned queue projection into the Queue panel: the GUI
    /// Client's analog of the TUI Client's sync pass, once per frame before
    /// the paint (design D5).
    pub(crate) fn sync_surfaces(&mut self) {
        let projection = self.app.queue_surface_projection();
        self.queue.sync(projection);
    }

    /// Paint the frame's surfaces. The Queue panel is the whole window in
    /// the first slice, and it is the surface that holds the GUI's focus —
    /// the GUI's focus stack arrives with its later surfaces — so its
    /// appearance focus is focused.
    pub(crate) fn paint(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.queue.paint(ui.painter(), ui.max_rect(), true);
            });
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
                // This Client's Hello does not advertise `pin-swap`
                // (design D9), so the Owner never sends a swap event to it;
                // the exhaustive arm stays a documented no-op.
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
            // This Client's Hello does not advertise `pin-swap` (design
            // D9), so the Owner never sends a swap event to it; the
            // exhaustive arm stays a documented no-op.
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
        // Row 6.3: the frame's raw egui events, translated to the neutral
        // vocabulary (design D4) and routed through the shared router. The
        // pointer state the translation reads is the frame's: egui's wheel
        // event carries no position, and its movement events carry no
        // button state, so the frame's pointer facts stand in for both.
        let (events, frame_modifiers, primary_down, pointer_pos) = self.ctx.input(|input| {
            (
                input.events.clone(),
                input.modifiers,
                input.pointer.primary_down(),
                input.pointer.latest_pos(),
            )
        });
        let points_per_line = self
            .ctx
            .options(|options| options.input_options.line_scroll_speed);
        let Self {
            app,
            queue,
            gestures,
            wheel,
            ..
        } = self;
        for input in neutral_input(
            &events,
            frame_modifiers,
            primary_down,
            pointer_pos,
            wheel,
            points_per_line,
        ) {
            outcome.input_received = true;
            match input {
                GuiInput::Key(_) => {
                    if app.route_gui_input(input, Some(gui_leaf(queue))) {
                        outcome.quit_requested = true;
                    }
                }
                GuiInput::Pointer(event) => {
                    // Feed the translated pointer events through the Client
                    // core's gesture policy so the recognizer's double-click
                    // and drag state stays current across frames, then
                    // resolve the recognized gesture against the Queue
                    // panel's last painted hit map (design D6).
                    if let Some(gesture) = gestures.recognize(
                        event.kind,
                        event.modifiers,
                        event.position,
                        Instant::now(),
                    ) {
                        app.route_gui_pointer(gesture, Some(gui_leaf(queue)));
                    } else {
                        // A hover move claims nothing but still disarms
                        // prefix mode, as every pointer event does.
                        let _ = app.route_gui_input(input, None);
                    }
                }
            }
        }
    }
}

/// The focused Queue panel as the router's leaf. The Queue panel is the
/// GUI's one mounted surface, and it holds the GUI's focus; the GUI's focus
/// stack arrives with its later surfaces, which extend this decision.
fn gui_leaf(queue: &mut QueuePanel) -> &mut dyn GuiSurfaceLeaf {
    queue
}

/// Test evidence for the headless GUI tests (row 6.4): the Queue panel's
/// cursor, scroll, and last painted report.
#[cfg(test)]
impl GuiTickHost {
    pub(crate) fn queue_test_cursor(&self) -> usize {
        self.queue.test_cursor()
    }

    pub(crate) fn queue_test_mark_count(&self) -> usize {
        self.queue.test_mark_count()
    }

    pub(crate) fn queue_test_capacity(&self) -> usize {
        self.queue.last_report().capacity.whole_items()
    }

    pub(crate) fn queue_test_report(&self) -> &mbv_surfaces::queue::QueueFrameReport {
        self.queue.last_report()
    }
}
