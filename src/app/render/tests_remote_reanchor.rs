use super::test_helpers::buffer_to_string;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_reanchor(width: u16, height: u16, cursor: usize) -> String {
    let targets = vec![
        (0usize, "device-a".to_string()),
        (1, "device-b".to_string()),
    ];
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_flag = false;
    terminal
        .draw(|f| {
            crate::app::render::render_remote_reanchor_popup_content(
                f,
                &mut dim_flag,
                &targets,
                cursor,
            )
        })
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
