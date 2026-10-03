use super::*;

/// One Emby library, one populated Audiobookshelf library, and a feed
/// subscription, so a mis-targeted `refresh_current_view` would have other
/// destinations' state to disturb.
fn mixed_services_app() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(1),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(1),

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
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
        ..LibraryTab::new(library)
    });
    let abs_library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state =
        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.append_page(
        0,
        20,
        1,
        vec![mbv_audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    abs_state.detail_cache.insert(
        "show-a".into(),
        vec![mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            description: None,
            published_at: None,
            duration_seconds: None,
        }],
    );
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_queue::FeedKind::Audio,
    }];
    app.feed_tab.entries.resize_with(1, Vec::new);
    app
}

/// F5 with a library selected reloads only that Emby library: it marks the
/// matched library's browse level loading, keeps the session split width and
/// the saved resting position, and leaves the Audiobookshelf catalog and the
/// Feeds tab untouched (#745).
#[test]
fn refresh_current_view_targets_the_selected_emby_library_only() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.list_pane_width = Some(64);
    app.replace_saved_library_position(0, mbv_queue::LibraryPosition::default());

    app.refresh_current_view();

    assert!(
        app.libs[0].nav_stack[0].loading,
        "selected Emby library must be marked loading"
    );
    assert_eq!(
        app.list_pane_width,
        Some(64),
        "F5 must not reset the Wide hero split"
    );
    assert!(
        app.saved_library_position(0).is_some(),
        "F5 must not clear the saved library position"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].shows.len(),
        1,
        "Audiobookshelf catalog must not be cleared"
    );
    assert!(
        app.audiobookshelf_browse[0]
            .detail_cache
            .contains_key("show-a"),
        "Audiobookshelf episodes must be preserved"
    );
    assert!(!app.feed_tab.loading, "Feeds must not be refreshed");
}

/// The split is one session field shared by every Wide hero surface: switching
/// wide library tabs neither clears nor re-clamps it, so the next surface
/// consumes the same raw width (clamped against its own content area at paint
/// time).
/// F5 refreshes the selected browse destination even while Queue holds Panel
/// focus; the unselected destinations are not the target (#745).
#[test]
fn refresh_current_view_with_queue_focus_refreshes_the_selected_destination() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;

    app.refresh_current_view();

    assert!(
        app.libs[0].nav_stack[0].loading,
        "Queue-focused F5 must refresh the selected Emby library"
    );
    assert_eq!(app.audiobookshelf_browse[0].shows.len(), 1);
    assert!(
        app.audiobookshelf_browse[0]
            .detail_cache
            .contains_key("show-a")
    );
    assert!(!app.feed_tab.loading);
}

#[test]
fn switching_wide_library_tabs_keeps_the_session_split_width() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.list_pane_width = Some(64);

    app.set_library_tab(0); // Home (a Wide hero surface)
    assert_eq!(app.list_pane_width, Some(64), "switch to Home");
    app.set_library_tab(1); // back to the Emby library
    assert_eq!(app.list_pane_width, Some(64), "switch back to a library");
}

/// A stale Service library index (removed or replaced while selected) must
/// select Home and report that the triggering destination-specific
/// operation must stop.
#[test]
fn normalize_stale_browse_destination_resolves_stale_service_indexes_to_home() {
    // Stale Emby index with no Emby libraries.
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    assert!(app.normalize_stale_browse_destination());
    assert!(app.tab.is_home());

    // Stale Audiobookshelf index with no Audiobookshelf libraries.
    let mut app = make_app_stub();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    assert!(app.normalize_stale_browse_destination());
    assert!(app.tab.is_home());

    // A valid Audiobookshelf index is not stale even when Emby has none.
    let mut app = mixed_services_app();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::AudiobookshelfLibrary(0));
}
