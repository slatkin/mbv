//! Contracts of the tab bar's overflow window: what fits, where the chevrons
//! show, and that the current tab stays painted.

use super::{CHEVRON_WIDTH, TabWindow, start_showing, tab_window};

const WIDTHS: [f32; 5] = [100.0; 5];

#[test]
fn an_unmeasured_strip_paints_every_tab_and_no_chevron() {
    assert_eq!(
        tab_window(&WIDTHS, 2, 0.0),
        TabWindow {
            start: 0,
            end: 5,
            left: false,
            right: false
        }
    );
}

#[test]
fn a_strip_that_holds_every_tab_needs_no_chevron() {
    let window = tab_window(&WIDTHS, 0, 500.0);
    assert_eq!((window.end, window.left, window.right), (5, false, false));
}

#[test]
fn an_overflowing_strip_reserves_the_right_chevron() {
    // 250 holds two tabs; the right chevron then leaves room for two (224).
    let window = tab_window(&WIDTHS, 0, 250.0);
    assert_eq!((window.end, window.left, window.right), (2, false, true));
}

#[test]
fn a_scrolled_strip_reserves_the_left_chevron() {
    // The left chevron leaves 276; the last tabs fit with no right chevron.
    let window = tab_window(&WIDTHS, 3, 100.0 + CHEVRON_WIDTH);
    assert_eq!(
        (window.start, window.end, window.left, window.right),
        (3, 4, true, true)
    );
}

#[test]
fn a_strip_narrower_than_one_tab_still_paints_one() {
    let window = tab_window(&WIDTHS, 1, 10.0);
    assert_eq!((window.start, window.end), (1, 2));
}

#[test]
fn the_current_tab_scrolls_into_view_from_either_side() {
    assert_eq!(start_showing(&WIDTHS, 4, 0, 250.0), 3);
    assert_eq!(start_showing(&WIDTHS, 1, 3, 250.0), 1);
    assert_eq!(start_showing(&WIDTHS, 1, 0, 250.0), 0);
}
