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

use super::{LIB_EVENT_BOUND, TestCore, pump_until, send_library};
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

/// Pump GUI ticks until the root level holds rows: the fetch worker's
/// completion lands on the lib channel, and each round blocks for one more
/// of them instead of sleeping between steps.
fn pump_until_root_rows(test: &mut TestCore) {
    loop {
        assert!(test.core.step(), "the core thread never quits mid-drain");
        if test
            .core
            .app_mut()
            .tv_root_item_ids_for_test(0)
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
        {
            return;
        }
        assert!(
            test.core
                .app_mut()
                .drain_lib_event_blocking_for_test(LIB_EVENT_BOUND),
            "the All series mode's root load sent no further completion"
        );
    }
}

/// The Shows section a frame carries, present only while a Shows
/// destination presents.
fn shows_section(frame: &crate::Held) -> Option<&mbv_gui::bridge::frame::library::GuiShowsFrame> {
    frame.library()?.shows.as_ref()
}

/// Pump GUI ticks until the Shows section a frame carries satisfies `want`,
/// waiting for the next library completion between steps: the section is
/// absent from a frame that re-carried no changed content, so a miss waits
/// for the fetch that changes it.
fn pump_until_shows(
    test: &mut TestCore,
    want: impl Fn(&mbv_gui::bridge::frame::library::GuiShowsFrame) -> bool,
    what: &str,
) {
    pump_until(test, |frame| shows_section(frame).is_some_and(&want), what);
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
/// `drain_lib_event` applies the refresh completion's App arm. The mock
/// answers each request by its own path, so the open's eager Upcoming
/// snapshot and the mode's load cannot take each other's answer.
#[test]
fn opening_shows_in_all_mode_lands_the_series_root_rows_on_the_tick() {
    let http = MockHttp::new();
    http.respond_when("IncludeItemTypes=Series", 200, SERIES);
    http.respond_when("/Shows/Upcoming", 200, NO_PREMIERES);
    let mut test = shows_core(&http);

    let app = test.core.app_mut();
    app.gui_tv_open(0);
    app.gui_tv_set_mode(0, &mbv_queue::TvContentMode::All);
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
    let frame = test.held();
    let shows = shows_section(&frame).expect("the open Shows destination presents its content");

    assert_eq!(
        shows.list.lib_idx, 0,
        "the frame names the library the commands carry"
    );
    let names: Vec<&str> = shows
        .list
        .pills
        .iter()
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(names, ["New", "Upcoming", "All"]);
    let selected: Vec<&str> = shows
        .list
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.name.as_str())
        .collect();
    assert_eq!(
        selected,
        ["New"],
        "one mode pill is selected, the resolved default"
    );
    assert!(
        !shows.list.pills.iter().any(|pill| pill.collection),
        "the mode pills are single-select"
    );
    assert!(
        shows.list.rows.is_empty() && shows.detail.hero.is_none(),
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
    let frame = test.held();
    let shows = shows_section(&frame).expect("the open's frame carries the Shows content");
    assert_eq!(
        shows.list.lib_idx, lib_idx,
        "the frame names the library the commands carry"
    );
}

/// Pump until the All series mode selects `series` and the Hero's title is
/// `title`.
fn pump_until_series_hero(test: &mut TestCore, series: &str, title: &str) {
    pump_until_shows(
        test,
        |shows| {
            shows.list.series.len() == 2
                && shows.detail.selected.as_deref() == Some(series)
                && shows
                    .detail
                    .hero
                    .as_ref()
                    .is_some_and(|hero| hero.title == title)
        },
        "the All series mode's rows and Hero",
    );
}

/// Pump until the Workspace shows `episode_id` under `season_id`.
fn pump_until_workspace(test: &mut TestCore, season_id: &str, episode_id: &str, what: &str) {
    pump_until_shows(
        test,
        |shows| {
            shows
                .detail
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
/// season's episodes. The mock answers every request by its own path, so the
/// concurrent fetch workers' interleaving cannot change an answer (the
/// pinned total keeps the root load's completion from spawning a
/// default-mode reload behind the script).
#[test]
fn a_series_select_moves_the_hero_and_a_season_select_moves_the_workspace() {
    let http = MockHttp::new();
    // The open's default root load and the set-mode command's All series
    // load read the series list, the eager Upcoming snapshot reads the
    // premieres, each series' detail reads its own season list, and each
    // season fetch reads its own episodes.
    http.respond_when("IncludeItemTypes=Series", 200, SERIES);
    http.respond_when("/Shows/Upcoming", 200, NO_PREMIERES);
    http.respond_when("ParentId=series-a&", 200, SEASONS_A);
    http.respond_when("ParentId=series-b&", 200, SEASONS_B);
    http.respond_when("ParentId=s1-a&", 200, SEASON_1_A);
    http.respond_when("ParentId=s1-b&", 200, SEASON_1_B);
    http.respond_when("ParentId=s2-b&", 200, SEASON_2_B);
    let mut test = shows_command_core(&http);

    open_shows_destination(&mut test, 2, 1);
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSetMode {
            lib_idx: 1,
            mode: mbv_gui::bridge::library::GuiShowsMode::All,
        },
    );
    pump_until_series_hero(&mut test, "series-a", "Alfred");
    pump_until_workspace(&mut test, "s1-a", "ep-a1", "the default series' Workspace");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSelect {
            lib_idx: 1,
            row: "series-b".into(),
        },
    );
    let frame = test.held();
    let shows = shows_section(&frame).expect("the frame carries the Shows content");
    assert_eq!(
        shows.detail.selected.as_deref(),
        Some("series-b"),
        "the frame paints the core's selection"
    );
    assert_eq!(
        shows.detail.hero.as_ref().expect("the Hero moved").title,
        "Bridgerton",
        "the Hero's facts follow the selected series"
    );
    pump_until_workspace(&mut test, "s1-b", "ep-b1", "the selected series' Workspace");

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::ShowsSelectSeason {
            lib_idx: 1,
            season_id: "s2-b".into(),
        },
    );
    pump_until_workspace(&mut test, "s2-b", "ep-b2", "the season command's Workspace");
}
