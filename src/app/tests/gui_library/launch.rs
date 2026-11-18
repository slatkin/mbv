//! The GUI launch file's collection selection tests: one save and restart
//! round trip per collection dropdown, and the stale-id fallback to All.

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
fn seed_launch_selection(
    app: &mut App,
    target: &GuiCollectionTarget,
    keys: &[&str],
    order: Option<GuiDateOrder>,
) {
    let set = GuiCollectionSet::Checked(keys.iter().map(ToString::to_string).collect());
    let stored_order = order.unwrap_or_default();
    match target {
        GuiCollectionTarget::Music(lib_idx) => {
            app.gui_library_groups.insert(
                *lib_idx,
                GuiCollectionSelection {
                    set,
                    order: stored_order,
                },
            );
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            let stored = app.gui_podcast_libs.entry(*lib_idx).or_default();
            stored.selection = GuiCollectionSelection {
                set,
                order: stored_order,
            };
        }
        GuiCollectionTarget::Feeds => {
            app.gui_feeds_selection.selection = GuiCollectionSelection {
                set,
                order: stored_order,
            };
        }
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            app.gui_emby_feed_sets.insert(
                *lib_idx,
                GuiCollectionSelection {
                    set,
                    order: stored_order,
                },
            );
        }
    }
}

/// The stored set and date order one destination holds after the restore.
fn restored_launch_selection<'a>(
    app: &'a App,
    target: &GuiCollectionTarget,
) -> (&'a GuiCollectionSet, Option<GuiDateOrder>) {
    match target {
        GuiCollectionTarget::Music(lib_idx) => {
            let stored = app
                .gui_library_groups
                .get(lib_idx)
                .expect("the set was restored");
            (&stored.set, None)
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            let stored = app
                .gui_podcast_libs
                .get(lib_idx)
                .expect("the set was restored");
            (&stored.selection.set, Some(stored.selection.order))
        }
        GuiCollectionTarget::Feeds => (
            &app.gui_feeds_selection.selection.set,
            Some(app.gui_feeds_selection.selection.order),
        ),
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            let stored = app
                .gui_emby_feed_sets
                .get(lib_idx)
                .expect("the set was restored");
            (&stored.set, Some(stored.order))
        }
    }
}

/// Every collection dropdown's checked set, in saved order, and its date
/// order where the destination stores one, survive a save and a restart
/// (bug fix: only Music used to persist).
#[rstest]
#[case::music(GuiCollectionTarget::Music(0), &[SOUL, HIPHOP], None)]
#[case::podcasts(
    GuiCollectionTarget::Podcasts(1),
    &["show-a", "show-b"],
    Some(GuiDateOrder::Oldest)
)]
#[case::feeds(
    GuiCollectionTarget::Feeds,
    &[FEED_B, FEED_A],
    Some(GuiDateOrder::Oldest)
)]
#[case::emby_feed(
    GuiCollectionTarget::EmbyFeed(0),
    &["folder-b", "folder-a"],
    Some(GuiDateOrder::Oldest)
)]
fn a_saved_checked_set_restores_as_saved(
    #[case] target: GuiCollectionTarget,
    #[case] keys: &[&str],
    #[case] order: Option<GuiDateOrder>,
) {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut source = launch_roundtrip_app(&target);
    seed_launch_selection(&mut source, &target, keys, order);
    let saved = source.gui_launch_state();
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");

    let mut restored = launch_roundtrip_app(&target);

    assert!(restored.restore_gui_launch_collections(&loaded, &loaded.destination));
    let (set, restored_order) = restored_launch_selection(&restored, &target);
    let expected: Vec<String> = keys.iter().map(ToString::to_string).collect();
    assert_eq!(set.checked_keys(), expected.as_slice());
    assert_eq!(restored_order, order);
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
    };

    assert!(app.restore_gui_launch_collections(&state, &destination));

    assert!(
        !app.gui_library_groups.contains_key(&0),
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
    app.gui_shows_views.insert(0, GuiShowsView::Upcoming);

    let saved = app.gui_launch_state();
    assert_eq!(saved.shows_view, mbv_config::GuiLaunchShowsView::Upcoming);
    mbv_config::save_gui_launch_state(&saved).expect("the snapshot is written");
    let loaded = mbv_config::load_gui_launch_state().expect("the snapshot is read");
    let mut restored = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    restored.libs.push(crate::app::LibraryTab::new(library));

    assert!(matches!(
        restored.open_gui_launch_destination(&loaded),
        GuiLaunchOpen::Opened(_)
    ));
    assert_eq!(restored.gui_shows_view(0), GuiShowsView::Upcoming);

    let old = mbv_config::GuiLaunchState {
        version: mbv_config::GUI_LAUNCH_STATE_VERSION,
        destination: loaded.destination,
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    };
    let mut fresh = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    fresh.libs.push(crate::app::LibraryTab::new(library));
    fresh.open_gui_launch_destination(&old);
    assert_eq!(fresh.gui_shows_view(0), GuiShowsView::All);
}
