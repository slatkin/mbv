//! The sidebar requests' GUI drain (`add-gui-settings-screen` design D2):
//! the core thread takes `App::pending_overlay` once per tick, after input
//! routing, and matches it exhaustively — one documented arm per variant, no
//! wildcard. The open actions drive the Settings screen's core state; every
//! TUI overlay request the GUI does not present logs
//! `gui.overlay_request.undelivered` at debug level and drops.
//!
//! The Settings screen's UI reset lives here too (design D10): its steps span
//! the App and this thread's own state, so the command runs them in one place.

use mbv::{GuiSettingsGroup, GuiSettingsSection, TickOutcome};
use mbv_ui_model::overlay::{OverlayRequest, SidebarId};
use mbv_ui_model::settings::SettingsDestination;
use mbv_ui_msg::{ComponentId, OverlayId};

use super::CoreThread;
use super::launch_restore::GuiLaunchRestore;

/// The debug log the dropped variants share (design D2): the request names a
/// TUI overlay the GUI does not present.
fn log_undelivered(request: &OverlayRequest) {
    tracing::debug!(
        name: "gui.overlay_request.undelivered",
        target: "gui_core",
        request = ?request,
        "the GUI presents no overlay for the shell request"
    );
}

impl CoreThread {
    /// The GUI's UI reset (`add-gui-settings-screen` design D10). The steps run
    /// in the design's order: the pending saved-launch restore is abandoned and
    /// Home is selected with the Library store's selections cleared, the Queue
    /// Surface state's cursor, marks, and viewport reset, the overlays and the
    /// Settings screen close, the saved launch location is deleted after every
    /// live write, and the reset counter rises last so the window content
    /// resets its stores after the core has.
    pub(super) fn reset_gui_ui_state(&mut self) {
        // Step 1: abandon the saved launch restore, then Home.
        self.launch_restore = GuiLaunchRestore::Done;
        self.launch_focus_taken = false;
        self.app.reset_gui_ui_state();
        // Step 3's core half: the Queue Surface's cursor, marks, and viewport.
        // The queue's contents and the viewed link stay.
        self.queue.reset_presentation();
        // Steps 4 to 6: the overlays and the Settings screen already closed
        // with the App's steps; the saved launch location goes, then the
        // counter rises.
        self.app.clear_gui_launch_state();
        self.app.bump_gui_ui_reset_epoch();
    }
    /// Drain the App's pending shell request once per tick (design D2), then
    /// mirror the Settings screen's state onto the core `FocusStack`: opening
    /// pushes `ComponentId::Overlay(OverlayId::Settings)`, closing pops it, so
    /// keys route with no leaf while the screen shows and the Panel focus
    /// returns on the pop. The GUI adds no `GuiSurfaceLeaf` for the id.
    pub(super) fn drain_pending_overlay(&mut self, outcome: &mut TickOutcome) {
        if let Some(request) = self.app.take_pending_overlay() {
            outcome.input_received = true;
            match &request {
                OverlayRequest::ToggleSidebar(sidebar) => match sidebar {
                    // F2 toggles the F2 group — every section except the two
                    // sidebars' own (design D2, the spec's open-action rule).
                    SidebarId::Settings => {
                        self.app.toggle_gui_settings(GuiSettingsGroup::F2);
                    }
                    // F3 and F4 toggle their own sections.
                    SidebarId::Sessions => {
                        self.app.toggle_gui_settings(GuiSettingsGroup::Sessions);
                    }
                    SidebarId::Playlists => {
                        self.app.toggle_gui_settings(GuiSettingsGroup::Playlists);
                    }
                    // Search has no toggle request path: its open raises
                    // `OpenSidebar(Search)` below, which the GUI drops too.
                    SidebarId::Search => log_undelivered(&request),
                },
                OverlayRequest::OpenSidebar(sidebar) => match sidebar {
                    // The first-start open and the Services and Keys rows
                    // reach the GUI here: the section the App's
                    // `settings_destination` names opens (design D2).
                    SidebarId::Settings => {
                        let section = match self.app.gui_settings_destination() {
                            SettingsDestination::Services | SettingsDestination::Main => {
                                GuiSettingsSection::Services
                            }
                            SettingsDestination::Keys => GuiSettingsSection::Keys,
                            // The Panel page is TUI state (design D4); the
                            // GUI maps it to Services.
                            SettingsDestination::Panel => GuiSettingsSection::Services,
                        };
                        self.app.open_gui_settings_at(section);
                    }
                    // The GUI presents no Search sidebar, and the two other
                    // ids never raise an open request.
                    SidebarId::Sessions | SidebarId::Playlists | SidebarId::Search => {
                        log_undelivered(&request);
                    }
                },
                OverlayRequest::DismissSidebar(sidebar) => match sidebar {
                    // The playlist play's `ReplacementExecutor` raises this
                    // after the queue replaces (design D2): the screen closes
                    // while it shows Playlists, so the replaced queue shows.
                    SidebarId::Playlists => {
                        if self
                            .app
                            .gui_settings()
                            .is_some_and(|state| state.section == GuiSettingsSection::Playlists)
                        {
                            self.app.close_gui_settings();
                        }
                    }
                    // The GUI's Sessions and Settings close through the
                    // screen's own commands, and no Search dismissal exists.
                    SidebarId::Settings | SidebarId::Sessions | SidebarId::Search => {
                        log_undelivered(&request);
                    }
                },
                // The GUI presented the confirmation and the daemon-lost
                // dialog as their own overlays at raise time (`ask_confirm`
                // and `raise_daemon_lost_modal`); the TUI-modal requests
                // themselves are undelivered (design D2).
                OverlayRequest::Confirm(_) | OverlayRequest::DaemonLost(_) => {
                    log_undelivered(&request);
                }
                // A TUI overlay the GUI does not present (design D2): the
                // save-playlist and confirm-family modals, the context menu
                // and its dismissal, and the nested Settings popups (the
                // multiselect, the library routes, and the feed management).
                OverlayRequest::SavePlaylist(_)
                | OverlayRequest::ContextMenu(_)
                | OverlayRequest::DismissContextMenu
                | OverlayRequest::OpenMultiselect(_)
                | OverlayRequest::OpenLibraryRoutes
                | OverlayRequest::OpenFeedsManage
                | OverlayRequest::DismissConfirm
                | OverlayRequest::DismissDaemonLost
                | OverlayRequest::DismissSavePlaylist => log_undelivered(&request),
            }
        }
        // The Settings id's stack membership follows the App's state (design
        // D2): the push and the pop are idempotent, so a tick that changed
        // nothing re-records the same membership.
        if self.app.gui_settings_is_open() {
            self.focus
                .push_overlay(ComponentId::Overlay(OverlayId::Settings));
        } else {
            self.focus
                .dismiss(&ComponentId::Overlay(OverlayId::Settings));
        }
    }
}
