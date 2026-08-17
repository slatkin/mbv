use super::*;
use crate::app::tests::*;

/// One Emby library, one populated Audiobookshelf library, and a feed
/// subscription, so a mis-targeted `refresh_current_view` would have other
/// destinations' state to disturb.
fn mixed_services_app() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(1),
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let abs_library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state =
        super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    abs_state.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab.entries.resize_with(1, Vec::new);
    app
}

/// F5 with a library selected reloads only that Emby library: it marks the
/// matched library's browse level loading and leaves the Audiobookshelf
/// catalog and the Feeds tab untouched.
#[test]
fn refresh_current_view_targets_the_focused_emby_library_only() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;

    app.refresh_current_view();

    assert!(
        app.libs[0].nav_stack[0].loading,
        "focused Emby library must be marked loading"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].shows.len(),
        1,
        "Audiobookshelf catalog must not be cleared"
    );
    assert!(
        app.audiobookshelf_browse[0].episodes.is_some(),
        "Audiobookshelf episodes must be preserved"
    );
    assert!(!app.feed_tab.loading, "Feeds must not be refreshed");
}

/// F5 with the queue panel focused refreshes only the visible queue and
/// leaves every browse destination (Emby, Audiobookshelf, Feeds) untouched.
#[test]
fn refresh_current_view_with_queue_focus_leaves_browse_destinations_untouched() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;

    app.refresh_current_view();

    assert!(
        !app.libs[0].nav_stack[0].loading,
        "queue refresh must not reload the Emby library"
    );
    assert_eq!(app.audiobookshelf_browse[0].shows.len(), 1);
    assert!(app.audiobookshelf_browse[0].episodes.is_some());
    assert!(!app.feed_tab.loading);
}

/// A stale Emby `lib_idx` reaching a bounds-checked browse helper mutates
/// nothing and never corrupts another library via a library-zero fallback.
#[test]
fn stale_emby_lib_index_mutates_no_library() {
    let mut app = two_emby_libraries_app();
    let stale = app.libs.len() + 1;
    app.move_lib_cursor(stale, 1);
    app.jump_lib_cursor(stale, true);
    app.go_back(stale);
    app.shuffle_play(stale);
    app.refresh_lib(stale);
    for idx in 0..2 {
        let lvl = &app.libs[idx].nav_stack[0];
        assert_eq!(lvl.cursor, 0, "lib {idx} cursor");
        assert!(!lvl.loading, "lib {idx} not loading");
    }
}
/// Two Emby libraries (each a two-item top browse level) so a stale index
/// that wrongly fallbacked to library zero could corrupt real state.
fn two_emby_libraries_app() -> App {
    let mut app = make_app_stub();
    for (id, title) in [("lib-movies", "Movies"), ("lib-music", "Music")] {
        let mut library = make_item(title, "CollectionFolder");
        library.id = id.into();
        let level = BrowseLevel {
            parent_id: id.into(),
            title: title.into(),
            items: make_items(2),
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        };
        app.libs.push(LibraryTab {
            library,
            search: None,
            nav_stack: vec![level],
            feed_home_video: None,
            album_track_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
    }
    app
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

/// Valid destinations and the non-Service tabs must be left unchanged and
/// report `false`.
#[test]
fn normalize_stale_browse_destination_leaves_valid_destinations_alone() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));

    app.tab = TabSelection::Home;
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::Home);

    app.tab = TabSelection::Feeds;
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::Feeds);
}
