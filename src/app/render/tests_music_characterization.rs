use super::test_helpers::{buffer_to_string, make_music_group_app};
use super::*;
use crate::app::layout::LayoutMain;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_music(app: &mut App, width: u16, height: u16, focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|f| {
            app.render_library(f, Rect::new(0, 0, width, height), focused, &mut layout);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    let states = [(120, 30, true, 0), (120, 30, false, 0), (60, 30, true, 0)];
    for (width, height, focused, cursor) in states {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].cursor = cursor;
        let output = render_music(&mut app, width, height, focused);
        assert!(
            output.contains("First Album"),
            "music rows missing in {width}x{height}: {output:?}"
        );
    }

    let mut selected = make_music_group_app();
    selected.libs[0].nav_stack[1].cursor = 0;
    let output = render_music(&mut selected, 60, 8, true);
    assert!(
        output.contains("First Album"),
        "selected music row missing: {output:?}"
    );
}
