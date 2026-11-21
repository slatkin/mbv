//! The Audiobooks catalog's own contracts (`gui-tech-debt-simplification`
//! design D10): the pointer markers its scroller and rows carry, and the rail
//! letter no book carries.

use super::*;

/// Contract (`LibraryAudiobooksList.test.ts`, `gui-pointer-target-interaction`
/// 3.3): the scroller names the Audiobooks list — not the `abs` hook prefix
/// its library attributes carry — and a row names its own id.
#[test]
fn the_scroller_and_rows_paint_their_pointer_markers() {
    let mut fixture = built::<AudiobooksList>(
        AudiobooksList::frame(AudiobooksList::rows(), false, false),
        AudiobooksList::detail_of(Some("b1")),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-pointer-list]", "data-pointer-list"),
        Some("audiobooks".to_owned())
    );
    assert_eq!(
        fixture.attr_of("[data-pointer-row]", "data-pointer-row"),
        Some("b1".to_owned())
    );
}

/// Contract (`LibraryAudiobooksList.test.ts`): a rail letter no row carries
/// is disabled, and its press scrolls nothing.
#[test]
fn a_rail_letter_no_row_carries_is_disabled() {
    let mut fixture = built::<AudiobooksList>(
        AudiobooksList::frame(AudiobooksList::rows(), false, false),
        AudiobooksList::detail_of(Some("b1")),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"C\"]", "disabled",),
        Some("true".to_owned()),
        "the letterless rail letter paints disabled"
    );

    fixture.jump('C');

    assert_eq!(
        fixture.state.scrolls.audiobooks.peek().scroll_top,
        0.0,
        "the disabled letter's press scrolls nothing"
    );
}
