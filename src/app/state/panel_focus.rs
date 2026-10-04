use crate::app::{App, PanelFocus, PanelMode};
use std::time::Instant;

/// How the displayed panels are presented, for the appearance-focus decision.
/// Wide view contrasts the focused panel against its sibling; mini view shows
/// one panel, which rests unless pinned and the window holds focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PanelPresentation {
    Wide,
    MiniUnpinned,
    MiniPinned { window_focused: bool },
}

/// The appearance-focus decision, a pure function so tests can exercise the
/// pinned-mini branch without a constructible `pinwin::Panel`.
pub(in crate::app) fn panel_appearance_focus_decision(
    presentation: PanelPresentation,
    focus: PanelFocus,
    panel: PanelFocus,
) -> bool {
    let panel_holds_focus = focus == panel;
    match presentation {
        PanelPresentation::Wide => panel_holds_focus,
        PanelPresentation::MiniUnpinned => false,
        PanelPresentation::MiniPinned { window_focused } => window_focused && panel_holds_focus,
    }
}

impl App {
    /// Whether the terminal is below `MINI_VIEW_THRESHOLD` columns, i.e. the
    /// Power View is in mini view: a single displayed panel, no sibling to
    /// contrast a focused palette against.
    pub(in crate::app) fn is_mini_view(&self) -> bool {
        self.terminal_width < mbv_render::layout::MINI_VIEW_THRESHOLD
    }

    /// The panel mode actually in effect for rendering/input this frame.
    /// Below `MINI_VIEW_THRESHOLD` columns the Power View ignores the stored
    /// three-state `panel_mode` and derives a two-state mini view from the
    /// ephemeral `mini_view_focus`; at 80+ columns the stored mode is used
    /// unchanged.
    pub(in crate::app) fn effective_panel_mode(&self) -> PanelMode {
        if self.is_mini_view() {
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
        if self.is_mini_view() {
            self.mini_view_focus
        } else {
            self.panel_focus
        }
    }

    /// Whether a panel's surfaces paint their focused palette this frame.
    /// Wide view contrasts the focused panel against its resting sibling, so
    /// the focus bit selects the palette. Mini view displays a single panel:
    /// outside the pinned panel it always rests, but a pinned mini view
    /// follows the window focus, painting the focused palette only while the
    /// window holds focus and the panel holds effective focus (see
    /// [`panel_appearance_focus_decision`]). Input routing keeps
    /// `effective_panel_focus`; this bit is for appearance (surface palette
    /// selection) only.
    pub(in crate::app) fn panel_appearance_focus(&self, panel: PanelFocus) -> bool {
        let presentation = match (self.is_mini_view(), self.pinned_panel.is_some()) {
            (false, _) => PanelPresentation::Wide,
            (true, false) => PanelPresentation::MiniUnpinned,
            (true, true) => PanelPresentation::MiniPinned {
                window_focused: self.window_focused,
            },
        };
        panel_appearance_focus_decision(presentation, self.effective_panel_focus(), panel)
    }

    /// Record that the terminal just regained focus, arming the
    /// refocus-click suppression window (see `handle_mouse`) and marking
    /// the window focused.
    pub(in crate::app) fn note_focus_gained(&mut self) {
        self.refocus_at = Some(Instant::now());
        self.window_focused = true;
    }

    /// Clear any pending refocus suppression -- the window shouldn't
    /// outlive the focus session that armed it -- and mark the window
    /// unfocused.
    pub(in crate::app) fn note_focus_lost(&mut self) {
        self.refocus_at = None;
        self.window_focused = false;
    }

    pub(in crate::app) fn set_panel_focus(&mut self, focus: PanelFocus) {
        // Below MINI_VIEW_THRESHOLD, focus-follow call sites (mouse clicks,
        // Alt+Left/Right, context actions) must move the ephemeral mini-view
        // focus, never the real persisted panel_focus -- that state has to
        // survive narrowing/widening untouched.
        if self.is_mini_view() {
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
