//! Tests of the GUI Library's Shows data path (add-gui-library-panel design
//! D12, row 7.1): the All series mode's root load lands the series rows, a
//! selected series' detail and a selected season's episodes land in the
//! series cache, and the Latest and Upcoming modes land their snapshots
//! through the App-side event arms. The Emby boundary is the in-memory mock
//! transport, so no request leaves the process and no worker outlives the
//! assertion.

use super::*;

use crate::gui_core::gui_library::GuiSectionRequest;
use mbv_net::mock_http::MockHttp;
use mbv_queue::TvContentMode;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

const TV_ID: &str = "lib-tv";

/// A series row of the All series root level.
fn series_item(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.to_string();
    item
}

/// A browse level over `items`, hanging off `parent_id`.
fn level(parent_id: &str, title: &str, items: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        parent_id: parent_id.to_string(),
        title: title.to_string(),
        items,
        rows: ServerRows::new(0),
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// A stub `App` whose Emby runtime holds a client over `http`, with a Shows
/// library active: the root level exists and is empty, as a first open finds
/// it before its mode's load lands, with no resolved TV mode yet.
fn tv_app(http: &MockHttp) -> App {
    let mut app = make_app_stub();
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .cached_emby_client_for_test()
        .expect("the installed setup holds a client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = TV_ID.into();
    library.collection_type = "tvshows".into();
    let mut tab = LibraryTab::new(library);
    tab.library_total = Some(0);
    tab.nav_stack.push(level(TV_ID, "Shows", Vec::new()));
    app.push_library_for_test(tab);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app
}

/// Drain one fetch completion, the Client drain's own path: the event rides
/// the lib channel exactly as the fetch worker sends it, and
/// `App::drain_lib_event`'s handler applies the App-side half.
fn land(app: &mut App) {
    let event = app
        .recv_lib_event_within_for_test(Duration::from_secs(2))
        .expect("the fetch completed");
    app.handle_lib_event(event, None);
}

/// The root level's item ids, in order.
fn root_ids(app: &App) -> Vec<String> {
    app.libs_for_test()[0]
        .nav_stack
        .last()
        .expect("the root level exists")
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// Opening the Shows library lands the series list on the App (row 7.1,
/// contract a; gui-collection-dropdown design D8): the GUI's one view mode is
/// the TUI's own `apply_tv_letter_mode` load of All, whatever the TUI's default
/// resolution or a restored position chose, and the mocked transport answers
/// its refresh.
#[test]
fn opening_shows_in_all_mode_lands_the_series_root_rows() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#,
    );
    let mut app = tv_app(&http);

    gui_tv_open(&mut app, 0);
    land(&mut app);

    assert_eq!(
        root_ids(&app),
        ["series-a".to_string(), "series-b".to_string()],
        "the series root rows land on the App"
    );
    let root = &app.libs_for_test()[0].nav_stack[0];
    assert!(!root.loading);
    assert_eq!(
        root.tv_content_mode.as_ref(),
        Some(&TvContentMode::All),
        "the GUI Shows destination opens on the series list"
    );
}

/// Selecting a series lands its seasons in `series_detail_cache`, and
/// selecting a season lands its episodes (row 7.1, contract b): the detail
/// trigger the presenting Client's workspace push owns today starts
/// App-side, and the season reset starts every series on its first season.
#[test]
fn selecting_a_series_lands_its_seasons_and_a_seasons_episodes() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"season-1","Name":"Season 1","Type":"Season"},{"Id":"season-2","Name":"Season 2","Type":"Season"}],"TotalRecordCount":2}"#,
    );
    // The detail landing's own first-season fetch, then the selected
    // season's fetch.
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-2a","Name":"Episode one","Type":"Episode"},{"Id":"ep-2b","Name":"Episode two","Type":"Episode"}],"TotalRecordCount":2}"#,
    );
    let mut app = tv_app(&http);
    app.edit_library_for_test(0, |library| {
        library.nav_stack[0].items = vec![series_item("series-a", "Alfred")];
    });
    // The series list is already the library's mode, so the open asks for no
    // load and the scripted answers belong to the detail fetch alone.
    app.edit_library_for_test(0, |library| {
        library.tv_content_mode = Some(TvContentMode::All);
    });
    gui_tv_open(&mut app, 0);

    gui_tv_select_series(&mut app, 0, "series-a");
    // The detail completion, then its own first-season fetch: drained in
    // order, so the selected season's fetch alone answers the next response.
    land(&mut app);
    land(&mut app);

    let detail = app
        .series_detail("series-a")
        .expect("the series detail landed");
    let seasons: Vec<&str> = detail
        .seasons
        .iter()
        .map(|season| season.id.as_str())
        .collect();
    assert_eq!(seasons, ["season-1", "season-2"]);
    let (series_id, season_id) = crate::gui_core::test_support::stub_gui_state(|gui| {
        let selection = &gui.tv_shows[&0];
        (selection.series_id.clone(), selection.season_id.clone())
    });
    assert_eq!(series_id.as_deref(), Some("series-a"));
    assert_eq!(
        season_id, None,
        "the season pills start on the first season"
    );

    gui_tv_select_season(&mut app, 0, "season-2");
    land(&mut app);

    let detail = app
        .series_detail("series-a")
        .expect("the series detail stays cached");
    let episodes: Vec<&str> = detail.episodes["season-2"]
        .iter()
        .map(|episode| episode.id.as_str())
        .collect();
    assert_eq!(episodes, ["ep-2a", "ep-2b"]);
    assert_eq!(
        crate::gui_core::test_support::stub_gui_state(|gui| gui.tv_shows[&0].season_id.clone())
            .as_deref(),
        Some("season-2")
    );
}

