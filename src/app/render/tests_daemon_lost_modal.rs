use super::test_helpers::buffer_to_string;
use crate::app::tests::make_app_stub;
use crate::app::types_daemon_lost::DaemonLostModal;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_daemon_lost(width: u16, height: u16, with_error: bool) -> String {
    let mut app = make_app_stub();
    app.daemon_lost_modal = Some(DaemonLostModal {
        last_playing_title: Some("Birthday Clip".into()),
        daemon_log_path: "/tmp/mbvd.log".into(),
        restart_error: with_error.then(|| "connection refused".into()),
    });
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render_daemon_lost_modal(f)).unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn daemon_lost_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, with_error) in [
        (70, 16, false),
        (70, 16, true),
        (24, 10, true),
        (40, 12, false),
    ] {
        let output = render_daemon_lost(width, height, with_error);
        assert!(
            output.contains("Daemon"),
            "daemon-lost modal missing: {output:?}"
        );
    }
}
