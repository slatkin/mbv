//! The GUI Library's Shows data path through the GUI tick (add-gui-library-
//! panel design D12, row 7.1, contract a): the App actions the later core
//! surface's commands will apply seed the selection and start the mode's
//! root load, and the tick's library drain applies the fetch completion
//! through the App arms — the same arms the presenting Client's shell drain
//! joins. The Emby answers come from the in-memory mock transport, so no
//! request leaves the process, and no live Owner, Service, or webview runs
//! (design D13).

use mbv::ClientTickHost;
use mbv_net::mock_http::MockHttp;

use super::{TestCore, send_library};
use crate::{test_core_with, test_emby_client};

/// A Shows library's series listing, as the mode load's ranged fetch
/// returns it.
const SERIES: &str = r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#;

/// A series' season listing, as the detail fetch returns it.
const SEASONS_A: &str = r#"{"Items":[{"Id":"s1-a","Name":"Season 1","Type":"Season"},{"Id":"s2-a","Name":"Season 2","Type":"Season"}],"TotalRecordCount":2}"#;
const SEASONS_B: &str = r#"{"Items":[{"Id":"s1-b","Name":"Season 1","Type":"Season"},{"Id":"s2-b","Name":"Season 2","Type":"Season"}],"TotalRecordCount":2}"#;

/// A season's episode listing, as the season fetch returns it.
const SEASON_1_A: &str = r#"{"Items":[{"Id":"ep-a1","Name":"Pilot","Type":"Episode","IndexNumber":1}],"TotalRecordCount":1}"#;
const SEASON_1_B: &str = r#"{"Items":[{"Id":"ep-b1","Name":"Opening","Type":"Episode","IndexNumber":1}],"TotalRecordCount":1}"#;
const SEASON_2_B: &str = r#"{"Items":[{"Id":"ep-b2","Name":"Follow-up","Type":"Episode","IndexNumber":1}],"TotalRecordCount":1}"#;

/// The empty answers the open's eager Upcoming snapshot and the resolved
/// default mode's Latest load return, so the later All series load's
/// response is the one the mode's rows read.
const NO_PREMIERES: &str = r#"{"Items":[],"TotalRecordCount":0}"#;

/// A core thread over an App whose Emby runtime holds a client over `http`,
/// with a Shows library seeded as the active destination: an empty root
/// level with no resolved TV mode, as a first open finds it. The seeding
/// lands after the core exists, so the construction's launch Home load saw
/// no Service and scripts no mock answer (design D13).
fn shows_core(http: &MockHttp) -> TestCore {
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    test.core
        .app_mut()
        .seed_gui_tv_library_for_test(test_emby_client(http));
    test
}

/// Pump GUI ticks until the root level holds rows (the fetch worker's
/// completion is asynchronous over the in-memory mock), bounded so a broken
/// fetch fails the assertion instead of hanging the test.
fn pump_until_root_rows(test: &mut TestCore) {
    for _ in 0..250 {
        if test
            .core
            .app_mut()
            .tv_root_item_ids_for_test(0)
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
        {
            return;
        }
        assert!(test.core.step(), "the core thread never quits mid-drain");
    }
    panic!("the All series mode's root load never landed on the App");
}

/// The Shows section a frame carries, present only while a Shows
/// destination presents.
fn shows_section(
    frame: &mbv_gui::bridge::frame::GuiFrame,
) -> Option<&mbv_gui::bridge::frame::library::GuiShowsFrame> {
    frame.library.as_ref()?.content.as_ref()?.shows.as_ref()
}

/// Pump GUI ticks until the Shows section a frame carries satisfies `want`,
/// bounded so a broken fetch fails the assertion instead of hanging the
/// test. Each step's frames drain through `latest_frame`, so the predicate
/// reads the newest emitted state, and each miss waits briefly on the mock
/// for one more transmitted request — the sanctioned seam for the fetch
/// workers' own progress, which the fetch-driven loads (a detail landing's
/// own first-season fetch) issue between steps.
fn pump_until_shows(
    test: &mut TestCore,
    http: &MockHttp,
    want: impl Fn(&mbv_gui::bridge::frame::library::GuiShowsFrame) -> bool,
    what: &str,
) {
    for _ in 0..250 {
        assert!(test.core.step(), "the core thread never quits mid-drain");
        if let Some(shows) = test.latest_frame().as_ref().and_then(shows_section)
            && want(shows)
        {
            return;
        }
        let _ = http.wait_for_requests(
            http.request_count() + 1,
            std::time::Duration::from_millis(50),
        );
    }
    panic!("the tick never produced {what}");
}

