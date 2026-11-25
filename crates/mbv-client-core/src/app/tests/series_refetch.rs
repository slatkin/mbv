//! The quiet in-view series-detail fetch mode (design D6): a quiet landing
//! caches the detail but starts no first-season episode fetch, so an open row
//! in view whose seasons stay closed costs no episode request. A plain fetch
//! of the same id while the quiet one runs drops the mark, so a pick still
//! loads the first season. The core refetches (design D9) belong here too: a
//! stale series detail's landing replaces the seasons and keeps the episodes of
//! the seasons it still holds, a failed detail refetch keeps the cached detail,
//! and a failed album-track refresh keeps the cached tracks.

use std::time::Duration;

use crate::app::tests::make_app_stub;
use crate::app::{LibEvent, MusicEvent, SeriesEvent};
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

/// One track, as a landed album-track fetch leaves it in the cache.
fn cached_track(track_id: &str) -> Vec<mbv_emby_model::EmbyItem> {
    let mut track = make_item(track_id, "Audio");
    track.id = track_id.into();
    vec![track]
}

/// One episode, as a landed season fetch leaves it in the cache.
fn cached_episode(episode_id: &str) -> Vec<mbv_emby_model::EmbyItem> {
    let mut episode = make_item(episode_id, "Episode");
    episode.id = episode_id.into();
    vec![episode]
}

#[test]
fn a_failed_album_tracks_refetch_keeps_the_cached_tracks() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 1);
    let mut app = fetchable_app(&http);
    app.cache_album_tracks("album-1".into(), cached_track("track-1"));

    app.refetch_album_tracks("album-1".into());
    app.handle_lib_event(
        LibEvent::Music(MusicEvent::AlbumTracksRefreshFailed {
            album_id: "album-1".into(),
        }),
        None,
    );

    assert_eq!(
        app.album_tracks_cache["album-1"][0].id, "track-1",
        "a failed refetch keeps the tracks the cache already holds"
    );
    assert!(
        !app.album_tracks_loading_for_test("album-1"),
        "a failed refetch frees the loading mark so a later refresh starts"
    );
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
fn second_quiet_call_on_a_running_quiet_fetch_keeps_the_quiet_mark() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);

    app.fetch_series_detail_quiet("show-1");
    app.fetch_series_detail_quiet("show-1");
    app.handle_series_detail_fetched_for_test("show-1", series_detail("season-1"));

    assert!(
        app.series_season_loading_for_test().is_empty(),
        "a second quiet call starts no fetch, so the landing stays quiet"
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

#[test]
fn a_stale_detail_landing_replaces_the_seasons_and_keeps_the_old_episodes() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);
    let mut cached = series_detail("season-1");
    cached
        .episodes
        .insert("season-1".into(), cached_episode("episode-1"));
    app.cache_series_detail("show-1", cached);
    app.mark_series_detail_stale("show-1");
    let mut landed = series_detail("season-1");
    let mut second = make_item("season-2", "Season");
    second.id = "season-2".into();
    landed.seasons.push(second);

    app.handle_series_detail_fetched_for_test("show-1", landed);

    let detail = app
        .series_detail_snapshot("show-1")
        .expect("a stale landing is cached");
    assert_eq!(
        detail
            .seasons
            .iter()
            .map(|season| season.id.as_str())
            .collect::<Vec<_>>(),
        vec!["season-1", "season-2"],
        "a stale landing replaces the seasons the view paints"
    );
    assert_eq!(
        detail.episodes["season-1"][0].id, "episode-1",
        "a season the new detail still holds keeps the episodes the cache holds"
    );
}

/// A failed quiet in-view detail load caches nothing (PR 1065 review, finding
/// 1): the view never holds a poisoned empty detail that no scroll or reopen
/// could fill, and the loading mark retires so a later report or landing
/// starts the load again. The pick path's failure keeps its empty fallback.
#[test]
fn a_failed_quiet_detail_load_caches_nothing_and_retires_the_marks() {
    let http = MockHttp::new();
    http.respond(500, "Error");
    let mut app = fetchable_app(&http);

    app.fetch_series_detail_quiet("show-1");
    let event = app
        .recv_lib_event_within_for_test(Duration::from_secs(2))
        .expect("the quiet fetch's failure event");
    app.handle_lib_event(event, None);

    assert!(
        app.series_detail_snapshot("show-1").is_none(),
        "a failed quiet load stores no empty detail"
    );
    assert!(
        !app.series_detail_loading("show-1"),
        "a failed quiet load retires the loading mark so a later load starts"
    );
}

