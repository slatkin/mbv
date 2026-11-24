fn tv_letter_pill(index: usize) -> mbv_ui_model::sort_filter::LetterFilter {
    mbv_ui_model::sort_filter::LetterFilter::for_index_for_kind(
        index,
        mbv_ui_model::sort_filter::LetterFilterKind::Tv,
    )
    .expect("the TV letter buckets exist")
}

#[test]
fn cycle_letter_pill_derives_from_filter_not_cursor() {
    // A tvshows library large enough to surface letter pills, at its top
    // browse level with pill bucket 0 (A-I) selected.
    let mut model = super::mounted_tv_model();
    model.app.set_library_total_for_test(0, 1000);
    model
        .app
        .set_library_level_letter_filter_for_test(0, 0, tv_letter_pill(0));

    // Stale cursor: cycle_letter_pill must ignore `level.cursor` and
    // advance the filter 0 -> 1 purely from `letter_filter`.
    model.app.set_library_level_cursor_for_test(0, 0, 7);
    model.app.cycle_letter_pill(0, 1);
    let after_stale = model
        .app
        .emby_library_level(0)
        .and_then(|level| level.letter_filter.clone());
    assert_eq!(after_stale.as_ref().map(|f| f.index), Some(1));
    assert!(
        model
            .app
            .emby_library_level(0)
            .is_some_and(|level| level.loading)
    );
    // select_letter_pill intentionally resets the level cursor to 0,
    // regardless of its prior (stale) value.
    assert_eq!(
        model.app.emby_library_level_cursor_for_test(0, 0),
        Some(0),
        "select_letter_pill resets the level cursor regardless of its prior value"
    );

    // Fresh cursor at a different value: identical filter result, proving
    // the cycle never consults `level.cursor`.
    let mut fresh = super::mounted_tv_model();
    fresh.app.set_library_total_for_test(0, 1000);
    fresh
        .app
        .set_library_level_letter_filter_for_test(0, 0, tv_letter_pill(0));
    fresh.app.set_library_level_cursor_for_test(0, 0, 0);
    fresh.app.cycle_letter_pill(0, 1);
    assert_eq!(
        fresh
            .app
            .emby_library_level(0)
            .and_then(|level| level.letter_filter.as_ref()),
        after_stale.as_ref(),
        "cycle_letter_pill result must not depend on level.cursor"
    );
}
