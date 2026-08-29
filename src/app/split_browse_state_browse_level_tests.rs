//! Characterization tests for `split-browse-state-interaction-fields` task 4.1.
//!
//! They pin three `BrowseLevel` behaviours the live-cursor / resting-position
//! split (tasks 4.2-4.5) must not regress:
//!   1. position restore on entering a library resolves the saved
//!      `focused_item_id` to a cursor, and round-trips back out;
//!   2. `go_back` re-anchors the parent level's cursor onto the child folder
//!      it popped out of (`actions_navigation.rs:239-278`);
//!   3. pagination prefetch fires exactly when the cursor comes within
//!      `PREFETCH_AHEAD` of the loaded edge (`library_search_actions.rs:240`).

use super::*;
use crate::app::tests::*;

fn movie_level(items: Vec<EmbyItem>, total_count: usize, cursor: usize) -> BrowseLevel {
    BrowseLevel {
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        total_count,
        items,
        cursor,
        scroll: 0,
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }
}

fn movie_lib(nav_stack: Vec<BrowseLevel>) -> LibraryTab {
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    LibraryTab {
        nav_stack,
        ..LibraryTab::new(library)
    }
}

#[test]
fn entering_library_restores_cursor_from_saved_focused_item_and_round_trips() {
    let saved = crate::config::LibraryPositionLevel {
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        focused_item_id: Some("id2".into()),
        cursor_index: 0,
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        letter_filter_index: None,
        library_total: None,
    };

    let level = BrowseLevel::from_position_level(&saved, make_items(5), 5, 3);
    assert_eq!(level.cursor, 2, "saved focused item id2 is at index 2");

    let round_trip = level.to_position_level();
    assert_eq!(round_trip.focused_item_id.as_deref(), Some("id2"));
    assert_eq!(round_trip.cursor_index, 2);
}

#[test]
fn go_back_reanchors_parent_cursor_onto_the_popped_child_folder() {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    let root = movie_level(make_items(3), 3, 0);
    let child = movie_level(Vec::new(), 0, 0);
    let child = BrowseLevel {
        parent_id: "id1".into(),
        ..child
    };
    app.libs.push(movie_lib(vec![root, child]));

    app.go_back(0);

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 1,
        "parent cursor re-anchors onto the child folder (id1) that was popped"
    );
}

#[test]
fn prefetch_holds_until_cursor_is_within_prefetch_ahead_of_loaded_edge() {
    // PREFETCH_AHEAD is 25; 30 of 100 items loaded (not fully loaded).
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.libs.push(movie_lib(vec![movie_level(make_items(30), 100, 4)]));

    // cursor 4: 4 + 25 = 29 < 30 -> still buffered, no fetch.
    app.maybe_fetch_next_page(0, app.libs[0].nav_stack.last().unwrap().cursor);
    assert!(
        !app.libs[0].nav_stack.last().unwrap().loading,
        "cursor comfortably inside the buffer must not trigger a page fetch"
    );

    // cursor 5: 5 + 25 = 30, not < 30 -> threshold reached, fetch starts.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 5;
    app.maybe_fetch_next_page(0, app.libs[0].nav_stack.last().unwrap().cursor);
    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "cursor within PREFETCH_AHEAD of the loaded edge must trigger a page fetch"
    );
}
