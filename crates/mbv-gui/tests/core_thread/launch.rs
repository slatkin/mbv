//! The GUI launch location (add-gui-library-panel design D9): the exit save
//! into the GUI's own state file, and the start restore of the destination,
//! the saved collection sets, and Panel focus. The state directory is a
//! `TestStateDirGuard` scratch path, and every Emby answer comes from the
//! in-memory mock transport, so no request leaves the process and no live
//! Owner, Service, or webview runs (design D13).

use mbv::{ClientTickHost, stub_app_with_queue};
use mbv_config::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchState, LaunchPanelFocus,
    TabIdentity, TestStateDirGuard, gui_launch_state_path, load_gui_launch_state,
    save_gui_launch_state,
};
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::frame::focus::GuiPanel;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::ServiceKind;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::tab_selection::TabSelection;

use crate::{TestCore, test_core_with, test_emby_client};

/// The library id and name the seeded Music tab holds.
const MUSIC_LIBRARY: (&str, &str) = ("lib-music", "Music");

/// The id of a Shows library tab the GUI does not present (Phase 1 presents
/// the Grouped Music libraries only).
const SHOWS_LIBRARY: (&str, &str) = ("lib-shows", "Shows");

/// The id and name of a second Grouped Music library the GUI presents.
const SECOND_MUSIC: (&str, &str) = ("lib-music-2", "Music 2");

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

/// Seed two grouped Music libraries: the first holds its group list, and the
/// second's pushed tab has none yet, so the Collections phase waits for it.
fn seed_two_music(app: &mut mbv::App, http: &MockHttp) {
    seed_music(app, http);
    app.push_gui_library_tab_for_test(EmbyItem::folder(
        SECOND_MUSIC.0.into(),
        SECOND_MUSIC.1.into(),
        "music".into(),
    ));
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
    let mut test = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-soul".into(),
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
            group_ids: vec!["grp-soul".into()],
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
        panel_focus: LaunchPanelFocus::Library,
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-hiphop".into(), "grp-gone".into()],
            picks: Vec::new(),
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
        panel_focus: LaunchPanelFocus::Library,
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-gone".into(), "grp-also-gone".into()],
            picks: Vec::new(),
        }],
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

/// A location saved on Shows opens Shows (add-gui-library-panel design D9,
/// row 7.1): the GUI presents the Shows destinations now, so the matching
/// presented library opens — the saved-destination fallback stays only for a
/// destination the GUI still does not present (row 13.1 owns its deletion).
#[test]
fn a_location_saved_on_shows_opens_shows() {
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
        "the saved destination is not the catalog's first library"
    );
    assert_eq!(
        app.tab_for_test(),
        TabSelection::EmbyLibrary(shows_index),
        "the saved Shows destination restored"
    );
}

/// A tab switch between the catalog landing and a group list's landing never
/// applies another destination's saved set (design D9): the Collections phase
/// records the destination phase 1 opened, applies only its saved entry, and
/// gives up when a different destination is active.
#[test]
fn a_mid_restore_destination_switch_applies_no_saved_pills() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(SECOND_MUSIC.0),
        panel_focus: LaunchPanelFocus::Queue,
        collections: vec![
            GuiDestinationCollections {
                destination: library_identity(MUSIC_LIBRARY.0),
                group_ids: vec!["grp-hiphop".into()],
                picks: Vec::new(),
            },
            GuiDestinationCollections {
                destination: library_identity(SECOND_MUSIC.0),
                group_ids: vec!["grp-jazz".into()],
                picks: Vec::new(),
            },
        ],
    })
    .unwrap();

    let mut test = core_with_seeded_catalog(seed_two_music);

    // Phase 1 opened the saved destination, whose group list has not landed,
    // so the restore waits in the Collections phase.
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(1)
    );

    // The user switches to the first Music library while the group list is
    // still pending.
    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 1 },
    );
    assert!(test.core.step());

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        None,
        "the switched-to destination keeps no saved pills"
    );
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(1),
        None,
        "the opened destination's saved set is not applied after the switch"
    );
}

/// A take-focus in the pre-catalog window survives the restore (design D9):
/// phase 1 still opens the saved destination, but it skips the saved
/// Panel-focus re-apply once the user has moved focus.
#[test]
fn a_pre_catalog_take_focus_survives_the_restore() {
    let _guard = TestStateDirGuard::new();
    save_gui_launch_state(&GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: library_identity(MUSIC_LIBRARY.0),
        panel_focus: LaunchPanelFocus::Queue,
        collections: vec![GuiDestinationCollections {
            destination: library_identity(MUSIC_LIBRARY.0),
            group_ids: vec!["grp-hiphop".into()],
            picks: Vec::new(),
        }],
    })
    .unwrap();

    // The catalog is absent at construction, so the window is interactive
    // while the restore waits in the Destination phase.
    let mut test = test_core_with(stub_app_with_queue(0));

    // The user takes Panel focus for the Library panel before the catalog
    // lands, and the frame carries it.
    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    assert_eq!(test.core.app_mut().gui_panel_focus(), PanelFocus::Library);
    assert_eq!(test.next_frame().focus.panel, GuiPanel::Library);

    // The catalog lands: the destination jump still runs, and the saved
    // Queue focus does not replace the taken Library focus.
    test.core.app_mut().seed_gui_music_library_tab_for_test();
    assert!(test.core.step());

    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(0)
    );
    assert_eq!(test.core.app_mut().gui_panel_focus(), PanelFocus::Library);
    // The saved collection set still applies: only the Panel-focus re-apply
    // is conditional. The wake's step emits the frame the restore's state
    // landed in.
    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        Some("grp-hiphop")
    );
    test.tx.send(CoreInput::Wake).unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_frame().focus.panel, GuiPanel::Library);
}
