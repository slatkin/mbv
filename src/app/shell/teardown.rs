//! The shell's exit teardown (design D3): the exit kind is shell-owned, so
//! the part of the run-loop teardown that names the shell `Model` lives here.
//! The teardown every Client shares over `App` stays in `dispatch`.
//!
//! The launch snapshot save is shared with the GUI Client's pin-swap
//! handshake, so this module is its shell-side home.

use crate::app::App;
use crate::app::dispatch::run_loop::teardown::ExitKind;
use std::time::Duration;

use super::Model;

pub(in crate::app) use crate::app::dispatch::run_loop::teardown::save_launch_state;

impl Model {
    /// Finish an orderly TUI teardown after taking the selected destination's
    /// bounded launch snapshot. The App remains the persistence authority;
    /// this shell query is the only reverse read from the mounted owner.
    pub(in crate::app) fn teardown(&mut self, quit_timeout: Duration) {
        let exit_kind = if self.swapped_out_exit {
            ExitKind::SwappedOut
        } else {
            ExitKind::Quit {
                launch_state: self.launch_state_snapshot(),
            }
        };
        self.app.teardown(quit_timeout, exit_kind);
    }
}

impl App {
    /// Run an exit teardown whose kind the TUI shell decided (tray-pin-swap
    /// design D6): persist the launch snapshot when the exit is orderly, then
    /// run the shared teardown.
    pub(in crate::app) fn teardown(&mut self, quit_timeout: Duration, exit_kind: ExitKind) {
        let swapped_out = matches!(exit_kind, ExitKind::SwappedOut);
        if let ExitKind::Quit { launch_state } = exit_kind {
            save_launch_state(&launch_state);
        }
        self.teardown_inner(quit_timeout, swapped_out);
    }
}
