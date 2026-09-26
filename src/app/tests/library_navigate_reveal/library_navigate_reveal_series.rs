use super::*;
use crate::app::state::types::events::NavigateLanding;

#[test]
fn series_landing_applies_the_searched_series_activation_on_the_root_level() {
    // Task 2.2: the App-side Series arm lands the show through
    // `activate_searched_series` on the target library's current root level
    // (letter pill included), then replaces the saved Library position with
    // the landed state before the tab switch (D4). Root-level-only: no new
    // browse level is pushed.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;

    let mut show = make_item("The Show", "Series");
    show.id = "ser1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(show),
            episode_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "root-level-only landing: no level below the series list"
    );
    let level = &app.libs[0].nav_stack[0];
    assert!(level.letter_filter.is_some(), "letter pill applied");
    let cursor = level.resting().cursor();
    assert_eq!(
        level.items[cursor].id, "ser1",
        "cursor rests on the navigated show within the filtered corpus"
    );
    assert!(!level.loading);
    // The landed state is the saved position (D4).
    let saved = app
        .saved_library_position(0)
        .expect("landing replaces the saved position");
    assert_eq!(saved.levels[0].focused_item_id.as_deref(), Some("ser1"));
    assert_eq!(saved.levels[0].cursor_index, cursor);
}

// ── U2 correction: ensure-then-land Series, per-kind artist, lifecycle ──

/// A TV library tab whose root level exists but only carries the first page
/// of a longer listing (`total_count` beyond `items`), so the pending Series
/// landing must wait for the whole-library prefetch.
fn app_with_paginated_tv_library() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    app.libs[0].library_total = Some(5);
    let mut other = make_item("Other Show", "Series");
    other.id = "ser0".into();
    app.libs[0].nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: "lib-tv".into(),
        title: "TV".into(),
        items: vec![other],
        total_count: 5,
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
    app
}

fn series_item(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.into();
    item
}

#[test]
fn series_landing_waits_for_the_whole_library_prefetch_on_a_paginated_root() {
    // U2 correction (finding 1): a series outside the loaded page needs the
    // whole-library `all_items` corpus; the pending landing retries on its
    // `AllItemsPrefetched` drain.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_paginated_tv_library();
    app.tab = TabSelection::Home;
    let mut show = series_item("ser1", "The Show");
    show.id = "ser1".into();

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(show),
            episode_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home);
    assert!(app.pending_series_landing.is_some());
    assert!(!app.status.contains("Could not land on"), "{}", app.status);

    app.handle_lib_event(LibEvent::AllItemsPrefetched {
        lib_idx: 0,
        parent_id: "lib-tv".into(),
        items: vec![
            series_item("ser0", "Other Show"),
            series_item("ser1", "The Show"),
            series_item("ser2", "Third Show"),
        ],
    });

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0), "landed on the drain");
    assert!(app.pending_series_landing.is_none());
    let level = &app.libs[0].nav_stack[0];
    assert_eq!(
        level.items[level.resting().cursor()].id,
        "ser1",
        "cursor on the series within the prefetched corpus"
    );
}

#[test]
fn series_landing_miss_after_the_whole_library_load_flashes_and_clears() {
    // U2 correction (finding 1): the miss rule is reserved for a genuinely
    // absent item. Once the whole-library corpus is in hand and still lacks
    // the show, the pending landing flashes and leaves the tab unchanged.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_paginated_tv_library();
    app.tab = TabSelection::Home;

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(series_item("ser-absent", "Missing Show")),
            episode_id: None,
        },
        switch_tab: true,
    });
    assert!(app.pending_series_landing.is_some());

    app.handle_lib_event(LibEvent::AllItemsPrefetched {
        lib_idx: 0,
        parent_id: "lib-tv".into(),
        items: vec![
            series_item("ser0", "Other Show"),
            series_item("ser2", "Third Show"),
        ],
    });

    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert!(
        app.pending_series_landing.is_none(),
        "a complete-corpus miss clears the pending landing"
    );
    assert!(
        app.status.contains("Could not land on"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert_eq!(
        app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the root level stays untouched by a miss"
    );
}
