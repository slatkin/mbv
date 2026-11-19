//! The Status bar's shared paint facts (D6 of `replace-tauri-with-blitz`):
//! the port of `lib/bridge/status.ts`. Every surface that paints a toast
//! takes one severity colour from here rather than one per painter: the
//! `toasts` section's cards (the floating stack, and the status lines that
//! still paint a toast inline until task row 2.4 removes that paint).

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
