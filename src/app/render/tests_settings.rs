use super::test_helpers::buffer_to_string;
use crate::app::layout::AppLayout;
use crate::app::tests::make_app_stub;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_settings(width: u16, height: u16, cursor: usize) -> String {
    let mut app = make_app_stub();
    app.settings_cursor = cursor;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    terminal
        .draw(|f| {
            app.render_settings_panel(f, &mut layout, Some(Rect::new(0, 0, width, height)));
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn settings_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor) in [(50, 20, 0), (50, 20, 3), (18, 10, 3), (30, 12, 1)] {
        let output = render_settings(width, height, cursor);
        assert!(
            output.contains("SETTINGS"),
            "settings shell missing: {output:?}"
        );
    }
}
