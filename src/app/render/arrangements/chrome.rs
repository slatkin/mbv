use crate::app::layout::{FocusState, FrameChromeGeometry};
use crate::app::render::components::chrome;
use crate::app::render::components::widgets::COLUMN_GAP;
use crate::app::{PanelMode, TABBAR_LEFT_RESERVE};
use ratatui::layout::Rect;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
const TAB_BAR_BOX_HEIGHT: u16 = 3;

/// Height of the player panel box below the tab bar (seekbar + title +
/// controls rows).
pub(in crate::app) const PLAYER_BOX_HEIGHT: u16 = 4;

/// Resolved app state needed to place the frame chrome for one terminal area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct ChromeGeometryInput {
    pub area: Rect,
    pub panel_mode: PanelMode,
    pub focus: FocusState,
    pub queue_column_width: u16,
    pub terminal_width: u16,
}

/// Inner tab-strip text width for a tab-bar box of `tab_bar_width` columns.
///
/// Shared by the chrome painter (`chrome_geometry` below) and the keyboard-path
/// `App::ensure_tab_visible`, so the tab-scroll math matches what is painted
/// rather than being re-derived (ADR 0022 Residual A). `PB_H` is the 2-column
/// padding inside the coloured box, applied on both sides.
pub(in crate::app) fn tab_strip_text_width(tab_bar_width: u16) -> u16 {
    const PB_H: u16 = 2;
    tab_bar_width.saturating_sub(2 * PB_H + TABBAR_LEFT_RESERVE)
}

