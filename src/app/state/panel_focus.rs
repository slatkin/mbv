use crate::app::{App, PanelFocus, PanelMode};
use std::time::Instant;

impl App {
    /// The panel mode actually in effect for rendering/input this frame.
    /// Below `MINI_VIEW_THRESHOLD` columns the Power View ignores the stored
    /// three-state `panel_mode` and derives a two-state mini view from the
    /// ephemeral `mini_view_focus`; at 80+ columns the stored mode is used
    /// unchanged.
    pub(in crate::app) fn effective_panel_mode(&self) -> PanelMode {
        if self.terminal_width < mbv_render::layout::MINI_VIEW_THRESHOLD {
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
    pub(in crate::app) fn effective_panel_focus(&self) -> PanelFocus {
        if self.terminal_width < mbv_render::layout::MINI_VIEW_THRESHOLD {
            self.mini_view_focus
        } else {
            self.panel_focus
        }
    }

    /// Whether a panel's surfaces paint their focused palette this frame.
    /// Wide view contrasts the focused panel against its resting sibling, so
    /// the focus bit selects the palette. Mini view displays a single panel --
    /// nothing to contrast against -- so below `MINI_VIEW_THRESHOLD` every
    /// panel paints the default (unfocused) palette regardless of which panel
    /// holds focus. Input routing keeps `effective_panel_focus`; this bit is
    /// for appearance (surface palette selection) only.
    pub(in crate::app) fn panel_appearance_focus(&self, panel: PanelFocus) -> bool {
        self.terminal_width >= mbv_render::layout::MINI_VIEW_THRESHOLD
            && self.effective_panel_focus() == panel
    }

    /// Record that the terminal just regained focus, arming the
    /// refocus-click suppression window (see `handle_mouse`).
    pub(in crate::app) fn note_focus_gained(&mut self) {
        self.refocus_at = Some(Instant::now());
    }

    /// Clear any pending refocus suppression -- the window shouldn't
    /// outlive the focus session that armed it.
    pub(in crate::app) fn note_focus_lost(&mut self) {
        self.refocus_at = None;
    }

    pub(in crate::app) fn set_panel_focus(&mut self, focus: PanelFocus) {
        // Below MINI_VIEW_THRESHOLD, focus-follow call sites (mouse clicks,
        // Alt+Left/Right, context actions) must move the ephemeral mini-view
        // focus, never the real persisted panel_focus -- that state has to
        // survive narrowing/widening untouched.
        if self.terminal_width < mbv_render::layout::MINI_VIEW_THRESHOLD {
            if self.mini_view_focus == focus {
                return;
            }
            if matches!(focus, PanelFocus::Queue) {
                self.focus_queue_initial_item();
            }
            self.mini_view_focus = focus;
            return;
        }
        if self.panel_focus == focus {
            return;
        }
        if matches!(focus, PanelFocus::Queue) {
            self.focus_queue_initial_item();
        }
        self.panel_focus = focus;
    }
}
