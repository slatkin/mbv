use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use mbv_ui_msg::{Msg, ServiceRequest};

use super::{ClientCore, ClientTickHost, TickOutcome};
use crate::app::App;
use crate::app::tests::make_app_stub;

/// A `ClientTickHost` stub: every phase answers "no input" except one
/// configurable channel drain. The `App` half is the standard stub, so the
/// `App`-side drains the core calls are inert.
struct StubHost {
    app: App,
    ws_event: bool,
}

impl ClientTickHost for StubHost {
    fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }
    fn drain_startup_workers(&mut self) -> bool {
        false
    }
    fn reproject_audiobookshelf_content(&mut self) {}
    fn drain_suspended_home_events(&mut self) -> bool {
        false
    }
    fn drain_player_events(&mut self, _input_received: &mut bool) -> bool {
        false
    }
    fn drain_lib_events(&mut self) -> bool {
        false
    }
    fn drain_search_results(&mut self) -> bool {
        false
    }
    fn tick_search_clock(&mut self, _now: Instant) -> Option<Msg> {
        None
    }
    fn handle_service_request(&mut self, _request: ServiceRequest) {}
    fn tick_inline_search_clock(&mut self, _now: Instant) -> bool {
        false
    }
    fn reproject_feed_content(&mut self) {}
    fn drain_feed_add_results(&mut self) -> bool {
        false
    }
    fn drain_card_image_completions(&mut self) -> bool {
        false
    }
    fn drain_resize_responses(&mut self) -> bool {
        false
    }
    fn drain_transport_events(&mut self) -> bool {
        false
    }
    fn drain_ws_events(&mut self) -> bool {
        self.ws_event
    }
    fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        false
    }
    fn drain_idle_feed(&mut self) -> bool {
        false
    }
    fn run_periodic_maintenance(&mut self) {}
    fn tick_input(&mut self, _outcome: &mut TickOutcome) {}
}

// The waker contract (design D7): a drain that reported new input must wake
// a sleeping Client, so a drained event does not wait out the Client's idle
// cadence to paint.
#[test]
fn waker_fires_when_a_channel_drain_reports_input() {
    let fired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&fired);
    let mut core = ClientCore::new();
    core.install_waker(move || flag.store(true, Ordering::Relaxed));
    let mut host = StubHost {
        app: make_app_stub(),
        ws_event: true,
    };

    let outcome = core.tick(&mut host, Instant::now());

    assert!(outcome.input_received);
    assert!(
        fired.load(Ordering::Relaxed),
        "the installed waker must fire when a drain delivered input"
    );
}
