use super::*;

#[test]
fn collapsed_power_right_panel_keeps_one_column_after_scrollbar() {
    let right_panel = Rect::new(0, 0, 80, 24);
    let content = power_right_panel_content_area(right_panel, true);
    assert_eq!(content.x + content.width, right_panel.right() - 1);
}

#[test]
fn one_column_power_right_panel_uses_full_left_pad() {
    // A right panel narrower than the two-column library threshold keeps
    // the full `POWER_TAB_LEFT_PAD` (2) for visual breathing room around
    // a single-column list.
    let right_panel = Rect::new(0, 0, 60, 24);
    let content = power_right_panel_content_area(right_panel, false);
    assert_eq!(content.x, right_panel.x + 2);
    assert_eq!(content.width, right_panel.width - 4);
}

#[test]
fn two_column_power_right_panel_uses_single_left_pad() {
    // A right panel at or above the two-column library threshold drops
    // the left pad to 1 so the left cell sits one column in instead of
    // two -- the 2-col mode otherwise reads as pushed in from the panel
    // edge.
    let right_panel = Rect::new(0, 0, 82, 24);
    let content = power_right_panel_content_area(right_panel, false);
    assert_eq!(content.x, right_panel.x + 1);
    assert_eq!(content.width, right_panel.width - 2);
}