/// The frame path's ensure loads the selected series' first season without a
/// select (design D14): D12's chain landed the detail and stopped there, so
/// the first season was selected but never fetched and the Hero's Workspace
/// painted empty. The destination driver's Shows pass starts the season load
/// the cached detail already makes possible, and a later frame reads the
/// episodes.
#[test]
fn the_frame_path_loads_the_first_seasons_episodes_without_a_select() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-1a","Name":"Episode one","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    let mut app = tv_app(&http);
    // The destination's settled root mode: the open step forces All before
    // any frame pass runs, so the frame ensure starts no root reload here.
    app.edit_library_for_test(0, |library| {
        library.tv_content_mode = Some(TvContentMode::All);
    });
    app.edit_library_for_test(0, |library| {
        library.nav_stack[0].items = vec![series_item("series-a", "Alfred")];
    });
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.tv_shows.insert(
            0,
            crate::gui_core::gui_library::GuiTvShowsSelection {
                series_id: Some("series-a".into()),
                ..Default::default()
            },
        );
    });
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    app.seed_series_detail_for_test(
        "series-a",
        SeriesDetail {
            seasons: vec![season],
            episodes: std::collections::HashMap::new(),
        },
    );

    crate::gui_core::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Shows { lib_idx: 0 },
    );
    land(&mut app);

    let episodes = &app
        .series_detail("series-a")
        .expect("the series detail landed")
        .episodes["season-1"];
    assert_eq!(
        episodes
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["ep-1a"],
        "the Workspace's first season loads without a season select"
    );
}

/// A failed Upcoming fetch clears the waiting root level's loading flag and
/// marks the request `Failed` (row 7.1, contract c): the event rides the lib
/// channel — no worker, and no live retry behind it — and the App-side arm
/// the GUI's drain runs applies it.
#[test]
fn a_failed_upcoming_fetch_clears_the_waiting_levels_loading_mark() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = tv_app(&MockHttp::new());
    app.edit_library_for_test(0, |library| {
        let root = &mut library.nav_stack[0];
        root.tv_content_mode = Some(TvContentMode::Upcoming);
        root.loading = true;
    });
    app.send_lib_event_for_test(LibEvent::ModelContent(
        ModelContentEvent::TvUpcomingSnapshotFailed {
            library_id: TV_ID.to_string(),
        },
    ));

    while app.drain_lib_event() {}

    assert!(
        app.tv_upcoming_fetch_failed(TV_ID),
        "the failed fetch is the retryable mark a later pill reselect retries"
    );
    assert!(
        !app.libs_for_test()[0].nav_stack[0].loading,
        "the waiting level's loading flag unsets"
    );
}

