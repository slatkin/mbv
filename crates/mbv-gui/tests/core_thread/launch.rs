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
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::tab_selection::TabSelection;

use crate::{TestCore, test_core_with, test_emby_client};

/// The library id and name the seeded Music tab holds.
const MUSIC_LIBRARY: (&str, &str) = ("lib-music", "Music");

/// The id of a Shows library tab the GUI presents as its own destination
/// (row 7.1).
const SHOWS_LIBRARY: (&str, &str) = ("lib-shows", "Shows");

/// The id and name of a plain Emby (Movies) library the GUI presents (row
/// 8.1).
const MOVIES_LIBRARY: (&str, &str) = ("lib-movies", "Movies");

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
    app.apply_gui_library_command(GuiLibraryCommand::OpenDestination { position: 0 }.into());
}

/// Seed the grouped Music library and the Audiobookshelf catalog, whose
/// podcast library holds no show yet, so a saved Podcasts entry waits for a
/// show list that has not landed.
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

/// Seed the grouped Music library and one plain Emby (Movies) library the
/// GUI presents (row 8.1), and leave Home active.
fn seed_music_and_movies(app: &mut mbv::App, http: &MockHttp) {
    seed_music(app, http);
    app.push_gui_library_tab_for_test(EmbyItem::folder(
        MOVIES_LIBRARY.0.into(),
        MOVIES_LIBRARY.1.into(),
        "movies".into(),
    ));
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
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music);

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        None,
        "a saved group the library lost leaves the default first group selected"
    );
}

/// A start with no saved location opens the first presented destination with
/// Library Panel focus. No choice is saved, so the destination shows its first
/// group, which one group always selects (design D6). Regression for the live
/// finding that a fresh launch left the Library panel unreachable.
#[test]
fn a_fresh_launch_opens_the_first_presented_destination() {
    let _guard = TestStateDirGuard::new();

    let mut test = core_with_seeded_catalog(seed_music_and_shows);

    let app = test.core.app_mut();
    assert_eq!(app.tab_for_test(), TabSelection::EmbyLibrary(0));
    assert_eq!(app.gui_panel_focus(), PanelFocus::Library);
    assert_eq!(
        app.gui_library_groups_for_test(0),
        None,
        "a fresh launch opens on the first group"
    );
}

/// A location saved on Shows opens Shows and leaves it the active destination
/// (add-gui-library-panel design D9, row 7.1, row 13.1): the GUI presents the
/// Shows destinations now, so the matching presented library opens and no
/// saved-destination fallback exists to take another destination.
#[test]
fn a_location_saved_on_shows_opens_shows() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(SHOWS_LIBRARY.0),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_shows);

    let app = test.core.app_mut();
    let shows_index = app
        .libs_for_test()
        .iter()
        .position(|tab| tab.library.id == SHOWS_LIBRARY.0)
        .expect("the catalog holds the Shows library");
    assert_ne!(
        shows_index, 0,
        "the saved destination is not the catalog's first library"
    );
    assert_eq!(
        app.tab_for_test(),
        TabSelection::EmbyLibrary(shows_index),
        "the saved Shows destination restored"
    );
    assert_eq!(
        app.gui_active_destination(),
        Some(mbv::GuiDestinationKind::Shows {
            lib_idx: shows_index
        }),
        "the restored destination is the active one"
    );
}

/// A saved destination the settled catalog no longer holds opens nothing
/// (add-gui-library-panel design D9, row 13.1): the presented set holds the
/// Music and Shows libraries, so the saved identity matches none of them, the
/// deleted fallback would have opened Music, and the restore finishes instead
/// of waiting for a destination that never lands.
#[test]
fn a_saved_destination_the_catalog_lost_opens_nothing() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity("lib-gone"),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_shows);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::Home,
        "the saved destination opens no destination the location did not name"
    );
}

