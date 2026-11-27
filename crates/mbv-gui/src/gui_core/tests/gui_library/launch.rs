//! The GUI launch file's tests: one save and restart round trip per
//! collection dropdown, each destination's list sort, and the stale-id
//! fallback to All.

use super::*;
use rstest::rstest;

const FEED_A: &str = "https://example.test/a";
const FEED_B: &str = "https://example.test/b";

/// A folder row of the feed-view library's root listing in the launch fixture.
fn folder_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.to_string();
    item.is_folder = true;
    item
}

/// A stub `App` with the active destination and landed catalog of one
/// collection dropdown: the saved selection's own identities, and no fetch
/// behind the install, so the test observes the launch path alone.
fn launch_roundtrip_app(target: &GuiCollectionTarget) -> App {
    match target {
        GuiCollectionTarget::Music(_) => {
            let http = MockHttp::new();
            let mut app = app_with_music_groups(&http);
            app.disconnect_emby_client_for_test();
            app
        }
        GuiCollectionTarget::Podcasts(_) => {
            let mut app = make_app_stub();
            app.seed_gui_abs_libraries_for_test();
            app.seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A"), ("show-b", "Show B")]);
            app.seed_active_tab_for_test(TabSelection::AudiobookshelfLibrary(1));
            app
        }
        GuiCollectionTarget::Feeds => {
            let mut app = make_app_stub();
            app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
            app.seed_active_tab_for_test(TabSelection::Feeds);
            app
        }
        GuiCollectionTarget::EmbyFeed(_) => {
            let mut app = make_app_stub();
            let mut config = app.config_for_test(|config| config.clone());
            config.server_url = "http://127.0.0.1:1".into();
            install_test_emby(&mut app, config);
            app.config_for_test(|config| config.feed_view_libraries = vec!["youtube".into()]);
            app.disconnect_emby_client_for_test();
            let mut library = make_item("YouTube", "CollectionFolder");
            library.id = "lib-feed".into();
            library.collection_type = "homevideos".into();
            library.is_folder = true;
            let mut tab = LibraryTab::new(library);
            tab.nav_stack.push(level(
                "lib-feed",
                "YouTube",
                vec![
                    folder_item("folder-a", "Folder A"),
                    folder_item("folder-b", "Folder B"),
                ],
            ));
            app.push_library_for_test(tab);
            app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
            app
        }
    }
}

/// Seed one destination's session selection as the user's presses leave it.
fn seed_launch_selection(app: &mut App, target: &GuiCollectionTarget, keys: &[&str]) {
    let _ = app;
    let set = GuiCollectionSet::Checked(keys.iter().map(ToString::to_string).collect());
    let selection = GuiCollectionSelection { set };
    crate::gui_core::test_support::stub_gui_state(|state| match target {
        GuiCollectionTarget::Music(lib_idx) => {
            state.library_groups.insert(*lib_idx, selection);
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            state.podcast_libs.entry(*lib_idx).or_default().selection = selection;
        }
        GuiCollectionTarget::Feeds => {
            state.feeds_selection.selection = selection;
        }
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            state.emby_feed_sets.insert(*lib_idx, selection);
        }
    });
}

/// The stored set one destination holds after the restore.
fn restored_launch_selection(app: &App, target: &GuiCollectionTarget) -> GuiCollectionSet {
    let _ = app;
    crate::gui_core::test_support::stub_gui_state(|state| match target {
        GuiCollectionTarget::Music(lib_idx) => state
            .library_groups
            .get(lib_idx)
            .expect("the set was restored")
            .set
            .clone(),
        GuiCollectionTarget::Podcasts(lib_idx) => state
            .podcast_libs
            .get(lib_idx)
            .expect("the set was restored")
            .selection
            .set
            .clone(),
        GuiCollectionTarget::Feeds => state.feeds_selection.selection.set.clone(),
        GuiCollectionTarget::EmbyFeed(lib_idx) => state
            .emby_feed_sets
            .get(lib_idx)
            .expect("the set was restored")
            .set
            .clone(),
    })
}

/// Seed one destination's sort as the user's press leaves it (gui-flat-list-sort
/// design D10).
fn seed_launch_sort(app: &mut App, sort: (GuiSortTarget, GuiListSort)) {
    set_gui_list_sort(app, sort.0, sort.1);
}