/// The shared landing path's default-mode resolution replaces the first
/// landed series root with the Latest episodes list (`resolve_tv_content_mode`'s
/// any-size Latest default), and the GUI Shows destination projects only
/// Series rows, so a launch restored onto Shows paints a blank tree until a
/// destination click forces the All list (the live defect of 2026-11-18: the
/// launch logged `browse.items.loaded` and `hero=false` until the first
/// `OpenDestination` click). The frame ensure must reload a settled root in
/// any other mode as the All series list, once.
#[test]
fn the_frame_ensure_reloads_a_latest_root_as_the_all_series_list() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-1a","Name":"Episode one","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#,
    );
    let mut app = tv_app(&http);
    // The live launch shape: no root level and no captured total, as the
    // destination opens before any browse fetch has run.
    app.edit_library_for_test(0, |library| {
        library.nav_stack.clear();
        library.library_total = None;
    });

    // The launch open step pushes the root and starts its fetch.
    app.activate_library_position(0);
    land(&mut app);

    // The landing's default-mode resolution swaps the landed series for the
    // Latest episodes list, and that fetch's completion settles it.
    assert_eq!(
        app.libs_for_test()[0].tv_content_mode.as_ref(),
        Some(&TvContentMode::Latest),
        "the first landing resolves the library's default TV mode"
    );
    land(&mut app);
    assert_eq!(
        crate::gui_core::test_support::gui_shows_projection(&mut app, 0).series,
        Vec::new(),
        "a settled Latest root projects no series row, the blank tree"
    );

    // The frame ensure reloads the root as the All series list, once: the
    // second pass finds the All list and starts no second request.
    crate::gui_core::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Shows { lib_idx: 0 },
    );
    land(&mut app);
    crate::gui_core::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Shows { lib_idx: 0 },
    );

    assert_eq!(
        crate::gui_core::test_support::gui_shows_projection(&mut app, 0)
            .series
            .iter()
            .map(|series| series.id.as_str())
            .collect::<Vec<_>>(),
        ["series-a", "series-b"],
        "the All series list the destination presents paints after the reload"
    );
    assert!(
        app.try_recv_lib_event().is_none(),
        "a settled All root starts no second request"
    );
}

/// The six open series a Shows in-view loader test reports, in paint order:
/// more rows than `SHOWS_OPEN_LOAD_LIMIT`, so the loader's bound shows.
const SIX_OPEN_SERIES: [&str; 6] = [
    "series-1", "series-2", "series-3", "series-4", "series-5", "series-6",
];

/// Answer every fetch of `series_ids` with one empty detail: each request
/// carries its series as its `ParentId`, so the answers follow their own path
/// whatever order the workers arrive in.
fn respond_with_empty_detail(http: &MockHttp, series_ids: &[&str]) {
    for series_id in series_ids {
        http.respond_when(
            &format!("ParentId={series_id}"),
            200,
            r#"{"Items":[],"TotalRecordCount":0}"#,
        );
    }
}

/// Report `series` as the open rows the Shows list paints in view (design D5),
/// with every season row closed.
fn report_open_series(app: &mut App, series: &[&str]) {
    apply_gui_library_command(
        app,
        GuiLibraryCommand::ShowsOpenInView {
            lib_idx: 0,
            series: series.iter().map(|id| (*id).to_string()).collect(),
            seasons: Vec::new(),
        },
    );
}

/// Land one queued series event through the GUI core's own drain
/// (`GuiCore::drain_lib_event`): the App-side half and the store effect the
/// landing raises both apply, as the GUI tick's drain does. The event rides
/// the lib channel exactly as the fetch worker sends it.
fn land_through_gui(app: &mut App) {
    let event = app
        .recv_lib_event_within_for_test(Duration::from_secs(2))
        .expect("the detail fetch completed");
    stub_gui_state(|gui| app.handle_lib_event(event, Some(gui)));
}