/// A destination that lands after the restore finished never moves the tab
/// (add-gui-library-panel design D9, row 13.1): the presented set settles
/// without the saved identity, so the restore is done, and the library the
/// catalog publishes later does not become the active destination under the
/// user.
#[test]
fn a_destination_presented_after_the_restore_finished_does_not_move_the_tab() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity("lib-gone"),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_shows);
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::Home,
        "the vanished identity opens nothing"
    );

    // The library the saved location named arrives after the restore finished.
    test.core
        .app_mut()
        .push_gui_library_tab_for_test(EmbyItem::folder(
            "lib-gone".into(),
            "Gone".into(),
            "music".into(),
        ));
    assert!(test.core.step());

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::Home,
        "a finished restore never moves the tab to a late destination"
    );
}

/// A saved Audiobookshelf destination waits while its catalog can still land
/// (add-gui-library-panel design D9, row 9.1, row 13.1): the Audiobookshelf
/// libraries are published and the startup is still running, so the restore
/// does not open the destination yet.
#[test]
fn a_saved_audiobooks_destination_waits_while_the_catalog_is_pending() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: abs_identity("lib-abs-books"),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(|app, _http| {
        app.seed_gui_abs_libraries_for_test();
        app.mark_gui_abs_catalog_pending_for_test();
    });

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::Home,
        "a pending catalog is not a catalog the destination is gone from"
    );
}

/// A location saved on a plain Emby (Movies) library opens it
/// (add-gui-library-panel row 8.1): the plain Emby-library destinations join
/// the presented set, so the matching presented library restores through the
/// same two-phase path.
#[test]
fn a_location_saved_on_a_movies_library_opens_it() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MOVIES_LIBRARY.0),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_movies);

    let app = test.core.app_mut();
    let movies_index = app
        .libs_for_test()
        .iter()
        .position(|tab| tab.library.id == MOVIES_LIBRARY.0)
        .expect("the catalog holds the Movies library");
    assert_ne!(
        movies_index, 0,
        "the saved destination is not the catalog's first library"
    );
    assert_eq!(
        app.tab_for_test(),
        TabSelection::EmbyLibrary(movies_index),
        "the saved Movies destination restored"
    );
}

/// The identity the saved location names for an Audiobookshelf library.
fn abs_identity(library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind: ServiceKind::Audiobookshelf,
        library_id: library_id.to_string(),
    }
}

/// Seed one Audiobookshelf book library the GUI presents, with no Emby
/// library at all: the Audiobooks destination is the presented one
/// (add-gui-library-panel row 9.1; row 10.1 presents Podcasts beside it).
fn seed_abs(app: &mut mbv::App, _http: &MockHttp) {
    app.seed_gui_abs_libraries_for_test();
}

/// A location saved on an Audiobookshelf book library opens it
/// (add-gui-library-panel design D9, row 9.1): the Audiobooks destinations
/// join the presented set, so the matching presented library restores through
/// the same first phase, whose resolution waits for the Audiobookshelf
/// catalog to settle instead of giving up while it can still land.
#[test]
fn a_location_saved_on_audiobooks_opens_it() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: abs_identity("lib-abs-books"),
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_abs);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::AudiobookshelfLibrary(0),
        "the saved Audiobooks destination restored"
    );
}

/// A tab switch between the catalog landing and a saved entry's show list
/// landing abandons the restore (design D9, D13): the destination phase 1
/// opened restores its own saved entry, and the destination the user switched
/// to never receives the saved set whose catalog landed after the switch.
#[test]
fn a_mid_restore_destination_switch_applies_no_saved_pills() {
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
                order: None,
                picks: Vec::new(),
            },
        ],
        shows_view: mbv_config::GuiLaunchShowsView::All,
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music_and_podcasts);

    // Phase 1 opened Music, whose landed group list takes its own saved entry;
    // the Podcasts entry waits for a show list that has not landed.
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        Some("grp-hiphop"),
        "the opened destination restores its own saved entry"
    );

    // The user switches to Podcasts while the restore still waits, and the
    // show list lands after the switch.
    let position = podcasts_position(&mut test);
    send_library(&mut test, GuiLibraryCommand::OpenDestination { position });
    test.core
        .app_mut()
        .seed_gui_podcast_shows_for_test(1, &[("show-a", "Show A")]);
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
    assert_eq!(listed, ["Show A"], "the show list landed after the switch");
    assert!(
        collections.all,
        "the switch abandons the restore: the saved show never installs"
    );
}
