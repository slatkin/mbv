use super::{App, PanelFocus, PanelMode};
use std::time::Instant;

impl App {
    /// The panel mode actually in effect for rendering/input this frame.
    /// Below `MINI_VIEW_THRESHOLD` columns the Power View ignores the stored
    /// three-state `panel_mode` and derives a two-state mini view from the
    /// ephemeral `mini_view_focus`; at 80+ columns the stored mode is used
    /// unchanged.
    pub(super) fn effective_panel_mode(&self) -> PanelMode {
        if self.terminal_width < super::MINI_VIEW_THRESHOLD {
            match self.mini_view_focus {
                PanelFocus::Library => PanelMode::LibraryOnly,
                PanelFocus::Queue => PanelMode::QueueOnly,
            }
        } else {
            self.panel_mode
        }
    }

    /// The panel focus actually in effect for input routing this frame. Below
    /// `MINI_VIEW_THRESHOLD` columns this is `mini_view_focus`; at 80+ columns
    /// the stored `panel_focus` is returned unchanged.
    pub(super) fn effective_panel_focus(&self) -> PanelFocus {
        if self.terminal_width < super::MINI_VIEW_THRESHOLD {
            self.mini_view_focus
        } else {
            self.panel_focus
        }
    }

    /// Record that the terminal just regained focus, arming the
    /// refocus-click suppression window (see `handle_mouse`).
    pub(super) fn note_focus_gained(&mut self) {
        self.refocus_at = Some(Instant::now());
    }

    /// Clear any pending refocus suppression -- the window shouldn't
    /// outlive the focus session that armed it.
    pub(super) fn note_focus_lost(&mut self) {
        self.refocus_at = None;
    }

    pub(super) fn set_panel_focus(&mut self, focus: PanelFocus) {
        if self.panel_focus == focus {
            return;
        }
        if matches!(focus, PanelFocus::Queue) {
            self.focus_queue_initial_item();
        }
        self.panel_focus = focus;
        self.save_prefs();
    }
}
