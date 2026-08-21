use super::test_helpers::buffer_to_string;
use crate::app::tests::make_app_stub;
use crate::app::types_confirm::{ConfirmAction, ConfirmModal};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_confirm(width: u16, height: u16, title: &str, message: &str, hint: &str) -> String {
    let mut app = make_app_stub();
    app.confirm_modal = Some(ConfirmModal {
        title: title.into(),
        message: message.into(),
        hint: hint.into(),
        on_confirm: ConfirmAction::ClearQueue,
    });
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render_confirm_modal(f)).unwrap();
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