/// Computes the root/chrome geometry for one frame without reading app state.
pub(in crate::app) fn chrome_geometry(input: ChromeGeometryInput) -> FrameChromeGeometry {
    let area = input.area;
    // Left panel (card + queue) | Right panel (library, remaining).
    let left_w = match input.panel_mode {
        PanelMode::Both => input.queue_column_width,
        PanelMode::LibraryOnly => 0,
        PanelMode::QueueOnly => area.width,
    };
    let right_w = area.width.saturating_sub(left_w);
    let queue_boundary_area = if input.panel_mode == PanelMode::Both && left_w > 0 {
        Rect {
            x: area.x + left_w - 1,
            y: area.y,
            width: 1,
            height: area.height,
        }
    } else {
        Rect::default()
    };
    let right_visible = input.panel_mode != PanelMode::QueueOnly;

    let content_h = area.height;
    let left_area = if input.panel_mode == PanelMode::LibraryOnly {
        Rect::default()
    } else {
        Rect {
            x: area.x,
            y: area.y,
            width: left_w,
            height: content_h,
        }
    };
    let panel_area = if input.terminal_width < crate::app::MINI_VIEW_THRESHOLD
        && input.panel_mode == PanelMode::LibraryOnly
    {
        area
    } else {
        left_area
    };
    let panel_content_area = chrome::left_panel_content_area(panel_area);

    // Full-column background behind the card image and queue list.
    let right_full_area = Rect {
        x: area.x + left_w + COLUMN_GAP,
        y: area.y,
        width: right_w.saturating_sub(COLUMN_GAP),
        height: area.height,
    };

    // Inner content area with padding inside the colored box (queue uses this).
    let left_content = Rect {
        x: left_area.x + 2,
        y: left_area.y + 1,
        width: left_area.width.saturating_sub(4),
        height: left_area.height.saturating_sub(2),
    };

    let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
    let right_area = Rect {
        x: area.x + left_w + COLUMN_GAP,
        y: area.y + tab_h + PLAYER_BOX_HEIGHT,
        width: right_w.saturating_sub(COLUMN_GAP),
        height: content_h
            .saturating_sub(1)
            .saturating_sub(tab_h)
            .saturating_sub(PLAYER_BOX_HEIGHT),
    };

    // Tab bar at the very top of the right column.
    let tab_bar_area = Rect {
        x: right_area.x,
        y: area.y,
        width: right_area.width,
        height: tab_h,
    };

    // Player panel below the tab bar (right column only).
    let player_area = if right_visible {
        Rect {
            x: right_area.x,
            y: area.y + tab_h,
            width: right_area.width,
            height: PLAYER_BOX_HEIGHT,
        }
    } else {
        Rect::default()
    };

    // Status bar sits at the bottom of the right panel only.
    let status_area = Rect {
        x: right_area.x,
        y: right_area.y + right_area.height,
        width: right_area.width,
        height: 1,
    };

    // Tab-bar hit targets; only published when the tab bar actually paints.
    let tabs_area = if right_visible {
        let tab_row = Rect {
            y: tab_bar_area.y + 1,
            height: 1,
            ..tab_bar_area
        };
        let tabs_x = tab_bar_area.x + 1;
        let tabs_w = tab_strip_text_width(tab_bar_area.width);
        Rect {
            x: tabs_x,
            width: tabs_w,
            ..tab_row
        }
    } else {
        Rect::default()
    };

    FrameChromeGeometry {
        panel_area,
        panel_content_area,
        left_area,
        queue_boundary_area,
        right_area,
        right_full_area,
        left_content,
        tab_bar_area,
        tabs_area,
        player_area,
        status_area,
        right_visible,
        focus: input.focus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PanelFocus;

    /// A wide frame (>= `MINI_VIEW_THRESHOLD`) with the given effective panel
    /// visibility and focus.
    fn wide(panel_mode: PanelMode, panel_focus: PanelFocus) -> FrameChromeGeometry {
        chrome_geometry(ChromeGeometryInput {
            area: Rect::new(0, 0, 160, 40),
            panel_mode,
            focus: FocusState::new(panel_focus, panel_mode),
            queue_column_width: 60,
            terminal_width: 160,
        })
    }

    #[test]
    fn right_column_is_focused_only_when_visible_and_library_holds_focus() {
        // Wide LibraryOnly: the right column is the whole frame and the
        // library panel holds focus, so the column surface follows it.
        assert!(wide(PanelMode::LibraryOnly, PanelFocus::Library)
            .focus
            .library_column_focused());

        // Wide Both with library focus: the column is the surface the focused
        // library panel sits on.
        let both_library = wide(PanelMode::Both, PanelFocus::Library);
        assert!(both_library.focus.library_column_focused());
        assert!(both_library.right_visible);
        assert!(!both_library.focus.queue_column_focused());

        // Wide Both with queue focus: the column is still visible but resting.
        let both_queue = wide(PanelMode::Both, PanelFocus::Queue);
        assert!(!both_queue.focus.library_column_focused());
        assert!(both_queue.right_visible);
        assert!(both_queue.focus.queue_column_focused());

        // QueueOnly: no right column exists, so it never reports focus even
        // though the stored focus bit is Library.
        let queue_only = wide(PanelMode::QueueOnly, PanelFocus::Library);
        assert!(!queue_only.right_visible);
        assert!(!queue_only.focus.library_column_focused());
        assert!(!queue_only.focus.queue_column_focused());
    }

    /// Narrow mini-view widths. Below `MINI_VIEW_THRESHOLD` `App` collapses
    /// `mini_view_focus` into one of two effective shapes before calling here:
    /// the library half supplies `LibraryOnly`/`Library` (the right column
    /// occupies the full frame width and is therefore visible and focused),
    /// while the queue half supplies `QueueOnly` (no right column at all).
    #[test]
    fn narrow_mini_view_reports_the_library_half_as_focused_and_the_queue_half_as_absent() {
        let narrow_width = crate::app::MINI_VIEW_THRESHOLD - 1;

        let library_half = chrome_geometry(ChromeGeometryInput {
            area: Rect::new(0, 0, narrow_width, 40),
            panel_mode: PanelMode::LibraryOnly,
            focus: FocusState::new(PanelFocus::Library, PanelMode::LibraryOnly),
            queue_column_width: 60,
            terminal_width: narrow_width,
        });
        assert!(library_half.right_visible);
        assert!(library_half.focus.library_column_focused());

        let queue_half = chrome_geometry(ChromeGeometryInput {
            area: Rect::new(0, 0, narrow_width, 40),
            panel_mode: PanelMode::QueueOnly,
            focus: FocusState::new(PanelFocus::Queue, PanelMode::QueueOnly),
            queue_column_width: 60,
            terminal_width: narrow_width,
        });
        assert!(!queue_half.right_visible);
        assert!(!queue_half.focus.library_column_focused());
    }
}
