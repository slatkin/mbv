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
    let level = mbv_queue::LibraryPositionLevel {
        fetched_rows: None,
        parent_id: "lib-movies".into(),
        title: "Power".into(),
        focused_item_id: Some("id1".into()),
        ..Default::default()
    };
    let position = mbv_queue::LibraryPosition {
        levels: vec![level.clone()],
        ..Default::default()
    };
    app.replace_saved_library_position(0, position.clone());
    // 2 items / 50 total: not fully loaded, so `spawn_all_items_prefetch` would do I/O.
    let level = BrowseLevel::from_position_level(&level, make_items(2), 50, 10);
    app.handle_lib_event(LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack: vec![level],
    }));
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
    let pre_feature_position = mbv_queue::LibraryPosition {
        levels: vec![mbv_queue::LibraryPositionLevel {
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

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: pre_feature_position.clone(),
        position: pre_feature_position,
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(673),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
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
    }));

    assert_eq!(app.libs[0].library_total, Some(673));
    assert!(app.should_show_letter_pills(0));
    assert_eq!(
        app.libs[0].nav_stack[0].letter_filter,
        Some(mbv_render::LetterFilter::default_filter_for_kind(
            mbv_render::LetterFilterKind::Movie
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
    let requested = mbv_queue::LibraryPosition {
        levels: vec![mbv_queue::LibraryPositionLevel {
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

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: requested.clone(),
        position: requested,
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(2),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),

            resting: mbv_ui_model::browse::BrowseResting::new(1, 0),
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
    }));

    assert!(app.libs[0].nav_stack.is_empty());
    assert!(
        !crate::config::load_library_position_state()
            .libraries
            .contains_key("lib-movies")
    );
}

/// Spec `destination-latest-modes`, scenario "TV keeps its established
/// default" (change `latest-pill-restart-default`): a restart no longer
/// carries a saved TV mode (the position document's mode is cleared on
/// load), so the restored TV library resolves its count-dependent default
/// from the library's total — Latest above the pill threshold, All at or
/// below it.
#[rstest]
#[case::large_library_opens_on_latest(673, mbv_queue::TvContentMode::Latest)]
#[case::small_library_opens_on_all(12, mbv_queue::TvContentMode::All)]
fn restoring_a_tv_position_without_a_saved_mode_resolves_the_count_dependent_default(
    #[case] total: usize,
    #[case] expected: mbv_queue::TvContentMode,
) {
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    let position = mbv_queue::LibraryPosition {
        levels: vec![mbv_queue::LibraryPositionLevel {
            fetched_rows: None,
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
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
    app.replace_saved_library_position(0, position.clone());
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::EmbyLibrary(0);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(total),
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: make_items(2),

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
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
    }));

    assert_eq!(app.libs[0].tv_content_mode, Some(expected.clone()));
    assert_eq!(app.libs[0].nav_stack[0].tv_content_mode, Some(expected));
}
