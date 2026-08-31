#![allow(dead_code, unused_imports)]
use crate::app::types_browse::BrowseResting;

use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, QueueScope, TabSelection,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerEvent;
use std::collections::HashMap;
use std::sync::mpsc;

fn folder(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

fn album(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "MusicAlbum");
    item.id = id.into();
    item.is_folder = true;
    item.media_type = "Audio".into();
    item
}

fn recursive_music_app() -> App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".into(), "artist".into(), "album".into()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-lib".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app
}
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

#[test]
fn active_lib_is_tvshows_bounds_miss_returns_false() {
    // The explicit-index contract: a bounds-miss index (async Service
    // removal between dispatch normalization and use) must return false --
    // never panic and never default to another library.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));

    assert!(!app.active_lib_is_tvshows(1), "index past the last library");
}

/// Pushes a top-level, non-loading, non-searching `BrowseLevel` onto
/// `lib`'s nav_stack -- the minimum state `should_show_letter_pills`
/// needs to consider the library "at its top browse level".
fn push_top_level(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
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
fn should_show_letter_pills_false_while_inline_search_active() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    assert!(app.should_show_letter_pills(0));
    app.inline_search_active = true;
    assert!(!app.should_show_letter_pills(0));
}

#[test]
fn select_letter_pill_is_noop_while_inline_search_active() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    app.inline_search_active = true;

    app.select_letter_pill(0, 0);

    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .is_none());
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

#[test]
fn inline_search_mounts_at_the_component_boundary() {
    let mut model = crate::app::shell::Model::new(crate::app::render::make_movie_app());
    model.open_inline_search();
    let id = model
        .inline_search_component_id(0)
        .expect("inline Search component mounted");
    assert!(model.application.mounted(&id));
}

#[test]
fn select_letter_pill_scopes_the_level_and_resets_cursor() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    app.libs[0].nav_stack[0].set_resting_cursor(4);
    app.libs[0].nav_stack[0].set_resting_scroll(2);

    app.select_letter_pill(0, 4); // "M–O"

    let lvl = app.libs[0].nav_stack.last().unwrap();
    let filter = lvl.letter_filter.as_ref().expect("pill should be set");
    assert_eq!(filter.index, 4);
    assert_eq!(filter.label, "M\u{2013}O");
    assert_eq!(filter.name_ge, Some("M"));
    assert_eq!(filter.name_lt, Some("P"));
    assert_eq!(lvl.resting().cursor(), 0);
    assert_eq!(lvl.resting().scroll(), 0);
    assert!(lvl.loading, "a scoped refresh should be in flight");
}

#[test]
fn select_letter_pill_is_a_noop_outside_letter_pill_eligibility() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    // library_total never captured -> should_show_letter_pills is false.

    app.select_letter_pill(0, 0);

    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .is_none());
}

#[test]
fn cycle_letter_pill_wraps_around() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    // Default (no pill selected yet) is treated as index 0; cycling back
    // wraps to the last bucket ("#").
    app.cycle_letter_pill(0, -1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(filter.label, "#");
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
        lvl.letter_filter = crate::app::render::LetterFilter::for_index(4);
    }
    let lvl = lib.nav_stack.last().unwrap();

    assert_eq!(
        full_library_fetch_limit(&lib, lvl),
        3000,
        "must fetch the whole library, not just the active M–O range"
    );
}

#[test]
fn full_library_fetch_limit_falls_back_to_total_count_before_library_total_is_known() {
    let mut lib = lib_tab("movies");
    push_top_level(&mut lib, 10);
    // library_total not yet captured (e.g. first-ever load in flight).
    let lvl = lib.nav_stack.last().unwrap();
    assert_eq!(full_library_fetch_limit(&lib, lvl), 10);
}

fn push_top_level_tv(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
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
        music_grouping: None,
    });
}

#[test]
fn should_show_letter_pills_true_for_large_tvshows_library() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(301);

    assert!(
        app.should_show_letter_pills(0),
        "large tvshows library should show letter pills"
    );
}

#[test]
fn should_show_letter_pills_true_for_any_tvshows_total() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(5);

    assert!(
        app.should_show_letter_pills(0),
        "any captured tvshows total qualifies"
    );
}

#[test]
fn select_letter_pill_scopes_tv_to_series() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    app.select_letter_pill(0, 4); // "M–O"

    let lvl = app.libs[0].nav_stack.last().unwrap();
    let filter = lvl.letter_filter.as_ref().expect("pill should be set");
    assert_eq!(filter.index, 4);
    assert_eq!(filter.label, "M\u{2013}O");
    assert_eq!(filter.name_ge, Some("M"));
    assert_eq!(filter.name_lt, Some("P"));
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
        filter.label, "#",
        "wrapping back from default should land on #"
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
        filter.label, "A\u{2013}C",
        "wrapping forward from # should land on A–C"
    );
}

#[test]
fn select_letter_pill_is_noop_on_tv_drilldown() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    // Push a second level (season drilldown) — no longer top-level
    push_top_level_tv(&mut app.libs[0], 5);

    app.select_letter_pill(0, 0);
    let lvl = app.libs[0].nav_stack.last().unwrap();
    assert!(
        lvl.letter_filter.is_none(),
        "pill selection should be ignored below the top browse level"
    );
}
