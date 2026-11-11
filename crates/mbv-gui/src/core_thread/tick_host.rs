//! The core thread's [`ClientTickHost`] (design D3): the drain phases and
//! the input phase `ClientCore::tick` drives each step. The first slice
//! mounts only the Queue, the Queue playback band, and the Status bar, so
//! most surfaces' drains stay no-ops while the App-owned halves apply.

use std::time::Instant;

use mbv::{App, ClientLifecycle, ClientTickHost, HomeEventDrain, PlayerEventDrain, TickOutcome};
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Msg, ServiceRequest};

use super::{CoreInput, CoreThread};

/// The Panel a Library command moves focus to (design D4): the two take-focus
/// commands move it on the core's `FocusStack` too, so the frame's focus and
/// the App's key gates agree. Every other command moves no focus — a
/// documented no-op, not a hidden one.
fn focus_target(command: &mbv::GuiLibraryCommand) -> Option<PanelFocus> {
    match command {
        mbv::GuiLibraryCommand::TakeFocus => Some(PanelFocus::Library),
        mbv::GuiLibraryCommand::TakeQueueFocus => Some(PanelFocus::Queue),
        _ => None,
    }
}

impl ClientTickHost for CoreThread {
    fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    fn drain_startup_workers(&mut self) -> bool {
        // The GUI presents no Home panel; a completion's rebuilt Home content
        // lands in the App's stored Home content (row 6.1), where the Home
        // projection of row 6.2 reads it. A startup this drain reports starts
        // the playback-target discovery the
        // full Emby startup itself never starts (add-gui-library-panel design
        // D11), exactly as the deleted client fill did on its completion.
        let drain = self.app.drain_startup_workers();
        if drain.emby_started {
            self.app.start_playback_target_discovery();
        }
        // A landed catalog can complete the saved launch location's first
        // phase (add-gui-library-panel design D9).
        self.restore_launch_location();
        drain.events
    }

    fn reproject_audiobookshelf_content(&mut self) {
        // The first slice presents no Audiobookshelf surface; the App-side
        // drain already applied the event's semantics.
    }

    fn drain_suspended_home_events(&mut self) -> bool {
        let mut had_events = false;
        loop {
            match self.app.drain_home_event() {
                HomeEventDrain::Empty => return had_events,
                // The GUI Client never joins the Pin swap —
                // `declare_gui_client` sends a plain Hello — so the
                // lifecycle arms consume their event with no action.
                HomeEventDrain::Consumed
                | HomeEventDrain::Client(
                    ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit,
                ) => had_events = true,
            }
        }
    }

    fn drain_player_events(&mut self, input_received: &mut bool) -> bool {
        match self.app.drain_player_event() {
            PlayerEventDrain::Empty => false,
            // The GUI Client never joins the Pin swap (see the home drain).
            PlayerEventDrain::Client(ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit) => {
                *input_received = true;
                false
            }
            PlayerEventDrain::Handled { restart } => {
                // The snapshot adoption ran App-side; the next frame's Queue
                // sync re-projects the adopted state (design D5).
                *input_received = true;
                restart
            }
        }
    }

    fn drain_lib_events(&mut self) -> bool {
        // The App-owned half applies, including the fetched `Recently added`
        // rows the arm accepts into the App's store (row 6.2), so the GUI's
        // next tick reads them from the App like the TUI shell does. The GUI
        // keeps no library content state, so no surface re-projection joins
        // the drain.
        let mut had_events = false;
        while self.app.drain_lib_event() {
            had_events = true;
        }
        // A landed group list can complete the saved launch location's second
        // phase (add-gui-library-panel design D9).
        self.restore_launch_location();
        had_events
    }

    fn drain_search_results(&mut self) -> bool {
        // The GUI mounts no search surface, so no component receives the
        // search worker's result (add-gui-library-panel design D12, row 12.1).
        // The top bar's own field reads the App store this App-side drain
        // fills, and the frame carries it; a landed result reports changed so
        // the next frame paints it.
        self.app.drain_gui_search_results()
    }

    fn tick_search_clock(&mut self, _now: Instant) -> Option<Msg> {
        // No search control holds a debounce deadline in the first slice.
        None
    }

    fn handle_service_request(&mut self, _request: ServiceRequest) {
        // Unreachable while `tick_search_clock` emits nothing: the search
        // debounce is the only Service request the core routes.
    }

    fn tick_inline_search_clock(&mut self, _now: Instant) -> bool {
        // The first slice presents no Inline Search control.
        false
    }

