use super::test_helpers::buffer_to_string;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_daemon_lost(width: u16, height: u16, with_error: bool) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_flag = false;
    terminal
        .draw(|f| {
            crate::app::render::render_daemon_lost_modal_content(
                f,
                &mut dim_flag,
                Some("Birthday Clip"),
                "/tmp/mbvd.log",
                with_error.then_some("connection refused"),
            )
        })
        .unwrap();
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