/// A core thread over an App whose catalog holds a grouped Music library
/// (whose store is already loaded, so its frame fetches nothing) and the
/// Shows library, with the Emby client over `http`. The seeding lands after
/// the core exists, so the construction's launch Home load saw no Service
/// and scripts no mock answer (design D13); the first step lets the launch
/// restore's first-presented open complete on the Music destination, so the
/// test's own open-destination command is the only TV activation the mock
/// answers.
fn shows_command_core(http: &MockHttp) -> TestCore {
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(http));
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");
    core.seed_gui_music_albums_for_test(0, "grp-hiphop", &[("alb-ill", "Licensed to Ill")]);
    core.push_gui_library_tab_for_test(mbv_emby_model::EmbyItem::folder(
        "lib-tv".to_string(),
        "Shows".to_string(),
        "tvshows".to_string(),
    ));
    core.pin_gui_library_total_for_test(1, 0);
    // The launch restore's first-presented open (design D9): the seeded
    // catalog's first presented destination is the Music library, whose
    // activation loads nothing.
    assert!(test.core.step());
    test
}

/// Opening the Shows library in the All series mode lands the series root
/// rows on the App through the GUI tick (row 7.1, contract a): the open and
/// set-mode actions run the TUI's own mode load, and the tick's
/// `drain_lib_event` applies the refresh completion's App arm. The mock's
/// request wait is the sanctioned seam for the fetch worker's own progress,
/// so the step pump never starves it.
#[test]
fn opening_shows_in_all_mode_lands_the_series_root_rows_on_the_tick() {
    let http = MockHttp::new();
    http.respond(200, SERIES);
    let mut test = shows_core(&http);

    let app = test.core.app_mut();
    app.gui_tv_open(0);
    app.gui_tv_set_mode(0, &mbv_queue::TvContentMode::All);
    assert!(
        http.wait_for_requests(1, std::time::Duration::from_secs(2)),
        "the mode load's fetch was never sent"
    );
    pump_until_root_rows(&mut test);

    assert_eq!(
        test.core.app_mut().tv_root_item_ids_for_test(0),
        Some(vec!["series-a".to_string(), "series-b".to_string()]),
        "the series root rows land on the App the GUI tick drains"
    );
}

/// The mocked bootstrap's catalog presents the Shows destination, and the
/// open that follows it (the restore's own first-presented open, or the
/// open-destination command) finds the Shows frame (row 7.1, design D12):
/// the three view-mode pills with the resolved default selected, and — the
/// default mode's root load still under way or failed against the
/// no-URL stub client — no rows and no Hero yet.
#[test]
fn an_open_shows_destination_after_the_bootstrap_finds_the_shows_frame() {
    let view = mbv_emby_model::EmbyItem::folder(
        "lib-tv".to_string(),
        "Shows".to_string(),
        "tvshows".to_string(),
    );
    let mut test = test_core_with(mbv::stub_app_with_completed_emby_startup(vec![view]));

    // The mocked bootstrap drains on the first tick; the catalog's one
    // presented destination is the Shows library, and the launch restore's
    // first-presented open activates it.
    assert!(test.core.step());
    let frame = test.latest_frame().expect("the bootstrap's frame emitted");
    let shows = shows_section(&frame).expect("the open Shows destination presents its content");

    assert_eq!(
        shows.lib_idx, 0,
        "the frame names the library the commands carry"
    );
    let names: Vec<&str> = shows.pills.iter().map(|pill| pill.name.as_str()).collect();
    assert_eq!(names, ["Latest", "Upcoming", "All series"]);
    let selected: Vec<&str> = shows
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(
        selected,
        ["Latest"],
        "one mode pill is selected, the resolved default"
    );
    assert!(
        !shows.pills.iter().any(|pill| pill.collection),
        "the mode pills are single-select"
    );
    assert!(
        shows.rows.is_empty() && shows.hero.is_none(),
        "the default mode's root load has not landed rows yet"
    );
}

