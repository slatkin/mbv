use super::*;

#[test]
fn go_back_ignores_popped_level_cursor_and_restores_by_parent_id() {
    // Without a prior mirror: the popped child cursor is deliberately
    // stale (99), yet go_back restores the parent cursor to row 2 (the
    // position of the child's parent_id "movie-third") -- by parent_id,
    // not the stale 99 and not a reset 0.
    let mut no_mirror = tv_two_level_model();
    no_mirror.app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(99);
    no_mirror.app.go_back(0);
    assert_eq!(no_mirror.app.libs[0].nav_stack.len(), 1);
    assert_eq!(no_mirror.app.libs[0].nav_stack[0].resting().cursor(), 2);

    // With a prior mirror call that actually mutates the popped child
    // cursor (to 1, the position of the component's selected id within the
    // child items): the restored parent cursor is still 2, identical and
    // not the mutated child cursor.
    let mut with_mirror = tv_two_level_model();
    with_mirror.app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(99);
    with_mirror.app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(1);
    assert_eq!(
        with_mirror.app.libs[0]
            .nav_stack
            .last()
            .unwrap()
            .resting()
            .cursor(),
        1,
        "mirror must mutate the popped child cursor to the component selection"
    );
    with_mirror.app.go_back(0);
    assert_eq!(with_mirror.app.libs[0].nav_stack.len(), 1);
    assert_eq!(with_mirror.app.libs[0].nav_stack[0].resting().cursor(), 2);

    // Season auto-skip: from the Episodes level, one go_back skips the
    // Season level and restores the Series cursor by parent_id (row 2),
    // despite a stale popped cursor (42).
    let mut skip = tv_season_skip_model();
    skip.app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .set_resting_cursor(42);
    skip.app.go_back(0);
    assert_eq!(skip.app.libs[0].nav_stack.len(), 1);
    assert_eq!(skip.app.libs[0].nav_stack[0].resting().cursor(), 2);
}

#[test]
fn cycle_letter_pill_derives_from_filter_not_cursor() {
    // A tvshows library large enough to surface letter pills, at its top
    // browse level with pill bucket 0 (A–C) selected.
    let mut model = super::mounted_tv_model();
    model.app.libs[0].library_total = Some(1000);
    model.app.libs[0].nav_stack[0].letter_filter =
        Some(crate::app::render::LetterFilter::for_index(0).unwrap());

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
    fresh.app.libs[0].nav_stack[0].letter_filter =
        Some(crate::app::render::LetterFilter::for_index(0).unwrap());
    fresh.app.libs[0].nav_stack[0].set_resting_cursor(0);
    fresh.app.cycle_letter_pill(0, 1);
    assert_eq!(
        fresh.app.libs[0].nav_stack[0].letter_filter, after_stale,
        "cycle_letter_pill result must not depend on level.cursor"
    );
}
