use super::home_video::render_home_video_item;
use crate::app::render::buffer_to_string;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_item(width: u16, height: u16, item_h: u16, selected: bool, focused: bool) -> String {
    let item = make_item("Birthday Clip", "Video");
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_home_video_item(
            f,
            &item,
            0,
            item_h,
            Rect::new(0, 0, width, height),
            width as usize,
            selected,
            focused,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

#[test]
fn home_video_item_characterization_covers_default_focused_narrow_and_selected_states() {
    assert_eq!(
        render_item(24, 1, 1, false, false),
        "Birthday Clip           \n"
    );
    assert_eq!(
        render_item(24, 1, 1, true, true),
        "  Birthday Clip         \n"
    );
    assert_eq!(render_item(8, 1, 1, true, true), "  Bir…  \n");
    // The expanded cell paints its title on the first content row below its
    // top spacer, with no frame glyphs around it.
    let expanded = render_item(24, 6, 6, true, true);
    let rows: Vec<&str> = expanded.lines().collect();
    assert_eq!(
        rows[1].trim_end(),
        "  Birthday Clip",
        "the expanded cell titles its first content row: {expanded:?}"
    );
    assert!(
        !expanded.contains('▁') && !expanded.contains('▔'),
        "the expanded cell paints no frame glyphs: {expanded:?}"
    );
}
