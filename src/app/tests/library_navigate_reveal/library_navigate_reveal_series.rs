use super::*;
use crate::app::state::types::events::NavigateLanding;
use std::time::Duration;

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

#[test]
fn series_landing_miss_flashes_and_leaves_the_active_tab_unchanged() {
    // Task 2.2: a miss (series absent from the level corpus) flashes the
    // library-error path and leaves the active tab unchanged, mirroring
    // task 4.2's failure handling.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;
    let (stack_len, first_parent, first_cursor) = (
        app.libs[0].nav_stack.len(),
        app.libs[0].nav_stack[0].parent_id.clone(),
        app.libs[0].nav_stack[0].resting().cursor(),
    );

    let mut absent = make_item("Missing Show", "Series");
    absent.id = "ser-absent".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(absent),
            episode_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(app.libs[0].nav_stack.len(), stack_len);
    assert_eq!(app.libs[0].nav_stack[0].parent_id, first_parent);
    assert_eq!(
        app.libs[0].nav_stack[0].resting().cursor(),
        first_cursor,
        "the root level is untouched by a miss"
    );
    assert!(
        app.status.contains("Could not land on"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
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
fn series_landing_on_an_unloaded_library_waits_then_lands_on_the_loaded_drain() {
    // U2 correction (finding 1, spec P1): the queue "Go to Library" on an
    // episode of a never-visited library must LAND, not flash. The arm
    // materializes the root level via `ensure_lib_loaded_for`, and the
    // `Loaded` drain retries the landing: land, save position, switch (D4).
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;
    // The series-detail fetch is orthogonal here; a cache hit keeps the
    // script to the single browse response.
    app.series_detail_cache.insert(
        "ser1".into(),
        SeriesDetail {
            seasons: Vec::new(),
            episodes: std::collections::HashMap::new(),
        },
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser0","Name":"Other Show","Type":"Series"},{"Id":"ser1","Name":"The Show","Type":"Series"}],"TotalRecordCount":2}"#,
    );

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(series_item("ser1", "The Show")),
            episode_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home, "no tab yank before landing");
    assert!(
        app.pending_series_landing.is_some(),
        "the landing is armed, not flashed"
    );
    assert!(
        !app.status.contains("Could not land on"),
        "an unloaded library is not a miss: {}",
        app.status
    );
    assert_eq!(app.libs[0].nav_stack.len(), 1, "root level materialized");

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("root browse loaded");
    assert!(matches!(ev, LibEvent::Loaded { .. }), "expected Loaded");
    app.handle_lib_event(ev);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0), "landed and switched");
    assert!(app.pending_series_landing.is_none(), "pending consumed");
    let level = &app.libs[0].nav_stack[0];
    let cursor = level.resting().cursor();
    assert_eq!(level.items[cursor].id, "ser1", "cursor on the show");
    let saved = app
        .saved_library_position(0)
        .expect("the landed state is the saved position (D4)");
    assert_eq!(saved.levels[0].focused_item_id.as_deref(), Some("ser1"));
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

#[test]
fn pending_series_landing_survives_a_foreign_library_drain() {
    // U2 correction (finding 1 wiring): a drain for another library must not
    // consume or move the pending landing.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_paginated_tv_library();
    app.tab = TabSelection::Home;
    app.pending_series_landing = Some(crate::app::state::types::events::PendingSeriesLanding {
        lib_idx: 0,
        reveal: Box::new(series_item("ser1", "The Show")),
        switch_tab: true,
        episode_id: None,
    });
    let mut other_lib = make_item("Music", "CollectionFolder");
    other_lib.id = "lib-other".into();
    other_lib.collection_type = "music".into();
    app.libs.push(LibraryTab::new(other_lib));

    app.handle_lib_event(LibEvent::AllItemsPrefetched {
        lib_idx: 1,
        parent_id: "lib-other".into(),
        items: vec![series_item("ser1", "The Show")],
    });

    assert!(
        app.pending_series_landing.is_some(),
        "a foreign library's drain leaves the pending landing armed"
    );
    assert_eq!(app.tab, TabSelection::Home);
}

#[test]
fn completed_series_landing_handoff_survives_an_unrelated_error_drain() {
    // P2 correction: the hand-off is armed only after the landing already
    // succeeded, so an unrelated `LibEvent::Error` drain must not swallow the
    // owed workspace/overlay open. The pre-landing pending landing is still
    // dropped by the same drain.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(series_item("ser1", "The Show")),
            episode_id: None,
        },
        switch_tab: true,
    });
    assert!(
        app.pending_series_handoff.is_some(),
        "the completed landing armed the hand-off"
    );
    // A second, still-unresolved landing: the error drain must drop it.
    app.pending_series_landing = Some(crate::app::state::types::events::PendingSeriesLanding {
        lib_idx: 0,
        reveal: Box::new(series_item("ser2", "Third Show")),
        switch_tab: true,
        episode_id: None,
    });

    app.handle_lib_event(LibEvent::Error("an unrelated refresh failed".into()));

    assert!(
        app.pending_series_handoff.is_some(),
        "a completed landing's hand-off survives an unrelated error"
    );
    assert!(
        app.pending_series_landing.is_none(),
        "the pre-landing pending is still dropped by the error drain"
    );
}