    fn reproject_feed_content(&mut self) {
        // The first slice presents no feed surface.
    }

    fn drain_feed_add_results(&mut self) -> bool {
        // The GUI opens no feed-add flow, so nothing sends on its channel.
        false
    }

    fn drain_card_image_completions(&mut self) -> bool {
        // The visual slot's artwork paints now (design D10): the slot's
        // fetches resolve through this drain, so a completion's decoded
        // image lands in the cache and the next frame carries it. The GUI
        // paints no TV workspace, so a Series key's re-push is the TUI's.
        self.app.drain_card_image_completions().drained
    }

    fn drain_resize_responses(&mut self) -> bool {
        // The GUI issues no image resize request (the TUI's resize drag owns
        // them), so the response channel stays empty.
        false
    }

    fn drain_transport_events(&mut self) -> bool {
        self.app.drain_transport_events()
    }

    fn drain_ws_events(&mut self) -> bool {
        let mut had_events = false;
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
        // The GUI never initializes the idle feed; the App-side drain is a
        // no-op without one.
        self.app.drain_idle_feed()
    }

    fn run_periodic_maintenance(&mut self) {
        self.app.run_periodic_maintenance();
    }

    fn tick_input(&mut self, outcome: &mut TickOutcome) {
        // Take the collected inputs out first: the routing borrows the
        // whole `self`, which the drain's field borrow would conflict with.
        for input in std::mem::take(&mut self.pending) {
            match input {
                CoreInput::Wake => outcome.input_received = true,
                CoreInput::Key(key) => {
                    // The chord routes through `App::route_gui_input` with
                    // the focused surface as the leaf (design D4); a routed
                    // quit ends the run.
                    outcome.input_received = true;
                    if self.route_key(key) {
                        outcome.quit_requested = true;
                    }
                }
                CoreInput::Pointer(event) => {
                    // The gesture resolves against the slice hit map and
                    // routes through `App::route_gui_pointer` with the
                    // surface that owns the region (design D4). A press on
                    // the daemon-lost dialog's quit choice ends the run
                    // through the same quit edge a routed quit key uses.
                    outcome.input_received = true;
                    if self.route_pointer(event) {
                        outcome.quit_requested = true;
                    }
                }
                CoreInput::Library(command) => {
                    // The Library panel's request (add-gui-library-panel design
                    // D2): the App applies the semantic intent, and the
                    // take-focus variants then move the core's `FocusStack`
                    // with the App's stored focus (design D4), so the frame
                    // and the App's key gates agree.
                    outcome.input_received = true;
                    let focus_target = focus_target(&command);
                    self.app.apply_gui_library_command(command);
                    if let Some(panel) = focus_target {
                        // The take-focus may run in the pre-catalog window
                        // (design D9): the restore then leaves the taken
                        // focus alone.
                        self.note_launch_focus_taken();
                        self.focus.set_panel_focus(panel);
                    }
                }
                CoreInput::Layout(report) => {
                    // A `report_layout` always emits the current frame,
                    // changed or not (design D3), and re-arms the theme: the
                    // front end sends the report when it mounts, so the
                    // frame after a reload carries the theme again (design
                    // D7). The report applies to the capacity and the hit
                    // map first.
                    self.apply_layout(&report);
                    self.frame_requested = true;
                    self.theme_pending = true;
                    // The reload forgets the front end's artwork too: the
                    // next frame re-carries it, encode-free (design D10).
                    self.artwork.rearm();
                    // The reloaded window holds no Library content either
                    // (add-gui-library-panel design D2): the next frame
                    // re-carries it under the same revision.
                    self.library.rearm();
                    // The reloaded window holds no trickplay sheet either
                    // (task 6.7): the next frame re-carries the hovered one.
                    self.trickplay.rearm();
                }
                CoreInput::Close => {
                    // The window closed (design D9): the run ends as a quit
                    // does, so the same edge runs — a no-op on the window
                    // that is already gone — and the run's exit boundary
                    // tears the shared state down before the App's drop
                    // releases the Client.
                    outcome.quit_requested = true;
                    break;
                }
            }
        }
        // The sidebar requests' drain (`add-gui-settings-screen` design D2):
        // once per tick, after input routing — a request the routing raised
        // drives the Settings screen this tick, and the Settings id's stack
        // membership follows the App's state.
        self.drain_pending_overlay(outcome);
    }
}
