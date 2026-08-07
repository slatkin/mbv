use super::{App, PanelFocus};
use std::time::Instant;

impl App {
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
