use super::hero_left;
use ratatui::layout::Rect;

/// The shared padded panes used by wide library presentations.
pub(in crate::app::render) struct WideLibraryPanes {
    pub left_panel: Rect,
    pub right_panel: Rect,
    pub left_area: Rect,
    pub right_area: Rect,
}

pub(in crate::app::render) fn wide_library_panes(
    area: Rect,
    pad_x: u16,
    pad_y: u16,
) -> Option<WideLibraryPanes> {
    let (mut left_panel, right_panel) = hero_left::shared_hero_presentation(area)?;
    left_panel.height = area.height.saturating_sub(1);
    let left_area = Rect {
        x: left_panel.x.saturating_add(pad_x),
        y: left_panel.y.saturating_add(pad_y),
        width: left_panel.width.saturating_sub(pad_x * 2),
        height: left_panel.height.saturating_sub(pad_y * 2),
    };
    let right_area = Rect {
        x: right_panel.x,
        y: right_panel.y.saturating_add(pad_y),
        width: right_panel.width,
        height: right_panel.height.saturating_sub(pad_y * 2),
    };
    Some(WideLibraryPanes {
        left_panel,
        right_panel,
        left_area,
        right_area,
    })
}

pub(in crate::app::render) fn wide_list_area(panel: Rect, pad_x: u16, pad_y: u16) -> Rect {
    Rect {
        x: panel.x.saturating_add(pad_x),
        y: panel.y.saturating_add(pad_y),
        width: panel.width.saturating_sub(pad_x * 2),
        height: panel.height.saturating_sub(pad_y * 2),
    }
}

pub(in crate::app::render) fn inline_library_areas(area: Rect, controls_rows: u16) -> (Rect, Rect) {
    (
        Rect {
            height: controls_rows.min(area.height),
            ..area
        },
        Rect {
            y: area.y.saturating_add(controls_rows),
            height: area.height.saturating_sub(controls_rows),
            ..area
        },
    )
}

pub(in crate::app::render) fn selected_detail_content_area(
    hero_area: Rect,
    side_padding: u16,
    extra_rows: u16,
) -> Rect {
    Rect {
        x: hero_area.x.saturating_add(side_padding),
        y: hero_area.y.saturating_add(2),
        width: hero_area.width.saturating_sub(side_padding * 2),
        height: hero_area.height.saturating_sub(extra_rows),
    }
}