/// Every collection dropdown's checked set, in saved order, and its list sort,
/// survive a save and a restart (bug fix: only Music used to persist;
/// gui-flat-list-sort design D10).
#[rstest]
#[case::music(GuiCollectionTarget::Music(0), &[SOUL, HIPHOP], None)]
#[case::podcasts(
    GuiCollectionTarget::Podcasts(1),
    &["show-a", "show-b"],
    Some((GuiSortTarget::Podcasts(1), GuiListSort::Oldest))
)]
#[case::feeds(
    GuiCollectionTarget::Feeds,
    &[FEED_B, FEED_A],
    Some((GuiSortTarget::Feeds, GuiListSort::Oldest))
)]
#[case::emby_feed(
    GuiCollectionTarget::EmbyFeed(0),
    &["folder-b", "folder-a"],
    Some((GuiSortTarget::Library(0), GuiListSort::Oldest))
)]
fn a_saved_checked_set_restores_as_saved(
    #[case] target: GuiCollectionTarget,
    #[case] keys: &[&str],
    #[case] sort: Option<(GuiSortTarget, GuiListSort)>,
) {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut source = launch_roundtrip_app(&target);
    seed_launch_selection(&mut source, &target, keys);
    if let Some(sort) = sort {
        seed_launch_sort(&mut source, sort);
    }
    let saved = crate::gui_core::GuiCore::new(source).gui_launch_state();
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");

    let mut restored = launch_roundtrip_app(&target);

    assert!(matches!(
        crate::gui_core::test_support::open_launch_destination(&mut restored, &loaded),
        GuiLaunchOpen::Opened(_)
    ));
    assert!(crate::gui_core::test_support::restore_launch_collections(
        &mut restored,
        &loaded,
        &loaded.destination
    ));
    let set = restored_launch_selection(&restored, &target);
    let expected: Vec<String> = keys.iter().map(ToString::to_string).collect();
    assert_eq!(set.checked_keys(), expected.as_slice());
    if let Some((sort_target, expected_sort)) = sort {
        assert_eq!(
            gui_list_sort(&restored, sort_target),
            expected_sort,
            "the destination's saved sort restores"
        );
    }
}

