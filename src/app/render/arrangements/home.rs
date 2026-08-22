use ratatui::layout::Rect;

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
