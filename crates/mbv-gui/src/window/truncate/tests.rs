//! Headless tests of the ellipsis cut (D16): one per contract of task 2.4 —
//! a text that fits is unchanged, and a text that does not fit ends in `…`
//! and its measured width is at most the box width. The measure runs over
//! the bundled faces, so no test opens a window or sleeps.

use rstest::rstest;

use super::{TextStyle, first_measure_needs_remeasure, fit_text};

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

/// Contract 4 (D12): only a box's first measure after mount, and only when it
/// read an unlaid (zero-wide) box, asks for one post-layout re-measure. Every
/// later measure settles, so a collapsed box cannot ask again and the bump its
/// request drives cannot cycle.
#[rstest]
#[case::first_measure_of_an_unlaid_box(true, 0.0, true)]
#[case::first_measure_of_a_laid_out_box(true, 120.0, false)]
#[case::remeasure_of_an_unlaid_box(false, 0.0, false)]
fn only_a_first_measure_of_an_unlaid_box_requests_a_remeasure(
    #[case] is_first_measure: bool,
    #[case] client_width: f64,
    #[case] expected: bool,
) {
    assert_eq!(
        first_measure_needs_remeasure(is_first_measure, client_width),
        expected
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
