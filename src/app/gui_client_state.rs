//! The GUI Client's Client-core presentation state (add-tauri-gui-client row
//! 6.2): the canvas panel's collapse and queue-fold toggles, the queue filter,
//! and the open menu or popover (design D4a's "new core state" rows).
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
mod daemon_lost;
mod overlay;
mod player_menu;
mod popover;
mod projection;
mod queue_chooser;
mod queue_controls;
mod row_menu;
mod state;
mod target;

pub(in crate::app) use self::overlay::GuiOverlay;
#[cfg(test)]
pub(in crate::app) use self::overlay::GuiRowMenu;
pub use self::projection::{
    GuiConfirmButtonProjection, GuiConfirmButtonTone, GuiOverlayProjection, PanelTogglesProjection,
    QueueChooserEntryProjection, QueueChooserRenameProjection, QueueHeaderProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};
pub use self::state::GuiClientState;

use crate::gui::GuiCore;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

impl GuiCore {
    /// Whether the player panel hides its transport controls until the
    /// pointer enters the stage (gui-art-player-overlay design D5): the
    /// preference the GUI Client's panel frame carries. The core owns it, so
    /// the section build reads it here rather than from the window.
    #[must_use]
    pub fn gui_player_auto_hide(&self) -> bool {
        self.gui.player_auto_hide
    }

    /// The canvas panel's collapse and queue-fold facts (design D10).
    #[must_use]
    pub fn panel_toggles_projection(&self) -> PanelTogglesProjection {
        PanelTogglesProjection {
            collapsed: self.gui.panel_toggles.collapsed,
            queue_folded: self.gui.panel_toggles.queue_folded,
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
        if !self.app.queue_filter_active() {
            return false;
        }
        match key.code {
            Key::Esc if key.modifiers.is_empty() => {
                self.app.close_queue_filter();
                true
            }
            Key::Enter if key.modifiers.is_empty() => {
                self.app.finish_queue_filter_edit();
                true
            }
            Key::Backspace if key.modifiers.is_empty() => {
                self.app.pop_queue_filter_char();
                true
            }
            Key::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.app.push_queue_filter_char(c);
                true
            }
            _ => false,
        }
    }

    /// The `z` toggle (design D10): collapse the panel to the rail.
    pub(in crate::app) fn toggle_panel_collapse(&mut self) {
        self.gui.panel_toggles.collapsed = !self.gui.panel_toggles.collapsed;
    }

    /// The `q` toggle (design D10): fold the expanded panel's queue.
    pub(in crate::app) fn toggle_queue_fold(&mut self) {
        self.gui.panel_toggles.queue_folded = !self.gui.panel_toggles.queue_folded;
    }
}