/// A failed in-view first episode fetch stores no empty list (PR 1065 review,
/// finding 1): the open season stays in its not-loaded state, so the loader's
/// next pump starts the load again, and the view never paints a landed-but-
/// empty season a retry could never fill.
#[test]
fn a_failed_in_view_season_load_stores_no_empty_episode_list() {
    let http = MockHttp::new();
    http.respond(500, "Error");
    let mut app = fetchable_app(&http);
    app.cache_series_detail("show-1", series_detail("season-1"));

    app.fetch_series_season_episodes_in_view("show-1".into(), "season-1".into());
    let event = app
        .recv_lib_event_within_for_test(Duration::from_secs(2))
        .expect("the season fetch's failure event");
    app.handle_lib_event(event, None);

    let detail = app
        .series_detail_snapshot("show-1")
        .expect("the cached detail stays");
    assert!(
        !detail.episodes.contains_key("season-1"),
        "a failed in-view season load stores no empty episode list"
    );
    assert!(
        !app.series_season_loading("show-1", "season-1"),
        "a failed in-view season load retires the loading mark"
    );
}

#[test]
fn a_failed_detail_refetch_keeps_the_cached_detail_and_clears_the_stale_mark() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 1);
    let mut app = fetchable_app(&http);
    app.cache_series_detail("show-1", series_detail("season-1"));
    app.mark_series_detail_stale("show-1");

    app.refetch_series_detail("show-1");
    app.handle_lib_event(
        LibEvent::Series(SeriesEvent::DetailRefreshFailed {
            series_id: "show-1".into(),
        }),
        None,
    );

    assert!(
        app.series_detail_snapshot("show-1")
            .is_some_and(|detail| detail
                .seasons
                .first()
                .is_some_and(|season| season.id == "season-1")),
        "a failed refetch keeps the detail the cache holds"
    );
    assert!(
        !app.series_detail_stale("show-1"),
        "a failed refetch clears the stale mark, so it never retries on each landing"
    );
}

/// A season landing keeps the detail refetch's loading mark while its fetch runs
/// (unit 7 review, finding P1): the series' own detail fetch is still in flight,
/// so the mark holds and the loader starts no second refetch, and the detail
/// landing then clears it. Without the in-flight set the season landing cleared
/// the mark, and the second refetch's landing ran without its quiet mark and
/// started a first-season fetch nobody asked for.
#[test]
fn a_season_landing_keeps_the_running_detail_refetches_loading_mark() {
    let http = MockHttp::new();
    http.respond_repeated(200, EMPTY_ITEMS, 2);
    let mut app = fetchable_app(&http);
    let mut cached = series_detail("season-1");
    cached
        .episodes
        .insert("season-1".into(), cached_episode("episode-1"));
    app.cache_series_detail("show-1", cached);
    app.mark_series_detail_stale("show-1");
    app.mark_series_season_stale("show-1", "season-1");

    app.refetch_series_detail("show-1");
    app.refetch_series_season_episodes("show-1".into(), "season-1".into());
    app.handle_lib_event(
        LibEvent::Series(SeriesEvent::SeasonEpisodesFetched {
            series_id: "show-1".into(),
            season_id: "season-1".into(),
            episodes: cached_episode("episode-2"),
        }),
        None,
    );

    assert!(
        app.series_detail_loading("show-1"),
        "a season landing keeps the loading mark of the detail fetch still running"
    );

    app.handle_series_detail_fetched_for_test("show-1", series_detail("season-1"));

    assert!(
        !app.series_detail_loading("show-1"),
        "the detail landing itself clears the loading mark"
    );
}
