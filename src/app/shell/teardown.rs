//! The shell's exit teardown decision (design D3 of `split-tui-gui-clients`):
//! the exit kind is shell-owned, so the part of the run-loop teardown that
//! names the shell `Model` lives here. The teardown every Client shares over
//! `App` — and the terminal-restoring `App` half — stays in `dispatch`.
//!
//! The launch snapshot save is shared with the GUI Client's pin-swap
//! handshake, so this module is its shell-side home.

use crate::app::dispatch::run_loop::teardown::save_launch_state;
use std::time::Duration;

use super::Model;

impl Model {
    /// Finish an orderly TUI teardown after taking the selected destination's
    /// bounded launch snapshot. The App remains the persistence authority;
    /// this shell query is the only reverse read from the mounted owner. A Pin
    /// swap replaced this TUI at its own exit boundary, so that exit saves no
    /// launch snapshot and requests no coordinated shutdown (spec
    /// daemon-lifecycle "Swapped-out TUI with Stay Alive off").
    pub(in crate::app) fn teardown(&mut self, quit_timeout: Duration) {
        let swapped_out = self.swapped_out_exit;
        if !swapped_out {
            save_launch_state(&self.launch_state_snapshot());
        }
        self.app.teardown(quit_timeout, swapped_out);
    }
}
