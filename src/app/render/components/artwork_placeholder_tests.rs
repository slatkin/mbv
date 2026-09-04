use super::artwork_placeholder::render_artwork_placeholder;
use crate::app::palette;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

#[test]
fn artwork_placeholder_paints_requested_extent() {
    let mut terminal = Terminal::new(TestBackend::new(8, 5)).unwrap();
    let area = Rect::new(2, 1, 4, 3);

    terminal
        .draw(|frame| render_artwork_placeholder(frame, area))
        .unwrap();

    let buffer = terminal.backend().buffer();
    for y in 0..5 {
        for x in 0..8 {
            let expected = if area.contains((x, y).into()) {
                palette::SURFACE_ARTWORK_PLACEHOLDER
            } else {
                ratatui::style::Color::Reset
            };
            assert_eq!(buffer[(x, y)].bg, expected, "cell ({x}, {y})");
        }
    }
}
