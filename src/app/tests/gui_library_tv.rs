//! Tests of the GUI Library's Shows data path (add-gui-library-panel design
//! D12, row 7.1): the All series mode's root load lands the series rows, a
//! selected series' detail and a selected season's episodes land in the
//! series cache, and the Latest and Upcoming modes land their snapshots
//! through the App-side event arms. The Emby boundary is the in-memory mock
//! transport, so no request leaves the process and no worker outlives the
//! assertion.

use super::*;
use std::sync::Mutex;

use crate::app::state::service_runtime::EmbyRuntime;
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
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = TV_ID.into();
    library.collection_type = "tvshows".into();
    let mut tab = LibraryTab::new(library);
    tab.library_total = Some(0);
    tab.nav_stack.push(level(TV_ID, "Shows", Vec::new()));
    app.libs.push(tab);
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// Drain one fetch completion, the Client drain's own path: the event rides
/// the lib channel exactly as the fetch worker sends it, and
/// `App::drain_lib_event`'s handler applies the App-side half.
fn land(app: &mut App) {
    let event = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the fetch completed");
    app.handle_lib_event(event);
}

/// The root level's item ids, in order.
fn root_ids(app: &App) -> Vec<String> {
    app.libs[0]
        .nav_stack
        .last()
        .expect("the root level exists")
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// Opening the Shows library in the All series mode lands the series root
/// rows on the App (row 7.1, contract a): the mode load is the TUI's own
/// `apply_tv_letter_mode`, whose refresh the mocked transport answers, and
/// the selection stores the mode as its active one.
#[test]
fn opening_shows_in_all_mode_lands_the_series_root_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#,
    );
    let mut app = tv_app(&http);

    app.gui_tv_open(0);
    app.gui_tv_set_mode(0, &TvContentMode::All);
    land(&mut app);

    assert_eq!(
        root_ids(&app),
        ["series-a".to_string(), "series-b".to_string()],
        "the series root rows land on the App"
    );
    let root = &app.libs[0].nav_stack[0];
    assert!(!root.loading);
    assert_eq!(root.tv_content_mode.as_ref(), Some(&TvContentMode::All));
    assert_eq!(app.gui_tv_mode(0).as_ref(), Some(&TvContentMode::All));
}

/// Selecting a series lands its seasons in `series_detail_cache`, and
/// selecting a season lands its episodes (row 7.1, contract b): the detail
/// trigger the presenting Client's workspace push owns today starts
/// App-side, and the season reset starts every series on its first season.
#[test]
fn selecting_a_series_lands_its_seasons_and_a_seasons_episodes() {
    let _guard = crate::config::TestStateDirGuard::new();
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
    app.libs[0].nav_stack[0].items = vec![series_item("series-a", "Alfred")];
    app.gui_tv_open(0);

    app.gui_tv_select_series(0, "series-a");
    // The detail completion, then its own first-season fetch: drained in
    // order, so the selected season's fetch alone answers the next response.
    land(&mut app);
    land(&mut app);

    let detail = app
        .series_detail_cache
        .get("series-a")
        .expect("the series detail landed");
    let seasons: Vec<&str> = detail
        .seasons
        .iter()
        .map(|season| season.id.as_str())
        .collect();
    assert_eq!(seasons, ["season-1", "season-2"]);
    let selection = &app.gui_tv_shows[&0];
    assert_eq!(selection.series_id.as_deref(), Some("series-a"));
    assert_eq!(
        selection.season_id, None,
        "the season pills start on the first season"
    );

    app.gui_tv_select_season(0, "season-2");
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
    assert_eq!(app.gui_tv_shows[&0].season_id.as_deref(), Some("season-2"));
}

/// The Latest mode lands its rows on the App (row 7.1, contract c): the
/// mode's own fetch (`get_latest_episodes`) replaces the root level, whose
/// fetch fields the spawn aligned first so the completion is not stale.
#[test]
fn the_latest_mode_lands_its_rows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-new","Name":"New episode","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    let mut app = tv_app(&http);
    app.gui_tv_open(0);

    app.gui_tv_set_mode(0, &TvContentMode::Latest);
    land(&mut app);

    assert_eq!(root_ids(&app), ["ep-new".to_string()]);
    let root = &app.libs[0].nav_stack[0];
    assert!(!root.loading);
    assert_eq!(root.item_types.as_deref(), Some("Episode"));
    assert_eq!(app.gui_tv_mode(0).as_ref(), Some(&TvContentMode::Latest));
}

/// The Upcoming mode lands its snapshot on the App (row 7.1, contract c):
/// the fetch's completion stores the per-session premieres and fills the
/// waiting Upcoming root level, through the same acceptance the presenting
/// Client's shell drain runs.
#[test]
fn the_upcoming_mode_lands_its_snapshot() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-1","Type":"Episode","SeriesId":"series-a","SeriesName":"Alfred"}]}"#,
    );
    let mut app = tv_app(&http);
    app.gui_tv_open(0);

    app.gui_tv_set_mode(0, &TvContentMode::Upcoming);
    // The Upcoming build sends the snapshot first, then the level's Loaded.
    land(&mut app);
    land(&mut app);

    let premieres = &app.tv_upcoming_snapshots[TV_ID];
    assert_eq!(premieres.len(), 1);
    assert_eq!(premieres[0].series_name, "Alfred");
    assert!(
        !app.tv_upcoming_fetches.contains_key(TV_ID),
        "the acceptance clears the request mark"
    );
    let root = &app.libs[0].nav_stack[0];
    assert!(!root.loading);
    assert_eq!(
        root.tv_content_mode.as_ref(),
        Some(&TvContentMode::Upcoming)
    );
    assert!(
        root.items.is_empty(),
        "premieres are not level items; they live in the snapshot"
    );
}

/// A failed Upcoming fetch clears the waiting root level's loading flag and
/// marks the request `Failed` (row 7.1, contract c): the event rides the lib
/// channel — no worker, and no live retry behind it — and the App-side arm
/// the GUI's drain runs applies it.
#[test]
fn a_failed_upcoming_fetch_clears_the_waiting_levels_loading_mark() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = tv_app(&MockHttp::new());
    let root = &mut app.libs[0].nav_stack[0];
    root.tv_content_mode = Some(TvContentMode::Upcoming);
    root.loading = true;
    let _ = app.channels.lib_tx.send(LibEvent::ModelContent(
        ModelContentEvent::TvUpcomingSnapshotFailed {
            library_id: TV_ID.to_string(),
        },
    ));

    while app.drain_lib_event() {}

    assert_eq!(
        app.tv_upcoming_fetches.get(TV_ID),
        Some(&crate::app::state::app_struct::TvUpcomingFetchState::Failed),
        "the failed fetch is the retryable mark a later pill reselect retries"
    );
    assert!(
        !app.libs[0].nav_stack[0].loading,
        "the waiting level's loading flag unsets"
    );
}
