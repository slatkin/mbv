//! The GUI launch location (add-gui-library-panel design D9): the exit save
//! into the GUI's own state file, and the start restore of the destination,
//! the saved collection sets, and Panel focus. The state directory is a
//! `TestStateDirGuard` scratch path, and every Emby answer comes from the
//! in-memory mock transport, so no request leaves the process and no live
//! Owner, Service, or webview runs (design D13).

use mbv::{ClientTickHost, Config, stub_app_with_queue};
use mbv_config::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchState, LaunchPanelFocus,
    TabIdentity, TestStateDirGuard, gui_launch_state_path, load_gui_launch_state,
    save_gui_launch_state,
};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::ServiceKind;
use mbv_ui_model::tab_selection::TabSelection;

use crate::{TestCore, test_core_with};

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

/// The library id and name the seeded Music tab holds.
const MUSIC_LIBRARY: (&str, &str) = ("lib-music", "Music");

/// The id of a Shows library tab the GUI does not present (Phase 1 presents
/// the Grouped Music libraries only).
const SHOWS_LIBRARY: (&str, &str) = ("lib-shows", "Shows");

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
    let client = EmbyClient::new(Config {
        server_url: TEST_URL.into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    test.core.app_mut().seed_gui_music_library_for_test(client);
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

/// Seed the grouped Music library and one Shows library the GUI does not
/// present, and leave Home active.
fn seed_music_and_shows(app: &mut mbv::App, http: &MockHttp) {
    seed_music(app, http);
    app.push_gui_library_tab_for_test(EmbyItem::folder(
        SHOWS_LIBRARY.0.into(),
        SHOWS_LIBRARY.1.into(),
        "tvshows".into(),
    ));
    app.apply_gui_library_command(GuiLibraryCommand::OpenDestination { position: 0 }.into());
}

/// Send one Library command, as the `library` command's bridge translation
/// hands it to the core, and run one step.
fn send_library(test: &mut TestCore, command: GuiLibraryCommand) {
    test.tx.send(CoreInput::Library(command.into())).unwrap();
    assert!(test.core.step());
}

/// The exit save writes the GUI's own file and nothing writes it while the
/// window is open: Music with two selected groups and Library focus lands in
/// the location, and the file appears only at the run's exit boundary.
#[test]
fn the_window_close_saves_the_launch_location_once() {
    let _guard = TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, NO_ALBUMS);
    http.respond(200, NO_ALBUMS);
    let mut test = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SetCollections {
            lib_idx: 0,
            group_ids: vec!["grp-hiphop".into(), "grp-soul".into()],
        },
    );
    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    assert!(
        !gui_launch_state_path().exists(),
        "no write happens while the window is open"
    );

    test.tx.send(CoreInput::Close).unwrap();
    test.core.run();

    let state = load_gui_launch_state().expect("the exit saved the GUI launch location");
    assert_eq!(state.destination, library_identity(MUSIC_LIBRARY.0));
    assert_eq!(state.panel_focus, LaunchPanelFocus::Library);
    assert_eq!(
        state.collections,
        vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-hiphop".into(), "grp-soul".into()],
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
        panel_focus: LaunchPanelFocus::Library,
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-hiphop".into(), "grp-gone".into()],
        }],
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music);

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(0)
    );
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        ["grp-hiphop"]
    );
}

/// When every saved group is gone, the first group the library holds is
/// selected: a restored set never leaves the destination with no pill.
#[test]
fn a_restored_location_whose_groups_are_all_gone_selects_the_first_group() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MUSIC_LIBRARY.0),
        panel_focus: LaunchPanelFocus::Library,
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-gone".into(), "grp-also-gone".into()],
        }],
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_music);

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        ["grp-hiphop"]
    );
}

/// A location saved on Shows opens Music instead: the GUI presents only the
/// Grouped Music libraries, so a saved destination it does not present falls
/// back to the first presented one.
#[test]
fn a_location_saved_on_shows_opens_music() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(SHOWS_LIBRARY.0),
        panel_focus: LaunchPanelFocus::Queue,
        collections: Vec::new(),
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
        "the saved destination is not the first presented one"
    );
    assert_eq!(app.tab_for_test(), TabSelection::EmbyLibrary(0));
}
