use super::test_helpers::buffer_to_string;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_confirm(width: u16, height: u16, title: &str, message: &str, hint: &str) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_flag = false;
    terminal
        .draw(|f| {
            crate::app::render::render_confirm_modal_content(f, &mut dim_flag, title, message, hint)
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn confirm_modal_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, title, message, hint) in [
        (
            70,
            16,
            " Clear Queue ",
            "Clear the queue?",
            "[y] Confirm    [Esc] Cancel",
        ),
        (
            70,
            16,
            " Remove Item ",
            "Remove now-playing item?",
            "[y] Confirm    [Esc] Cancel",
        ),
        (24, 10, " Rescan ", "Rescan?", "[y] Confirm    [Esc] Cancel"),
        (
            40,
            12,
            " Overwrite ",
            "Overwrite playlist?",
            "[Enter] Confirm    [Esc] Cancel",
        ),
    ] {
        let output = render_confirm(width, height, title, message, hint);
        assert!(
            output.contains(message),
            "confirm message missing: {output:?}"
        );
        assert!(
            output.contains("Confirm"),
            "confirm hint missing: {output:?}"
        );
    }
}

/// `unify-surface-colour` 4.2: the modal frame names `Surface::PopupFrame`
/// itself, so every caller paints the same popup fill without passing a
/// colour. Pin the painted body cell through the production resolver.
#[test]
fn confirm_modal_frame_paints_the_popup_frame_surface() {
    use crate::app::palette;

    let width = 70;
    let height = 16;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut dim_flag = false;
    terminal
        .draw(|f| {
            crate::app::render::render_confirm_modal_content(
                f,
                &mut dim_flag,
                " Clear Queue ",
                "Clear the queue?",
                "[y] Confirm    [Esc] Cancel",
            )
        })
        .unwrap();
    let frame = palette::surface_colors_for_column_focus(palette::Surface::PopupFrame, false).fill;
    let buffer = terminal.backend().buffer();
    // The 60-wide, 7-high frame is centred: x = (70 - 60) / 2 = 5,
    // y = (16 - 7) / 2 = 4.
    assert_eq!(
        buffer[(5, 4)].style().bg,
        Some(frame),
        "the frame body paints the PopupFrame surface"
    );
}