/// A saved entry restores through the destination it names, not through the
/// destination that was active at exit (gui-collection-dropdown "The
/// collection selection persists per destination", scenario "Non-Music
/// selections survive a restart"): Music is the opened destination and holds
/// no saved entry, and the saved Feeds entry still installs through the Feeds
/// site.
#[test]
fn a_saved_entry_of_a_destination_that_was_not_active_restores() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.disconnect_emby_client_for_test();
    app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
    let destination = saved_music_identity();
    let state = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: destination.clone(),
        collections: vec![mbv_config::GuiDestinationCollections {
            destination: mbv_config::TabIdentity::Feeds,
            target: mbv_config::GuiLaunchCollectionTarget::Feeds,
            group_ids: vec![FEED_B.to_string()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };

    assert!(crate::gui_core::test_support::restore_launch_collections(
        &mut app,
        &state,
        &destination
    ));

    let set = restored_launch_selection(&app, &GuiCollectionTarget::Feeds);
    let expected = vec![FEED_B.to_string()];
    assert_eq!(
        set.checked_keys(),
        expected.as_slice(),
        "the saved Feeds entry installs although Music was the opened destination"
    );
}

/// A saved entry of a resting destination installs (gui-collection-dropdown
/// design D13): a Music library the session has not opened holds no group
/// list, so the restore must not wait for a catalog the resting tab never
/// loads — the saved groups install as saved, and the per-frame effective read
/// drops a group the landed catalog no longer holds.
#[test]
fn a_saved_resting_music_set_installs_without_a_group_list() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.seed_music_levels_for_test(&["group", "album"]);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = LIB_ID.into();
    library.collection_type = "music".into();
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    let destination = saved_music_identity();
    let state = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: destination.clone(),
        collections: vec![mbv_config::GuiDestinationCollections {
            destination: destination.clone(),
            target: mbv_config::GuiLaunchCollectionTarget::Music,
            group_ids: vec![SOUL.to_string()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };

    assert!(
        crate::gui_core::test_support::restore_launch_collections(&mut app, &state, &destination),
        "a resting destination's entry needs no retry"
    );

    let stored = crate::gui_core::test_support::stub_gui_state(|state| {
        state
            .library_groups
            .get(&0)
            .expect("the saved resting set installs with no group list to retain against")
            .set
            .clone()
    });
    assert_eq!(
        stored.checked_keys(),
        [SOUL.to_string()].as_slice(),
        "the saved group installs as saved"
    );
}

/// A tab switch between the opened destination and a saved entry whose
/// destination catalog can still land abandons the restore (design D9): the
/// Podcasts entry waits while the Audiobookshelf catalog is still connecting,
/// the user opens Podcasts, and the settled catalog installs no saved show
/// under the destination the user moved to.
#[test]
fn a_mid_restore_destination_switch_applies_no_saved_set() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.seed_gui_abs_libraries_for_test();
    app.seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A")]);
    app.seed_audiobookshelf_runtime_starting_for_test(true);
    let destination = saved_music_identity();
    let state = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: destination.clone(),
        collections: vec![mbv_config::GuiDestinationCollections {
            destination: mbv_config::TabIdentity::ServiceLibrary {
                kind: mbv_queue::ServiceKind::Audiobookshelf,
                library_id: "lib-abs-podcasts".into(),
            },
            target: mbv_config::GuiLaunchCollectionTarget::Podcasts,
            group_ids: vec!["show-a".to_string()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };

    assert!(
        !crate::gui_core::test_support::restore_launch_collections(&mut app, &state, &destination),
        "the Podcasts entry waits while its catalog can still land"
    );

    // The user opens Podcasts, and the Audiobookshelf catalog settles.
    app.seed_active_tab_for_test(TabSelection::AudiobookshelfLibrary(1));
    app.seed_audiobookshelf_runtime_starting_for_test(false);

    assert!(
        crate::gui_core::test_support::restore_launch_collections(&mut app, &state, &destination),
        "the switch abandons the restore"
    );
    assert!(
        !crate::gui_core::test_support::stub_gui_state(|state| state.podcast_libs.contains_key(&1)),
        "the saved show never installs under the destination the user moved to"
    );
}

/// A saved group the library no longer holds falls back to All
/// (gui-collection-dropdown "The collection selection persists per
/// destination", scenario "A saved collection no longer exists").
#[test]
fn a_saved_group_no_longer_held_restores_as_all() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    let destination = saved_music_identity();
    let state = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: destination.clone(),
        collections: vec![mbv_config::GuiDestinationCollections {
            destination: destination.clone(),
            target: mbv_config::GuiLaunchCollectionTarget::Music,
            group_ids: vec!["grp-gone".to_string()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };

    assert!(crate::gui_core::test_support::restore_launch_collections(
        &mut app,
        &state,
        &destination
    ));

    assert!(
        !crate::gui_core::test_support::stub_gui_state(|state| state
            .library_groups
            .contains_key(&0)),
        "the vanished group restores as All: no entry is stored"
    );
    assert!(app.try_recv_lib_event().is_none(), "All fetches nothing");
}

/// Contract (emby-favourites design D5): only Shows persists — a Favourites
/// choice on a Music library saves no view, so the field stays All.
#[test]
fn a_music_favourites_choice_saves_no_view() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.seed_music_levels_for_test(&["group", "album"]);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.push_library_for_test(mbv_ui_model::library_tab::LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    crate::gui_core::test_support::stub_gui_state(|state| {
        state.library_views.insert(0, GuiLibraryView::Favourites)
    });

    let saved =
        crate::gui_core::test_support::with_stub_gui(&mut app, |core| core.gui_launch_state());
    assert_eq!(saved.shows_view, mbv_config::GuiLaunchShowsView::All);
}

/// Contract (design D14): the Shows view saves in its own launch field and an
/// old file without the field loads as All.
#[test]
fn the_shows_view_survives_a_restart() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.push_library_for_test(mbv_ui_model::library_tab::LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    crate::gui_core::test_support::stub_gui_state(|state| {
        state.library_views.insert(0, GuiLibraryView::Upcoming)
    });

    let saved =
        crate::gui_core::test_support::with_stub_gui(&mut app, |core| core.gui_launch_state());
    assert_eq!(saved.shows_view, mbv_config::GuiLaunchShowsView::Upcoming);

    // A Favourites choice does not survive a restart: it saves as All
    // (emby-favourites design D5, spec "Favourites does not survive a
    // restart").
    crate::gui_core::test_support::stub_gui_state(|state| {
        state.library_views.insert(0, GuiLibraryView::Favourites)
    });
    let saved_favourites =
        crate::gui_core::test_support::with_stub_gui(&mut app, |core| core.gui_launch_state());
    assert_eq!(
        saved_favourites.shows_view,
        mbv_config::GuiLaunchShowsView::All
    );
    // The round trip below still saves and restores the Upcoming choice, so
    // the shared stub state carries it again.
    crate::gui_core::test_support::stub_gui_state(|state| {
        state.library_views.insert(0, GuiLibraryView::Upcoming)
    });
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");
    let mut restored = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    restored.push_library_for_test(mbv_ui_model::library_tab::LibraryTab::new(library));

    assert!(matches!(
        crate::gui_core::test_support::open_launch_destination(&mut restored, &loaded),
        GuiLaunchOpen::Opened(_)
    ));
    assert_eq!(
        crate::gui_core::test_support::stub_gui_state(|state| state.library_views.get(&0).copied()),
        Some(GuiLibraryView::Upcoming),
        "the restored launch installs the saved Shows view"
    );
    // The Client state belongs to one Client, so the fresh start begins with a
    // fresh one (task 8.1): the state the old file restores into carries no
    // view from the start before it.
    crate::gui_core::test_support::stub_gui_state(|state| {
        *state = crate::gui_core::GuiClientState::new(false);
    });

    let old = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: mbv_config::TabIdentity::ServiceLibrary {
            kind: mbv_queue::ServiceKind::Emby,
            library_id: "lib-tv".to_string(),
        },
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };
    // The Client state belongs to one Client, so the fresh start begins with a
    // fresh one (task 8.1): the state the old file restores into carries no
    // view from the start before it.
    crate::gui_core::test_support::stub_gui_state(|state| {
        *state = crate::gui_core::GuiClientState::new(false);
    });
    let mut fresh = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;
    fresh.push_library_for_test(mbv_ui_model::library_tab::LibraryTab::new(library));
    crate::gui_core::test_support::open_launch_destination(&mut fresh, &old);
    assert_eq!(
        crate::gui_core::test_support::stub_gui_state(|state| state.library_views.get(&0).copied()),
        Some(GuiLibraryView::All),
        "an old file with no view opens on All"
    );
}
