use super::test_helpers::buffer_to_string;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_menu(width: u16, height: u16, cursor: usize) -> String {
    let entries: &[(&'static str, bool)] = &[("Play", true), ("Queue", true)];
    let rect = Rect::new(5, 5, 10, 4);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| crate::app::render::render_context_menu_content(f, rect, entries, cursor))
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn context_menu_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor) in [(60, 20, 0), (60, 20, 1), (20, 10, 1), (32, 12, 0)] {
        let output = render_menu(width, height, cursor);
        assert!(output.contains("Play"), "context menu missing: {output:?}");
    }
}
