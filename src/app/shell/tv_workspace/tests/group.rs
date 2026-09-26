use super::*;

#[test]
fn cycle_letter_pill_derives_from_filter_not_cursor() {
    // A tvshows library large enough to surface letter pills, at its top
    // browse level with pill bucket 0 (A-I) selected.
    let mut model = super::mounted_tv_model();
    model.app.libs[0].library_total = Some(1000);
    model.app.libs[0].nav_stack[0].letter_filter = Some(
        crate::app::render::LetterFilter::for_index_for_kind(
            0,
            crate::app::render::LetterFilterKind::Tv,
        )
        .unwrap(),
    );

    // Stale cursor: cycle_letter_pill must ignore `level.cursor` and
    // advance the filter 0 -> 1 purely from `letter_filter`.
    model.app.libs[0].nav_stack[0].set_resting_cursor(7);
    model.app.cycle_letter_pill(0, 1);
    let after_stale = model.app.libs[0].nav_stack[0].letter_filter.clone();
    assert_eq!(after_stale.as_ref().map(|f| f.index), Some(1));
    assert!(model.app.libs[0].nav_stack[0].loading);
    // select_letter_pill intentionally resets the level cursor to 0,
    // regardless of its prior (stale) value.
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "select_letter_pill resets the level cursor regardless of its prior value"
    );

    // Fresh cursor at a different value: identical filter result, proving
    // the cycle never consults `level.cursor`.
    let mut fresh = super::mounted_tv_model();
    fresh.app.libs[0].library_total = Some(1000);
    fresh.app.libs[0].nav_stack[0].letter_filter = Some(
        crate::app::render::LetterFilter::for_index_for_kind(
            0,
            crate::app::render::LetterFilterKind::Tv,
        )
        .unwrap(),
    );
    fresh.app.libs[0].nav_stack[0].set_resting_cursor(0);
    fresh.app.cycle_letter_pill(0, 1);
    assert_eq!(
        fresh.app.libs[0].nav_stack[0].letter_filter, after_stale,
        "cycle_letter_pill result must not depend on level.cursor"
    );
}
