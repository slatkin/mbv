use super::test_helpers::*;
use super::*;
use crate::app::layout::LayoutPlayback;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn music_group_pills_scroll_within_reserved_space_when_overflowing() {
    let mut app = make_power_music_group_app();
    app.queue_column_width = 20;
    let width = 40u16;
    let height = 20u16;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);

    let row3 = out.lines().nth(3).unwrap();

    let right_col_x = (app.queue_column_width + COLUMN_GAP) as usize;
    assert!(
        row3.chars().take(right_col_x).all(|c| c == ' '),
        "expected the pill row to be confined to the right library column:\n{out}"
    );

    assert!(!layout.selector_tabs.is_empty());
    for (rect, _) in &layout.selector_tabs {
        assert_eq!(rect.y, 3, "expected pill hitboxes on the pills row");
        assert!(
            rect.x as usize >= right_col_x,
            "expected pill hitboxes confined to the right column"
        );
        assert!(
            rect.x + rect.width <= width,
            "expected pill hitboxes confined to the visible pill row"
        );
    }
}
