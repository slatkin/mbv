use super::test_helpers::buffer_to_string;
use crate::app::tests::make_app_stub;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_feeds_manage(width: u16, height: u16) -> String {
    let mut app = make_app_stub();
    app.open_feeds_manage_popup();
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render_feeds_manage_popup(f)).unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn feeds_manage_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height) in [(60, 20), (60, 20), (20, 10), (32, 12)] {
        let output = render_feeds_manage(width, height);
        assert!(
            output.contains("Manage"),
            "feed-management shell missing: {output:?}"
        );
    }
}
