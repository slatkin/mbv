//! The GUI launch location (add-gui-library-panel design D9): the exit save
//! into the GUI's own state file, and the start restore of the destination and
//! the saved collection sets. The state directory is a `TestStateDirGuard`
//! scratch path, and every Emby answer comes from the in-memory mock transport,
//! so no request leaves the process and no live Owner, Service, or window runs
//! (design D13).

use mbv::{ClientTickHost, stub_app_with_queue};
use mbv_config::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchCollectionTarget, GuiLaunchState,
    TabIdentity, TestStateDirGuard, gui_launch_state_path, load_gui_launch_state,
    save_gui_launch_state,
};
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::ServiceKind;
use mbv_ui_model::tab_selection::TabSelection;

use crate::{TestCore, test_core_with, test_emby_client};

/// The library id and name the seeded Music tab holds.
const MUSIC_LIBRARY: (&str, &str) = ("lib-music", "Music");

/// The id of a Shows library tab the GUI presents as its own destination
/// (row 7.1).
const SHOWS_LIBRARY: (&str, &str) = ("lib-shows", "Shows");

/// The id of the Audiobookshelf podcast library the seeded catalog holds.
const PODCASTS_LIBRARY: &str = "lib-abs-podcasts";

/// The identity the saved location names for a library.
fn library_identity(library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind: ServiceKind::Emby,
        library_id: library_id.to_string(),
    }
}

/// One group's album listing, as the mock transport returns it: an empty page
/// ends the fetch, so the selection's fetch threads finish inside the test.
const NO_ALBUMS: &str = r#"{"Items":[],"TotalRecordCount":0}"#;

/// A core thread over an App with an empty queue, with the grouped Music
/// library seeded after the core exists: the launch Home load sees no Service,
/// so it starts no Emby request the test would have to script.
fn music_core(http: &MockHttp) -> TestCore {
    let mut test = test_core_with(stub_app_with_queue(0));
    test.core
        .app_mut()
        .seed_gui_music_library_for_test(test_emby_client(http));
    test
}

/// A core thread over an App that already holds the seeded grouped Music
/// library, so the saved location's restore runs at construction: the catalog
/// is present before the first step, and with no Service client the launch
/// Home load starts no Emby request and leaves the seeded tabs alone.
fn core_with_seeded_catalog(build: impl FnOnce(&mut mbv::App, &MockHttp)) -> TestCore {
    let http = MockHttp::new();
    let mut app = stub_app_with_queue(0);
    build(&mut app, &http);
    test_core_with(app)
}

/// Seed the grouped Music library without a Service client.
fn seed_music(app: &mut mbv::App, _http: &MockHttp) {
    app.seed_gui_music_library_tab_for_test();
}

/// Seed the grouped Music library and one Shows library the GUI presents, and
/// leave Home active.
fn seed_music_and_shows(app: &mut mbv::App, http: &MockHttp) {
    seed_music(app, http);
    app.push_gui_library_tab_for_test(EmbyItem::folder(
        SHOWS_LIBRARY.0.into(),
        SHOWS_LIBRARY.1.into(),
        "tvshows".into(),
    ));
    let mut core = mbv::GuiCore::new(std::mem::replace(app, mbv_gui::test_support::handoff_app()));
    core.apply_gui_library_command(GuiLibraryCommand::OpenDestination { position: 0 }.into());
    *app = core.into_app();
}

/// Seed the grouped Music library and the Audiobookshelf catalog, whose
/// podcast library holds no show yet, so the show list lands only after the
/// user opens the destination.
fn seed_music_and_podcasts(app: &mut mbv::App, http: &MockHttp) {
    seed_music(app, http);
    app.seed_gui_abs_libraries_for_test();
}

/// The tab position the presented Podcasts destination carries.
fn podcasts_position(test: &mut TestCore) -> usize {
    test.core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Podcasts")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Podcasts tab")
}

/// Send one Library command, as the `library` command's bridge translation
/// hands it to the core, and run one step.
fn send_library(test: &mut TestCore, command: GuiLibraryCommand) {
    test.tx.send(CoreInput::Library(command.into())).unwrap();
    assert!(test.core.step());
}