/// A six-series report starts the loader's limit of detail fetches and no more
/// (design D5): the fifth and sixth open rows wait for a landing, so their
/// seasons arrive only once a slot frees.
#[test]
fn a_six_series_report_starts_exactly_four_detail_fetches() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    respond_with_empty_detail(&http, &SIX_OPEN_SERIES);
    let mut app = tv_app(&http);

    report_open_series(&mut app, &SIX_OPEN_SERIES);

    assert!(app.series_detail_loading("series-1"));
    assert!(app.series_detail_loading("series-2"));
    assert!(app.series_detail_loading("series-3"));
    assert!(app.series_detail_loading("series-4"));
    assert!(
        !app.series_detail_loading("series-5"),
        "the fifth open row waits for a landing"
    );
    assert!(
        !app.series_detail_loading("series-6"),
        "the sixth open row waits for a landing"
    );
}

/// One detail landing frees its slot and the loader starts the next open row
/// (design D5): the report's order decides which row waits, and the landing
/// reaches the loader through the GUI's own store seam.
#[test]
fn one_detail_landing_through_the_gui_drain_starts_the_fifth() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    respond_with_empty_detail(&http, &SIX_OPEN_SERIES);
    let mut app = tv_app(&http);
    report_open_series(&mut app, &SIX_OPEN_SERIES);

    land_through_gui(&mut app);

    assert!(
        app.series_detail_loading("series-5"),
        "the landing frees its slot for the next open row in report order"
    );
}

/// A row the next report drops never starts, even after a landing frees a slot
/// (spec "A row scrolled past"): the report, not the earlier one, decides what
/// the loader starts, so the row that scrolled out of view loads nothing.
#[test]
fn a_series_dropped_from_the_report_before_its_fetch_starts_never_starts() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    respond_with_empty_detail(&http, &SIX_OPEN_SERIES);
    let mut app = tv_app(&http);
    report_open_series(&mut app, &SIX_OPEN_SERIES);

    report_open_series(
        &mut app,
        &["series-1", "series-2", "series-3", "series-4", "series-6"],
    );

    assert!(
        !app.series_detail_loading("series-5"),
        "the row the newer report dropped starts nothing while the loader is full"
    );

    land_through_gui(&mut app);

    assert!(
        app.series_detail_loading("series-6"),
        "the newer report's next row takes the freed slot"
    );
    assert!(
        !app.series_detail_loading("series-5"),
        "the dropped row never starts"
    );
}

/// An open season whose series detail is cached and whose episodes are missing
/// starts its episode fetch through the loader (design D5): the season needs no
/// detail fetch first, and no pick precedes it.
#[test]
fn an_open_season_of_a_cached_series_starts_its_episode_fetch() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when(
        "ParentId=season-1",
        200,
        r#"{"Items":[],"TotalRecordCount":0}"#,
    );
    let mut app = tv_app(&http);
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    app.seed_series_detail_for_test(
        "series-1",
        SeriesDetail {
            seasons: vec![season],
            episodes: std::collections::HashMap::new(),
        },
    );

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsOpenInView {
            lib_idx: 0,
            series: Vec::new(),
            seasons: vec![("series-1".to_string(), "season-1".to_string())],
        },
    );

    assert!(
        app.series_season_loading("series-1", "season-1"),
        "the open season's episodes load without a pick"
    );
}

/// A failed New refetch keeps the landed rows (gui-list-header-buttons design
/// D8): the store keeps the list it already held, so a refresh that fails
/// never empties the painted rows. The Emby boundary is the mock transport and
/// the result lands through the GUI's own client-event drain.
#[test]
fn a_failed_new_refetch_keeps_the_landed_rows() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = tv_app(&http);
    let mut episode = make_item("Episode one", "Episode");
    episode.id = "ep-old".into();
    stub_gui_state(|gui| {
        gui.shows_new
            .insert(TV_ID.to_string(), GuiSectionRequest::Landed(vec![episode]));
    });
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::New,
        },
    );
    http.fail(std::io::ErrorKind::ConnectionRefused);

    stub_gui_state(|gui| {
        crate::gui_core::gui_library::refetch_shows_new(&mut app, gui, TV_ID.to_string());
    });
    crate::gui_core::test_support::wait_for_client_event(&mut app);

    assert_eq!(
        crate::gui_core::test_support::gui_shows_projection(&mut app, 0)
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["ep-old"],
        "the failed refetch keeps the rows the store landed"
    );
}