/// Send the Shows destination's open command, as the destination tabs'
/// `openDestination` carries it, and assert the frame it builds names the
/// Shows library.
fn open_shows_destination(test: &mut TestCore, position: usize, lib_idx: usize) {
    send_library(
        test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );
    let frame = test.latest_frame().expect("the open's frame emitted");
    let shows = shows_section(&frame).expect("the open's frame carries the Shows content");
    assert_eq!(
        shows.lib_idx, lib_idx,
        "the frame names the library the commands carry"
    );
}

/// Pump until the All series mode selects `series` and the Hero's title is
/// `title`.
fn pump_until_series_hero(test: &mut TestCore, http: &MockHttp, series: &str, title: &str) {
    pump_until_shows(
        test,
        http,
        |shows| {
            shows.rows.len() == 2
                && shows.selected.as_deref() == Some(series)
                && shows.hero.as_ref().is_some_and(|hero| hero.title == title)
        },
        "the All series mode's rows and Hero",
    );
}

/// Pump until the Workspace shows `episode_id` under `season_id`.
fn pump_until_workspace(
    test: &mut TestCore,
    http: &MockHttp,
    season_id: &str,
    episode_id: &str,
    what: &str,
) {
    pump_until_shows(
        test,
        http,
        |shows| {
            shows
                .hero
                .as_ref()
                .and_then(|hero| hero.workspace.as_ref())
                .is_some_and(|workspace| {
                    workspace.selected_season.as_deref() == Some(season_id)
                        && workspace
                            .episodes
                            .iter()
                            .any(|row| row.episode_id == episode_id)
                })
        },
        what,
    );
}

/// A series select moves the Hero and a season select moves the Workspace
/// (row 7.1, design D12): the open-destination command activates the Shows
/// library (the tab settlement's default root load and the eager Upcoming
/// snapshot ride the mock), the set-mode command loads the series list and
/// the frame path's detail ensure loads the default series' Workspace, a
/// select command names the row the frame painted and the Hero follows it
/// with its own Workspace, and a season command moves the Workspace to that
/// season's episodes. Each `wait_for_requests` is the sanctioned seam for
/// the fetch workers' own progress (the pinned total keeps the root load's
/// completion from spawning a default-mode reload behind the script).
#[test]
fn a_series_select_moves_the_hero_and_a_season_select_moves_the_workspace() {
    let http = MockHttp::new();
    // In request order: the open's default root load, the eager Upcoming
    // snapshot, the All series load, series-a's detail (the frame path's
    // ensure) and its first-season fetch, then series-b's detail (the
    // select) with its first-season fetch, then the selected season's
    // episodes.
    http.respond(200, SERIES);
    http.respond(200, NO_PREMIERES);
    http.respond(200, SERIES);
    http.respond(200, SEASONS_A);
    http.respond(200, SEASON_1_A);
    http.respond(200, SEASONS_B);
    http.respond(200, SEASON_1_B);
    http.respond(200, SEASON_2_B);
    let mut test = shows_command_core(&http);

    open_shows_destination(&mut test, 2, 1);
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSetMode {
            lib_idx: 1,
            mode: mbv_gui::bridge::library::GuiShowsMode::All,
        },
    );
    pump_until_series_hero(&mut test, &http, "series-a", "Alfred");
    pump_until_workspace(
        &mut test,
        &http,
        "s1-a",
        "ep-a1",
        "the default series' Workspace",
    );

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSelect {
            lib_idx: 1,
            row: "series-b".into(),
        },
    );
    let frame = test.latest_frame().expect("the select's frame emitted");
    let shows = shows_section(&frame).expect("the frame carries the Shows content");
    assert_eq!(
        shows.selected.as_deref(),
        Some("series-b"),
        "the frame paints the core's selection"
    );
    assert_eq!(
        shows.hero.as_ref().expect("the Hero moved").title,
        "Bridgerton",
        "the Hero's facts follow the selected series"
    );
    pump_until_workspace(
        &mut test,
        &http,
        "s1-b",
        "ep-b1",
        "the selected series' Workspace",
    );

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSelectSeason {
            lib_idx: 1,
            season_id: "s2-b".into(),
        },
    );
    pump_until_workspace(
        &mut test,
        &http,
        "s2-b",
        "ep-b2",
        "the season command's Workspace",
    );
}
