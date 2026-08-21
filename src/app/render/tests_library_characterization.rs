use super::test_helpers::{buffer_to_string, make_movie_app};
use super::*;
use crate::app::layout::LayoutMain;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_library(app: &mut App, width: u16, height: u16, focused: bool) -> String {
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
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    let states = [
        (120, 40, true, 0),
        (120, 40, false, 0),
        (60, 20, true, 0),
        (60, 20, true, 1),
    ];
    for (width, height, focused, cursor) in states {
        let mut app = make_movie_app();
        app.libs[0].nav_stack[0].cursor = cursor;
        let output = render_library(&mut app, width, height, focused);
        assert!(
            output.contains("Movie"),
            "library rows missing in {width}x{height}: {output:?}"
        );
    }
}
