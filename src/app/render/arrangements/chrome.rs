use crate::app::layout::FrameChromeGeometry;
use crate::app::render::components::chrome;
use crate::app::render::components::widgets::COLUMN_GAP;
use crate::app::{PanelFocus, PanelMode, TABBAR_LEFT_RESERVE};
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
    pub panel_focus: PanelFocus,
    pub queue_column_width: u16,
    pub terminal_width: u16,
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
    let queue_focused = matches!(input.panel_focus, PanelFocus::Queue);

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
        let pb_h: u16 = 2; // 2-col padding inside the coloured box
        let tabs_x = tab_bar_area.x + 1;
        let tabs_w = tab_bar_area
            .width
            .saturating_sub(2 * pb_h + TABBAR_LEFT_RESERVE);
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
        right_area,
        right_full_area,
        left_content,
        tab_bar_area,
        tabs_area,
        player_area,
        status_area,
        right_visible,
        queue_focused,
        left_w,
        right_w,
    }
}
