//! The quiet in-view series-detail fetch mode (design D6): a quiet landing
//! caches the detail but starts no first-season episode fetch, so an open row
//! in view whose seasons stay closed costs no episode request. A plain fetch
//! of the same id while the quiet one runs drops the mark, so a pick still
//! loads the first season.

use crate::app::tests::make_app_stub;
use mbv_emby_model::test_support::make_item;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::SeriesDetail;

/// A library listing with no items: the answer both requests need, so neither
/// worker stalls on an unscripted transport.
const EMPTY_ITEMS: &str = r#"{"Items":[],"TotalRecordCount":0}"#;

/// One season and no episodes: the landing the quiet rule decides about.
fn series_detail(season_id: &str) -> SeriesDetail {
    let mut season = make_item(season_id, "Season");
    season.id = season_id.into();
    SeriesDetail {
        seasons: vec![season],
        episodes: std::collections::HashMap::new(),
    }
}

/// An `App` holding a ready mock Emby client, so a detail fetch starts.
fn fetchable_app(http: &MockHttp) -> crate::app::App {
    let mut app = make_app_stub();
    let config = crate::app::config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..crate::app::config::Config::default()
    };
    let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    app
}

#[test]
fn quiet_detail_landing_starts_no_first_season_episode_fetch() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);

    app.fetch_series_detail_quiet("show-1");
    app.handle_series_detail_fetched_for_test("show-1", series_detail("season-1"));

    assert!(
        app.series_detail_snapshot("show-1")
            .is_some_and(|detail| detail.seasons.first().is_some_and(|s| s.id == "season-1")),
        "a quiet landing still caches the detail"
    );
    assert!(
        app.series_season_loading_for_test().is_empty(),
        "a quiet landing starts no first-season episode fetch"
    );
}

#[test]
fn quiet_fetch_joining_a_plain_fetch_starts_the_first_season_on_landing() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);

    app.fetch_series_detail("show-1");
    app.fetch_series_detail_quiet("show-1");
    app.handle_series_detail_fetched_for_test("show-1", series_detail("season-1"));

    assert_eq!(
        app.series_season_loading_for_test(),
        &std::collections::HashSet::from([("show-1".to_string(), "season-1".to_string())]),
        "a quiet call that joins a running plain fetch leaves its landing plain"
    );
}

#[test]
fn plain_detail_fetch_while_a_quiet_fetch_runs_starts_the_first_season() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);

    app.fetch_series_detail_quiet("show-1");
    app.fetch_series_detail("show-1");
    app.handle_series_detail_fetched_for_test("show-1", series_detail("season-1"));

    assert_eq!(
        app.series_season_loading_for_test(),
        &std::collections::HashSet::from([("show-1".to_string(), "season-1".to_string())]),
        "a plain fetch of the id while the quiet load runs restores the first-season fetch"
    );
}
