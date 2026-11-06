//! The TUI Client's [`ClientTickHost`] (design D7 of `extract-client-core`):
//! `Model` delegates each shared phase to the existing drains and
//! projections, so [`ClientCore::tick`] runs the run loop's exact phase
//! order over the `TuiRealm` shell.

use std::time::Instant;

use mbv_ui_msg::{Msg, ServiceRequest};

use super::super::Model;
use crate::app::{App, ClientTickHost, TickOutcome};

impl ClientTickHost for Model {
    fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    fn drain_startup_workers(&mut self) -> bool {
        self.drain_startup_workers()
    }

    fn reproject_audiobookshelf_content(&mut self) {
        // ABS startup/refresh reset the browse state and reconcile
        // per-episode progress; re-project the owners the drain can have
        // changed.
        self.push_audiobookshelf_podcast_content();
        self.push_audiobookshelf_book_content();
        self.push_music_workspace_content();
        // ABS teardown, replacement, and catalog rebuilds restructure
        // `audiobookshelf_libraries` inside the App-side drain; evict
        // fetched Home rows whose podcast library is gone (review P2 of
        // Unit C). Only an actual eviction re-projects Home; once the
        // rebuild lands, the Home fetch refills the sources.
        if self.evict_gone_home_recently_added() {
            self.push_home_content();
        }
    }

    fn drain_suspended_home_events(&mut self) -> bool {
        self.drain_suspended_home_events()
    }

    fn drain_player_events(&mut self, input_received: &mut bool) -> bool {
        self.drain_player_events(input_received)
    }

    fn drain_lib_events(&mut self) -> bool {
        self.drain_lib_events()
    }

    fn drain_search_results(&mut self) -> bool {
        self.drain_search_results()
    }

    fn tick_search_clock(&mut self, now: Instant) -> Option<Msg> {
        self.tick_search_clock(now)
    }

    fn handle_service_request(&mut self, request: ServiceRequest) {
        // The run-loop call site discarded the return value; keep that.
        let _ = Model::handle_service_request(self, request);
    }

    fn tick_inline_search_clock(&mut self, now: Instant) -> bool {
        self.tick_inline_search_clock(now)
    }

    fn reproject_feed_content(&mut self) {
        // Feed results update their embedded destination owner. Feeds rows
        // are derived from `all_entries` at Home-push time (spec
        // `home-latest-sections`), so a feed refresh re-projects Home.
        self.push_active_emby_library_owner_content();
        self.push_home_content();
    }

    fn drain_feed_add_results(&mut self) -> bool {
        self.drain_feed_add_results()
    }

    fn drain_card_image_completions(&mut self) -> bool {
        self.drain_card_image_completions()
    }

    fn drain_resize_responses(&mut self) -> bool {
        self.drain_resize_responses()
    }

    fn drain_transport_events(&mut self) -> bool {
        self.drain_transport_events()
    }

    fn drain_ws_events(&mut self) -> bool {
        self.drain_ws_events()
    }

    fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        self.drain_audiobookshelf_socket_events()
    }

    fn drain_idle_feed(&mut self) -> bool {
        self.drain_idle_feed()
    }

    fn run_periodic_maintenance(&mut self) {
        self.run_periodic_maintenance();
    }

    fn tick_input(&mut self, outcome: &mut TickOutcome) {
        // The TUI's input phase is the TuiRealm listener poll, the mouse
        // fold, the keyboard router arbitration, and the message dispatch.
        if self.tick_terminal_messages(
            &mut outcome.input_received,
            &mut outcome.workspace_relayout.music,
            &mut outcome.workspace_relayout.tv,
        ) {
            outcome.quit_requested = true;
        }
    }
}