/// The exit save writes the GUI's own file and nothing writes it while the
/// window is open: Music with two selected groups lands in the location, and
/// the file appears only at the run's exit boundary.
#[test]
fn the_window_close_saves_the_launch_location_once() {
    let _guard = TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, NO_ALBUMS);
    let mut test = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::CollectionToggle {
            target: mbv::GuiCollectionTarget::Music(0),
            key: "grp-soul".into(),
        },
    );
    assert!(
        !gui_launch_state_path().exists(),
        "no write happens while the window is open"
    );

    test.tx.send(CoreInput::Close).unwrap();
    test.core.run();

    let state = load_gui_launch_state().expect("the exit saved the GUI launch location");
    assert_eq!(state.destination, library_identity(MUSIC_LIBRARY.0));
    assert_eq!(
        state.collections,
        vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            target: GuiLaunchCollectionTarget::Music,
            group_ids: vec!["grp-soul".into()],
            order: None,
            picks: Vec::new(),
        }]
    );
}

/// The start restore opens the saved destination with the saved groups the
/// library still holds, and drops the one it does not: the saved set names
/// Hip-Hop, Soul, and a group the library no longer has.
#[test]
fn a_restored_location_drops_a_group_the_library_no_longer_has() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MUSIC_LIBRARY.0),
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            target: GuiLaunchCollectionTarget::Music,
            group_ids: vec!["grp-hiphop".into(), "grp-gone".into()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(0)
    );
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        Some("grp-hiphop")
    );
}

/// When every saved group is gone, no choice is restored: the destination opens
/// on its first group (design D6).
#[test]
fn a_restored_location_whose_groups_are_all_gone_selects_the_first_group() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MUSIC_LIBRARY.0),
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            target: GuiLaunchCollectionTarget::Music,
            group_ids: vec!["grp-gone".into(), "grp-also-gone".into()],
            order: None,
            picks: Vec::new(),
        }],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music);

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        None,
        "a saved group the library lost leaves the default first group selected"
    );
}

/// The identity the saved location names for an Audiobookshelf library.
fn abs_identity(library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind: ServiceKind::Audiobookshelf,
        library_id: library_id.to_string(),
    }
}

/// Every start opens on Home, whatever destination the saved location holds;
/// the sticky part of the location is each destination's own selection, which
/// `a_saved_resting_destination_restores_before_the_user_opens_it` owns.
#[test]
fn every_start_opens_on_home_whatever_destination_was_saved() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(SHOWS_LIBRARY.0),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_shows);

    assert_eq!(test.core.app_mut().tab_for_test(), TabSelection::Home);
}

/// A saved entry of a resting destination installs before the user opens it
/// (design D9, D13): the destination phase 1 opened restores its own saved
/// entry, and the destination the user has not opened yet restores its own
/// too, although its show list has not landed — the resting tab never loads
/// that list, so the restore must not wait for it. The list lands when the
/// user opens the destination, and the saved show is still the one checked.
#[test]
fn a_saved_resting_destination_restores_before_the_user_opens_it() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MUSIC_LIBRARY.0),
        collections: vec![
            GuiDestinationCollections {
                destination: library_identity(MUSIC_LIBRARY.0),
                target: GuiLaunchCollectionTarget::Music,
                group_ids: vec!["grp-hiphop".into()],
                order: None,
                picks: Vec::new(),
            },
            GuiDestinationCollections {
                destination: abs_identity(PODCASTS_LIBRARY),
                target: GuiLaunchCollectionTarget::Podcasts,
                group_ids: vec!["show-a".into()],
                order: Some(mbv_config::GuiLaunchDateOrder::Oldest),
                picks: Vec::new(),
            },
        ],
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_podcasts);

    // Phase 1 opened Music, whose landed group list takes its own saved entry;
    // the Podcasts entry installs although no show list has landed.
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        Some("grp-hiphop"),
        "the opened destination restores its own saved entry"
    );
    assert_eq!(
        test.core
            .gui_list_sort_for_test(mbv::GuiSortTarget::Podcasts(1)),
        mbv::GuiListSort::Oldest,
        "the resting destination installs its saved order as its sort with no show list"
    );

    // The user opens Podcasts, whose show list has landed by then.
    test.core
        .app_mut()
        .seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A")]);
    let position = podcasts_position(&mut test);
    send_library(&mut test, GuiLibraryCommand::OpenDestination { position });
    test.tx
        .send(crate::measured_layout(10, Vec::new()))
        .unwrap();
    assert!(test.core.step());

    let frame = test.held();
    let collections = frame
        .library_list()
        .and_then(|list| list.destination.collections())
        .expect("the Podcasts destination paints its collection dropdown");
    let listed: Vec<&str> = collections
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(listed, ["Show A"], "the show list landed on the open");
    assert!(
        !collections.all,
        "the saved show stays checked against the landed list"
    );
    assert!(
        collections.entries[0].checked,
        "the saved show is the one checked entry"
    );
}
