//! Test support for the TUI shell and for the tests that drive it.
//!
//! Everything here names the toolkit (ratatui's `TestBackend` and `Terminal`)
//! or drives a shell surface, so it lives on the shell side of the boundary.
//! The core tests keep only toolkit-free helpers.

use super::*;
pub(crate) use ratatui::Terminal;
pub(crate) use ratatui::backend::TestBackend;

/// Confirm the populated-queue replacement gate the way the shell's confirm
/// modal does, so a gated replacement executes.
pub(crate) fn confirm_replace_queue(app: &mut App) {
    app.apply_confirm_action(
        crate::app::ConfirmAction::ReplacePopulatedQueue,
        mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Enter, mbv_ui_msg::KeyModifiers::NONE),
    );
}
