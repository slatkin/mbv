//! Test support for the TUI shell and for the tests that drive it.
//!
//! Everything here names the toolkit (ratatui's `TestBackend` and `Terminal`)
//! or drives a shell surface, so it lives on the shell side of the boundary.
//! The core tests keep only toolkit-free helpers.

use super::*;
pub(crate) use ratatui::Terminal;
pub(crate) use ratatui::backend::TestBackend;

/// Record a terminal size the way the TUI shell records one: the shell keeps
/// the size (`Model::terminal_width`/`terminal_height`) and pushes the
/// single-panel fact (`App::set_single_panel`), which `is_mini_view` reads.
pub(in crate::tui) fn set_terminal_size(model: &mut Model, width: u16, height: u16) {
    model.terminal_width = width;
    model.terminal_height = height;
    let single_panel = super::single_panel_for_width(width);
    model.app.set_single_panel(single_panel);
}

/// Record only a terminal width, pushing the matching single-panel fact.
pub(in crate::tui) fn set_terminal_width(model: &mut Model, width: u16) {
    model.terminal_width = width;
    let single_panel = super::single_panel_for_width(width);
    model.app.set_single_panel(single_panel);
}
