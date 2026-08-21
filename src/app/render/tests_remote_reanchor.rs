use super::test_helpers::buffer_to_string;
use crate::app::tests::make_app_stub;
use crate::app::types_playback::RemoteReanchorPopup;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_reanchor(width: u16, height: u16, cursor: usize) -> String {
    let mut app = make_app_stub();
    app.remote_reanchor_popup = Some(RemoteReanchorPopup {
        targets: vec![(0, "device-a".into()), (1, "device-b".into())],
        cursor,
    });
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| app.render_remote_reanchor_popup(f))
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn remote_reanchor_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor) in [(60, 20, 0), (60, 20, 1), (20, 10, 1), (32, 12, 0)] {
        let output = render_reanchor(width, height, cursor);
        assert!(
            output.contains("Occurrence"),
            "re-anchor modal missing: {output:?}"
        );
    }
}
