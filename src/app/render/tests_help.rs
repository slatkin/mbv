use super::test_helpers::buffer_to_string;
use crate::app::tests::make_app_stub;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_help(width: u16, height: u16, scroll: u16) -> String {
    let mut app = make_app_stub();
    app.help_scroll = scroll;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| app.render_help_panel(f, Some(Rect::new(0, 0, width, height))))
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn help_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, scroll) in [(60, 20, 0), (60, 20, 2), (18, 8, 0), (32, 12, 1)] {
        let output = render_help(width, height, scroll);
        assert!(
            output.contains("KEYBOARD"),
            "help shell missing: {output:?}"
        );
    }
}
