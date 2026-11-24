//! Tests of the GUI Library's Shows data path (add-gui-library-panel design
//! D12, row 7.1): the All series mode's root load lands the series rows, a
//! selected series' detail and a selected season's episodes land in the
//! series cache, and the Latest and Upcoming modes land their snapshots
//! through the App-side event arms. The Emby boundary is the in-memory mock
//! transport, so no request leaves the process and no worker outlives the
//! assertion.

use super::*;

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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
    let (series_id, season_id) = crate::app::tests::stub_gui_state(|gui| {
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
        .series_detail_cache
        .get("series-a")
        .expect("the series detail stays cached");
    let episodes: Vec<&str> = detail.episodes["season-2"]
        .iter()
        .map(|episode| episode.id.as_str())
        .collect();
    assert_eq!(episodes, ["ep-2a", "ep-2b"]);
    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| gui.tv_shows[&0].season_id.clone()).as_deref(),
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
    crate::app::tests::stub_gui_state(|gui| {
        gui.tv_shows.insert(
            0,
            crate::app::gui_library::GuiTvShowsSelection {
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

    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Shows { lib_idx: 0 });
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
        crate::app::tests::gui_shows_projection(&mut app, 0).series,
        Vec::new(),
        "a settled Latest root projects no series row, the blank tree"
    );

    // The frame ensure reloads the root as the All series list, once: the
    // second pass finds the All list and starts no second request.
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Shows { lib_idx: 0 });
    land(&mut app);
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Shows { lib_idx: 0 });

    assert_eq!(
        crate::app::tests::gui_shows_projection(&mut app, 0)
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
