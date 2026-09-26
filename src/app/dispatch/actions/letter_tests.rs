use crate::app::state::types::browse::BrowseResting;

use crate::app::dispatch::library::browse::full_library_fetch_limit;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{BrowseLevel, LibraryTab};
use mbv_core::api::EmbyItem;
use rstest::rstest;

fn lib_tab(collection_type: &str) -> LibraryTab {
    let mut library = make_item("Lib", "CollectionFolder");
    library.id = "lib-1".into();
    library.collection_type = collection_type.into();
    LibraryTab::new(library)
}

#[test]
fn active_lib_is_tvshows_true_only_on_a_tvshows_library_tab() {
    // `shuffle_folder` (issue: TV libraries should shuffle from a
    // video-only fetch, everything else from the broader playable-items
    // fetch) branches on this. The matched Emby library index arrives as a
    // parameter (from `shuffle_play` / `execute_context_action`); the index
    // is the library under test, not the selected tab.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    app.libs.push(lib_tab("music"));

    assert!(
        app.active_lib_is_tvshows(0),
        "library 0 is a tvshows library"
    );

    assert!(
        !app.active_lib_is_tvshows(1),
        "library 1 is a music library"
    );
}

/// Pushes a top-level, non-loading, non-searching `BrowseLevel` onto
/// `lib`'s nav_stack -- the minimum state `should_show_letter_pills`
/// needs to consider the library "at its top browse level".
fn push_top_level(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: lib.library.id.clone(),
        title: lib.library.name.clone(),
        items: make_items(item_count),
        total_count: item_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
}

#[test]
fn should_show_letter_pills_true_for_any_captured_total() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);

    assert!(!app.should_show_letter_pills(0));

    app.libs[0].library_total = Some(5);
    assert!(
        app.should_show_letter_pills(0),
        "any captured total qualifies"
    );
}

#[test]
fn should_show_letter_pills_excludes_music_and_drilldowns() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("music"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    assert!(
        !app.should_show_letter_pills(0),
        "music libraries use group pills instead"
    );

    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[1], 10);
    app.libs[1].library_total = Some(1000);
    assert!(app.should_show_letter_pills(1));

    // A second nav level (drilled into a folder) is no longer the "top"
    // browse level.
    push_top_level(&mut app.libs[1], 5);
    assert!(
        !app.should_show_letter_pills(1),
        "hidden below the top browse level"
    );

    app.libs.push(lib_tab("homevideos"));
    push_top_level(&mut app.libs[2], 10);
    app.libs[2].library_total = Some(1000);
    assert!(
        !app.should_show_letter_pills(2),
        "home video libraries use folder pills instead"
    );
}

// Regression coverage for the bug found in review of the letter-pills
// PR: `spawn_all_items_prefetch`/`spawn_search_items_load` used to cap
// their unfiltered fetch's `limit` at `lvl.total_count`, which is the
// FILTERED range's count whenever a letter pill is active (e.g. ~40 for
// an `M–O` pill out of a 3,000-movie library) -- so `all_items` (the set
// `/`-search runs over) silently shrank to just the active range, and
// whole-library search missed everything outside it.
#[test]
fn full_library_fetch_limit_uses_true_total_not_the_filtered_range_count() {
    let mut lib = lib_tab("movies");
    push_top_level(&mut lib, 40); // the "M–O" slice: 40 items
    lib.library_total = Some(3000); // the library's true size
    {
        let lvl = lib.nav_stack.last_mut().unwrap();
        lvl.total_count = 40; // what get_items_sorted_ranged reported for M–O
        lvl.letter_filter = crate::app::render::LetterFilter::for_index_for_kind(
            4,
            crate::app::render::LetterFilterKind::Movie,
        );
    };
    let lvl = lib.nav_stack.last().unwrap();

    assert_eq!(
        full_library_fetch_limit(&lib, lvl),
        3000,
        "must fetch the whole library, not just the active M–O range"
    );
}

fn push_top_level_tv(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: lib.library.id.clone(),
        title: lib.library.name.clone(),
        items: make_items(item_count),
        total_count: item_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Series".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
}

#[rstest]
#[case::should_show_letter_pills_true_for_any_tvshows_total(5)]
fn should_show_letter_pills_true_for_tvshows_total(#[case] total: usize) {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(total);

    assert!(app.should_show_letter_pills(0));
}

#[test]
fn tv_first_capture_resolves_latest_and_replaces_large_library_rows() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 301);
    app.libs[0].nav_stack[0].total_count = 301;

    app.maybe_capture_library_total_and_apply_default_pill(0);

    assert_eq!(
        app.libs[0].tv_content_mode,
        Some(mbv_core::config::TvContentMode::Latest)
    );
    assert!(app.libs[0].nav_stack[0].items.is_empty());
    assert_eq!(
        app.libs[0].nav_stack[0].item_types.as_deref(),
        Some("Episode")
    );
    assert!(app.libs[0].nav_stack[0].loading);
}

fn series(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.into();
    item.is_folder = true;
    item
}

#[test]
fn activate_searched_series_marks_the_series_pill_and_cursor() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 1);
    app.libs[0].library_total = Some(1000);
    let corpus = vec![series("series-a", "Antelope"), series("series-z", "Zebra")];
    {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.all_items = Some(corpus.clone());
        level.items = corpus.clone();
        level.total_count = 2;
    };
    let zebra = series("series-z", "Zebra");

    assert!(app.activate_searched_series(0, &zebra));

    let level = app.libs[0].nav_stack.last().unwrap();
    let filter = level.letter_filter.as_ref().expect("pill group marked");
    assert_eq!(filter.label, "S-Z");
    assert_eq!(
        level
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["series-z"],
        "the level list narrows to the marked pill's range"
    );
    assert_eq!(level.total_count, 1);
    assert_eq!(level.resting().cursor(), 0, "cursor rests on the series");
    assert_eq!(
        level.all_items.as_ref().unwrap().len(),
        2,
        "the search corpus is retained for later searches"
    );
    assert!(!level.loading);
}

#[test]
fn select_letter_pill_scopes_tv_to_series() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    app.select_letter_pill(0, 1); // "J-R"

    let lvl = app.libs[0].nav_stack.last().unwrap();
    let filter = lvl.letter_filter.as_ref().expect("pill should be set");
    assert_eq!(filter.index, 1);
    assert_eq!(filter.label, "J-R");
    assert_eq!(filter.name_ge, Some("J"));
    assert_eq!(filter.name_lt, Some("S"));
    assert_eq!(lvl.item_types, Some("Series".to_string()));
    assert_eq!(lvl.resting().cursor(), 0);
    assert_eq!(lvl.resting().scroll(), 0);
    assert!(lvl.loading, "TV scoped refresh should be in flight");
}

#[test]
fn cycle_letter_pill_wraps_on_tvshows_library() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    app.cycle_letter_pill(0, -1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(
        filter.label, "S-Z",
        "wrapping back from default should land on S-Z"
    );

    app.cycle_letter_pill(0, 1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(
        filter.label, "A-I",
        "wrapping forward from S-Z should land on A-I"
    );
}
