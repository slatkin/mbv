//! Headless tests of the ellipsis cut (D16): one per contract of task 2.4 —
//! a text that fits is unchanged, and a text that does not fit ends in `…`
//! and its measured width is at most the box width. The measure runs over
//! the bundled faces, so no test opens a window or sleeps.

use super::{TextStyle, fit_text};

/// A wide box at the body size the GUI text uses (D12's example).
fn body_style() -> TextStyle {
    TextStyle::sans(13.0, 500.0)
}

/// Contract 1 (task 2.4): a text that fits is unchanged — no cut, no
/// ellipsis.
#[test]
fn text_that_fits_is_unchanged() {
    let text = "Episode one";
    assert_eq!(fit_text(text, &body_style(), 400.0), text);
}

/// Contract 2 (task 2.4): a text that does not fit ends in `…` and its
/// measured width is at most the box width.
#[test]
fn text_that_does_not_fit_is_cut_to_the_box_width() {
    let style = body_style();
    let text = "The Wheel of Time, Book One: The Eye of the World";
    let cut = fit_text(text, &style, 120.0);
    assert!(cut.ends_with('\u{2026}'), "{cut:?}");
    assert!(cut.len() < text.len(), "{cut:?}");
    let measured = super::MEASURE
        .lock()
        .expect("the measure is not poisoned")
        .width(&cut, &style);
    assert!(
        f64::from(measured) <= 120.0,
        "the cut text measures {measured} CSS px"
    );
}

/// Contract 3 (D12): a box the layout has not measured — a zero-width client
/// rect — keeps its whole text and lets the `truncate` class clip it, rather
/// than painting the ellipsis alone.
#[test]
fn text_in_a_box_with_no_width_is_unchanged() {
    let text = "Episode one";
    assert_eq!(fit_text(text, &body_style(), 0.0), text);
}
