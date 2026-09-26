use super::*;

#[test]
fn restoring_library_position_does_not_eagerly_prefetch_all_items() {
    // #260: restoring a library position must not eagerly prefetch all items.
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::EmbyLibrary(0);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab::new(library));
    let level = crate::config::LibraryPositionLevel {
        fetched_rows: None,
        parent_id: "lib-movies".into(),
        title: "Power".into(),
        focused_item_id: Some("id1".into()),
        ..Default::default()
    };
    let position = crate::config::LibraryPosition {
        levels: vec![level.clone()],
        ..Default::default()
    };
    app.replace_saved_library_position(0, position.clone());
    // 2 items / 50 total: not fully loaded, so `spawn_all_items_prefetch` would do I/O.
    let level = BrowseLevel::from_position_level(&level, make_items(2), 50, 10);
    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack: vec![level],
    });
    assert_eq!(app.libs[0].nav_stack[0].title, "Power");
    assert!(app.libs[0].nav_stack[0].all_items.is_none());
}

#[test]
fn restoring_pre_pill_feature_position_captures_library_total_and_shows_pills() {
    // Regression test: a `LibraryPosition` saved before the
    // letter-range-pill feature existed carries `library_total: None`
    // and `letter_filter_index: None`. Restoring such a position must
    // still capture `library_total` from the restored level's
    // `total_count` (via `maybe_capture_library_total_and_apply_default_pill`)
    // so `should_show_letter_pills` becomes true for large libraries --
    // otherwise the pill row never appears for any library opened
    // before this feature shipped.
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab::new(library));
    let pre_feature_position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            fetched_rows: None,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            focused_item_id: None,
            cursor_index: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            tv_content_mode: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, pre_feature_position.clone());
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::EmbyLibrary(0);

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: pre_feature_position.clone(),
        position: pre_feature_position,
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 673,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
    });

    assert_eq!(app.libs[0].library_total, Some(673));
    assert!(app.should_show_letter_pills(0));
    assert_eq!(
        app.libs[0].nav_stack[0].letter_filter,
        Some(super::render::LetterFilter::default_filter_for_kind(
            super::render::LetterFilterKind::Movie
        )),
        "large restored library should get the default A-C pill applied"
    );
}

#[test]
fn stale_restore_is_ignored_after_saved_position_is_cleared() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab::new(library));
    let requested = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            fetched_rows: None,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            tv_content_mode: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, requested.clone());
    app.clear_saved_library_position(0);

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: requested.clone(),
        position: requested,
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 2,
            resting: crate::app::state::types::browse::BrowseResting::new(1, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
    });

    assert!(app.libs[0].nav_stack.is_empty());
    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}
