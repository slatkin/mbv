//! The Status bar's shared paint facts (D6 of `replace-tauri-with-blitz`):
//! the port of `lib/bridge/status.ts`. The frame's `GuiStatusBar` is carried
//! to both surfaces that paint it — the Queue footer's line and the Library
//! Status bar — so its toast takes one severity colour from here rather
//! than one per painter.

use crate::bridge::frame::chrome::GuiToastSeverity;

/// The toast text's severity foreground (`toast_spans` in the TUI's status
/// bar panel).
#[must_use]
pub fn toast_color(severity: GuiToastSeverity) -> &'static str {
    match severity {
        GuiToastSeverity::Neutral => "var(--mbv-textPrimary)",
        GuiToastSeverity::Success => "var(--mbv-statusAvailable)",
        GuiToastSeverity::Warning => "var(--mbv-textFocusAccent)",
        GuiToastSeverity::Error => "var(--mbv-statusError)",
    }
}
