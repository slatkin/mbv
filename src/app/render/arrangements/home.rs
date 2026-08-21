use ratatui::layout::Rect;

pub(in crate::app::render) fn content_area(area: Rect, offset: u16) -> Rect {
    Rect {
        y: area.y.saturating_add(offset),
        height: area.height.saturating_sub(offset),
        ..area
    }
}

pub(in crate::app::render) fn pills_area(area: Rect) -> Rect {
    Rect {
        y: area.y,
        height: 1.min(area.height),
        ..area
    }
}

pub(in crate::app::render) fn inline_hero_area(
    content_area: Rect,
    detail_screen_row: usize,
    hero_rows: u16,
) -> Rect {
    Rect {
        x: content_area.x,
        y: content_area.y + detail_screen_row as u16,
        width: content_area.width,
        height: hero_rows,
    }
}

pub(in crate::app::render) fn pill_gap(area: Rect) -> Rect {
    Rect {
        y: area.y.saturating_add(1),
        height: 1,
        ..area
    }
}
