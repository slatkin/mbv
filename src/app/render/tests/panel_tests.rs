use super::*;

#[test]
fn collapsed_right_panel_keeps_one_column_after_scrollbar() {
    let right_panel = Rect::new(0, 0, 80, 24);
    let content = right_panel_content_area(right_panel, true);
    assert_eq!(content.x + content.width, right_panel.right() - 1);
}
