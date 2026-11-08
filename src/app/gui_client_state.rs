//! The GUI Client's Client-core presentation state (add-tauri-gui-client row
//! 6.2): the canvas panel's collapse and queue-fold toggles, the queue rows'
//! right column, the queue filter, and the open menu or popover (design
//! D4a's "new core state" rows).
//!
//! The Client core holds the interaction state (the `gui-client` spec), so
//! every value here lives on the `App` — not in the window content — and a
//! window-content reload keeps it. The TUI Client never reads any of it. The
//! GUI-only chords route through the one keyboard policy (`key_policy.rs`'s
//! GUI-only gate); the open overlay's key rules are the GUI's leaf
//! equivalent for the TUI's mounted overlays: while the overlay is open the
//! router resolves the chord first (the overlay's focus fact closes the
//! global bindings), and these rules — not a surface's — consume or drop it.

mod confirm;
mod overlay;
mod popover;
mod projection;
mod row_menu;
mod target;

pub(in crate::app) use self::overlay::GuiOverlay;
pub use self::projection::{
    GuiConfirmButtonProjection, GuiConfirmButtonTone, GuiOverlayProjection, PanelTogglesProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};

use crate::app::state::app_struct::LaunchRestore;
use crate::app::{App, queue_summary_projection::QueueRightColumn};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

impl App {
    /// Adopt the GUI Client's `App` rules (add-gui-library-panel design D4,
    /// D9): the GUI arranges its own panels from its measured size and always
    /// paints the Queue panel, so the TUI's terminal-width Mini check and its
    /// hidden empty queue column do not apply, and the GUI's own launch
    /// location replaces the TUI's, so this `App` starts with
    /// `LaunchRestore::Done` and no TUI launch state moves its tab or Panel
    /// focus. The GUI Client calls it before its first step; the TUI Client
    /// never does, so the TUI's values do not change.
    pub fn enable_gui_client(&mut self) {
        self.gui_client = true;
        self.launch_restore = LaunchRestore::Done;
    }

    /// The canvas panel's collapse and queue-fold facts (design D10).
    #[must_use]
    pub fn panel_toggles_projection(&self) -> PanelTogglesProjection {
        PanelTogglesProjection {
            collapsed: self.panel_collapsed,
            queue_folded: self.queue_folded,
        }
    }

    /// The queue filter's key rules while its text entry holds the keyboard:
    /// unmodified and Shift-folded characters append (the DOM's real modifier
    /// keys never type through Ctrl or Alt), Backspace pops — an empty-text
    /// Backspace closes — Esc closes and clears, and Enter closes keeping
    /// the text. Every other chord falls through to the queue surface, so
    /// navigation keeps working while the filter is open. Returns whether
    /// the key was consumed.
    pub(in crate::app) fn apply_gui_filter_key(&mut self, key: KeyEvent) -> bool {
        if !self.queue_filter_active {
            return false;
        }
        match key.code {
            Key::Esc if key.modifiers.is_empty() => {
                self.queue_filter_active = false;
                self.queue_filter.clear();
                true
            }
            Key::Enter if key.modifiers.is_empty() => {
                self.queue_filter_active = false;
                true
            }
            Key::Backspace if key.modifiers.is_empty() => {
                if self.queue_filter.pop().is_none() {
                    self.queue_filter_active = false;
                    self.queue_filter.clear();
                }
                true
            }
            Key::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.queue_filter.push(c);
                true
            }
            _ => false,
        }
    }

    /// The `z` toggle (design D10): collapse the panel to the rail.
    pub(in crate::app) fn toggle_panel_collapse(&mut self) {
        self.panel_collapsed = !self.panel_collapsed;
    }

    /// The `q` toggle (design D10): fold the expanded panel's queue.
    pub(in crate::app) fn toggle_queue_fold(&mut self) {
        self.queue_folded = !self.queue_folded;
    }

    /// The queue header's Length / Ends-at switch.
    pub(in crate::app) fn toggle_queue_right_column(&mut self) {
        self.queue_right_column = match self.queue_right_column {
            QueueRightColumn::Length => QueueRightColumn::EndsAt,
            QueueRightColumn::EndsAt => QueueRightColumn::Length,
        };
    }

    /// Focus the queue header's filter text entry (the GUI-only `/`).
    pub(in crate::app) fn activate_queue_filter(&mut self) {
        self.queue_filter_active = true;
    }
}
