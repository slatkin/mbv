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
            app.emby_runtime.client = None;
            app
        }
        GuiCollectionTarget::Podcasts(_) => {
            let mut app = make_app_stub();
            app.seed_gui_abs_libraries_for_test();
            app.seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A"), ("show-b", "Show B")]);
            app.tab = TabSelection::AudiobookshelfLibrary(1);
            app
        }
        GuiCollectionTarget::Feeds => {
            let mut app = make_app_stub();
            app.seed_gui_feeds_for_test(&[("Feed A", FEED_A), ("Feed B", FEED_B)]);
            app.tab = TabSelection::Feeds;
            app
        }
        GuiCollectionTarget::EmbyFeed(_) => {
            let mut app = make_app_stub();
            let mut config = app.config.lock().unwrap().clone();
            config.server_url = "http://127.0.0.1:1".into();
            install_test_emby(&mut app, config);
            app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
            app.emby_runtime.client = None;
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
            app.libs.push(tab);
            app.tab = TabSelection::EmbyLibrary(0);
            app
        }
    }
}

/// Seed one destination's session selection as the user's presses leave it.
fn seed_launch_selection(app: &mut App, target: &GuiCollectionTarget, keys: &[&str]) {
    let set = GuiCollectionSet::Checked(keys.iter().map(ToString::to_string).collect());
    let selection = GuiCollectionSelection { set };
    match target {
        GuiCollectionTarget::Music(lib_idx) => {
            app.gui.library_groups.insert(*lib_idx, selection);
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            app.gui.podcast_libs.entry(*lib_idx).or_default().selection = selection;
        }
        GuiCollectionTarget::Feeds => {
            app.gui.feeds_selection.selection = selection;
        }
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            app.gui.emby_feed_sets.insert(*lib_idx, selection);
        }
    }
}

/// The stored set one destination holds after the restore.
fn restored_launch_selection<'a>(
    app: &'a App,
    target: &GuiCollectionTarget,
) -> &'a GuiCollectionSet {
    match target {
        GuiCollectionTarget::Music(lib_idx) => {
            &app.gui
                .library_groups
                .get(lib_idx)
                .expect("the set was restored")
                .set
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            &app.gui
                .podcast_libs
                .get(lib_idx)
                .expect("the set was restored")
                .selection
                .set
        }
        GuiCollectionTarget::Feeds => &app.gui.feeds_selection.selection.set,
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            &app.gui
                .emby_feed_sets
                .get(lib_idx)
                .expect("the set was restored")
                .set
        }
    }
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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut source = launch_roundtrip_app(&target);
    seed_launch_selection(&mut source, &target, keys);
    if let Some(sort) = sort {
        seed_launch_sort(&mut source, sort);
    }
    let saved = crate::gui::GuiCore::new(source).gui_launch_state();
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");

    let mut restored = launch_roundtrip_app(&target);

    assert!(matches!(
        crate::app::tests::open_launch_destination(&mut restored, &loaded),
        GuiLaunchOpen::Opened(_)
    ));
    assert!(crate::app::tests::restore_launch_collections(
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
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.emby_runtime.client = None;
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

    assert!(crate::app::tests::restore_launch_collections(
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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.music_levels = vec!["group".to_string(), "album".to_string()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = LIB_ID.into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
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
        crate::app::tests::restore_launch_collections(&mut app, &state, &destination),
        "a resting destination's entry needs no retry"
    );

    let stored = app
        .gui
        .library_groups
        .get(&0)
        .expect("the saved resting set installs with no group list to retain against");
    assert_eq!(
        stored.set.checked_keys(),
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
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.seed_gui_abs_libraries_for_test();
    app.seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A")]);
    app.audiobookshelf_runtime =
        crate::app::state::service_runtime::AudiobookshelfRuntime::new(true);
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
        !crate::app::tests::restore_launch_collections(&mut app, &state, &destination),
        "the Podcasts entry waits while its catalog can still land"
    );

    // The user opens Podcasts, and the Audiobookshelf catalog settles.
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app.audiobookshelf_runtime =
        crate::app::state::service_runtime::AudiobookshelfRuntime::new(false);

    assert!(
        crate::app::tests::restore_launch_collections(&mut app, &state, &destination),
        "the switch abandons the restore"
    );
    assert!(
        !app.gui.podcast_libs.contains_key(&1),
        "the saved show never installs under the destination the user moved to"
    );
}

/// A saved group the library no longer holds falls back to All
/// (gui-collection-dropdown "The collection selection persists per
/// destination", scenario "A saved collection no longer exists").
#[test]
fn a_saved_group_no_longer_held_restores_as_all() {
    let _guard = crate::config::TestStateDirGuard::new();
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

    assert!(crate::app::tests::restore_launch_collections(
        &mut app,
        &state,
        &destination
    ));

    assert!(
        !app.gui.library_groups.contains_key(&0),
        "the vanished group restores as All: no entry is stored"
    );
    assert!(
        app.channels.lib_rx.try_recv().is_err(),
        "All fetches nothing"
    );
}

/// Contract (design D14): the Shows view saves in its own launch field and an
/// old file without the field loads as All.
#[test]
fn the_shows_view_survives_a_restart() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    app.gui.shows_views.insert(0, GuiShowsView::Upcoming);

    let saved = crate::gui::GuiCore::new(app).gui_launch_state();
    assert_eq!(saved.shows_view, mbv_config::GuiLaunchShowsView::Upcoming);
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");
    let mut restored = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    restored.libs.push(crate::app::LibraryTab::new(library));

    assert!(matches!(
        crate::app::tests::open_launch_destination(&mut restored, &loaded),
        GuiLaunchOpen::Opened(_)
    ));
    assert_eq!(gui_shows_view(&restored, 0), GuiShowsView::Upcoming);

    let old = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: loaded.destination,
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };
    let mut fresh = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    fresh.libs.push(crate::app::LibraryTab::new(library));
    crate::app::tests::open_launch_destination(&mut fresh, &old);
    assert_eq!(gui_shows_view(&fresh, 0), GuiShowsView::All);
}
