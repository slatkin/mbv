use crate::app::render::components::home_video::home_panel_scroll;
// Characterization coverage stays beside the moved Home component.

#[test]
fn keeps_current_offset_when_row_already_visible() {
    assert_eq!(home_panel_scroll(0, 2, 6, 20, 10), 0);
}

#[test]
fn scrolls_down_to_reveal_row_below_viewport() {
    assert_eq!(home_panel_scroll(0, 14, 20, 30, 10), 10);
}

#[test]
fn scrolls_up_to_reveal_row_above_viewport() {
    assert_eq!(home_panel_scroll(8, 2, 6, 30, 10), 2);
}

#[test]
fn never_scrolls_past_end() {
    assert_eq!(home_panel_scroll(99, 11, 15, 15, 10), 5);
}
