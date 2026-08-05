use super::test_helpers::*;
use super::*;
use crate::app::tests::make_app_stub;

#[test]
fn expanded_tab_panel_has_two_column_side_gutters() {
    let mut app = make_app_stub();
    app.queue_column_width = 40;

    let layout = render_view(&mut app, 80, 24);

    assert_eq!(layout.left_area.x, 40 + POWER_TAB_LEFT_PAD);
    assert_eq!(layout.left_area.width, 40 - 2 * POWER_TAB_LEFT_PAD);
}

#[test]
fn collapsed_power_right_panel_keeps_one_column_after_scrollbar() {
    let right_panel = Rect::new(0, 0, 80, 24);
    let content = power_right_panel_content_area(right_panel, true);
    assert_eq!(content.x + content.width, right_panel.right() - 1);
}
