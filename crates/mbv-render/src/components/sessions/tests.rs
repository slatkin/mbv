use super::*;
use mbv_ui_model::panel_targets::SessionTransferOffers;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// The queue-transfer hints (design D6) list on the spare second footer
/// row only while the selected target offers the actions; the base hint
/// row never carries them, so `[Esc]close` cannot clip at the queue
/// column's minimum width.
#[test]
fn transfer_hints_list_on_the_second_footer_row_only_while_offered() {
    let area = Rect::new(0, 0, 40, 12);
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|f| {
            let _ = render_sessions_overlay_content(
                f,
                Some(area),
                1,
                false,
                false,
                SessionTransferOffers {
                    hand_off: true,
                    take: true,
                },
            );
        })
        .unwrap();
    let transfer_row: String = (0..40)
        .map(|x| terminal.backend().buffer()[(x, 11)].symbol().to_string())
        .collect();
    assert!(
        transfer_row.contains("[h]hand [t]take"),
        "both offered actions list on the second footer row: {transfer_row:?}"
    );
    let hint_row: String = (0..40)
        .map(|x| terminal.backend().buffer()[(x, 10)].symbol().to_string())
        .collect();
    assert!(
        !hint_row.contains("[h]hand") && !hint_row.contains("[t]take"),
        "the base hint row keeps its own hints only: {hint_row:?}"
    );

    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|f| {
            let _ = render_sessions_overlay_content(
                f,
                Some(area),
                1,
                false,
                false,
                SessionTransferOffers::default(),
            );
        })
        .unwrap();
    let transfer_row: String = (0..40)
        .map(|x| terminal.backend().buffer()[(x, 11)].symbol().to_string())
        .collect();
    assert!(
        transfer_row.trim().is_empty(),
        "no offered action leaves the second footer row blank: {transfer_row:?}"
    );
}
