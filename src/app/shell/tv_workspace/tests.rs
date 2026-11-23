use super::*;
use crate::app::tests::render_fixtures::make_movie_app;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_msg::{Msg, ShellRequest};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

mod group;

mod selection;

fn mounted_tv_model_with_mock_emby(http: &MockHttp) -> Model {
    let mut model = mounted_tv_model();
    let mut config = model.app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    crate::app::tests::install_test_emby(&mut model.app, config);
    let client = model
        .app
        .cached_emby_client_for_test()
        .expect("the installed test Emby client")
        .with_test_agent(http.agent());
    model.app.adopt_ready_emby_client_for_test(client);
    model
}

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.edit_library_for_test(0, |tab| {
        tab.library.collection_type = "tvshows".into();
        for item in &mut tab.nav_stack[0].items {
            item.item_type = "Series".into();
            item.image_tags.thumb = "tag".into();
        }
    });
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_show_area`), not this previous-frame paint rect.
    let mut model = Model::new(app);
    model.terminal_width = 160;
    model.terminal_height = 40;
    model.sync_tv_content();
    model.sync_active_destination();
    model
}

/// Task 2.1: the Wide push prefetches the identical canonical key the
/// painter requests, so `paint_home_image` on the consumed `HomeImagePaint`
/// starts no additional fetch and the reservation survives the paint.
///
/// The fixture has no Emby client, so `spawn_image_fetch` balances
/// `image_fetches_active` synchronously and never fills
/// `pending_image_fetches`: the counters cannot see an extra paint-time fetch.
/// The reservation set can — a fetch always reserves its own key first, so a
/// divergent paint-time key shows up as an extra entry in `card_image_loading`.
#[test]
fn expanding_an_uncached_show_starts_the_detail_fetch() {
    let http = MockHttp::new();
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    let mut model = mounted_tv_model_with_mock_emby(&http);
    assert!(model.app.series_detail_loading_for_test().is_empty());
    assert!(model.app.series_detail_snapshot("movie-focused").is_none());

    model.handle_tv_request(ShellRequest::TvTreeExpand {
        target: mbv_ui_msg::TvTreeTarget::Show("tv-id:13:movie-focused".into()),
    });

    assert_eq!(
        model.app.series_detail_loading_for_test(),
        &std::collections::HashSet::from(["movie-focused".into()]),
        "uncached expansion must arm the detail fetch"
    );
}

#[test]
fn season_pill_click_arms_the_selected_seasons_lazy_episode_fetch() {
    // Guards the blank-season-pill regression: only the first season's
    // episodes arrive with the series detail, so a clicked pill must arm
    // the same lazy fetch a keyboard season move (`TvSeasonMove`) arms --
    // otherwise every other pill's workspace stays empty forever.
    let http = MockHttp::new();
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    let mut model = mounted_tv_model_with_mock_emby(&http);
    let mut season_one = mbv_emby_model::test_support::make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = mbv_emby_model::test_support::make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), Vec::new());
    model.app.seed_series_detail_for_test(
        "movie-focused",
        mbv_ui_model::browse::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    // The owner moves its season cursor before the shell sees the click
    // (what a real pill click does in `apply_pane_click`).
    model.test_tv_owner_mut().move_season(1);

    model.handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::TvHitClick {
            hit: mbv_ui_msg::TvHit::SeasonTab(1),
        })),
        &mut false,
        &mut false,
    );

    assert!(
        model
            .app
            .series_season_loading_for_test()
            .contains(&("movie-focused".into(), "season-2".into())),
        "the clicked pill's season must be fetching, not blank"
    );
}

#[test]
fn season_expansion_waits_for_detail_then_fetches_only_the_requested_season() {
    let http = MockHttp::new();
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    let mut model = mounted_tv_model_with_mock_emby(&http);

    model
        .app
        .fetch_series_season_episodes("movie-focused".into(), "season-2".into());
    // Duplicate expansion intent is deduplicated while detail is in flight.
    model
        .app
        .fetch_series_season_episodes("movie-focused".into(), "season-2".into());
    assert_eq!(
        model.app.pending_series_season_expansions_for_test(),
        &std::collections::HashSet::from([("movie-focused".into(), "season-2".into())])
    );
    assert!(
        model
            .app
            .series_detail_loading_for_test()
            .contains("movie-focused")
    );

    let seasons = ["season-1", "season-2", "season-3"]
        .into_iter()
        .map(|id| {
            let mut season = mbv_emby_model::test_support::make_item(id, "Season");
            season.id = id.into();
            season
        })
        .collect();
    model.app.handle_series_detail_fetched(
        "movie-focused",
        mbv_ui_model::browse::SeriesDetail {
            seasons,
            episodes: std::collections::HashMap::new(),
        },
    );

    assert!(
        model
            .app
            .pending_series_season_expansions_for_test()
            .is_empty()
    );
    assert_eq!(
        model.app.series_season_loading_for_test(),
        &std::collections::HashSet::from([
            ("movie-focused".into(), "season-1".into()),
            ("movie-focused".into(), "season-2".into()),
        ]),
        "preserve the Hero's first-season fetch and fetch the expanded season, but not every season"
    );
}

#[test]
fn late_series_detail_completion_does_not_replace_cached_detail() {
    let mut app = make_movie_app();
    let mut cached_season = mbv_emby_model::test_support::make_item("Current", "Season");
    cached_season.id = "current-season".into();
    app.seed_series_detail_for_test(
        "show-id",
        mbv_ui_model::browse::SeriesDetail {
            seasons: vec![cached_season],
            episodes: std::collections::HashMap::new(),
        },
    );

    let mut stale_season = mbv_emby_model::test_support::make_item("Stale", "Season");
    stale_season.id = "stale-season".into();
    app.handle_series_detail_fetched(
        "show-id",
        mbv_ui_model::browse::SeriesDetail {
            seasons: vec![stale_season],
            episodes: std::collections::HashMap::new(),
        },
    );

    assert_eq!(
        app.series_detail_snapshot("show-id")
            .and_then(|detail| detail.seasons.first().map(|season| season.id.clone())),
        Some("current-season".to_string())
    );
}

#[test]
fn push_tv_workspace_content_fetches_uncached_selected_series_once() {
    let mut model = mounted_tv_model();
    model.app.adopt_ready_emby_runtime_for_test();

    model.push_tv_workspace_content();
    assert!(
        model
            .app
            .series_detail_loading_for_test()
            .contains("movie-focused")
    );

    // Re-pushing the same selection does not duplicate the request.
    model.push_tv_workspace_content();
    assert_eq!(model.app.series_detail_loading_for_test().len(), 1);

    model.app.seed_series_detail_for_test(
        "movie-focused",
        mbv_ui_model::browse::SeriesDetail {
            seasons: Vec::new(),
            episodes: std::collections::HashMap::new(),
        },
    );
    model.app.clear_series_detail_loading_for_test();
    model.push_tv_workspace_content();
    assert!(model.app.series_detail_loading_for_test().is_empty());
}

/// A Wide TV row move stamps navigation before the push in its handler, so
/// that push starts no series-detail request. The idle case is covered by the
/// test above.
#[test]
fn wide_tv_row_move_defers_series_detail_fetch() {
    let mut model = mounted_tv_model();
    model.app.adopt_ready_emby_runtime_for_test();

    model.handle_tv_request(ShellRequest::TvMoveRows { rows: 1 });

    assert!(model.app.series_detail_loading_for_test().is_empty());
}

/// keep-destination-components-mounted task 3.1: the TV workspace stays
/// mounted across wide→narrow→wide layout resizes (keep-mounted, D1).
/// Because the component is never unmounted/remounted, its private
/// pane/cursor state survives the round trip: after re-point it is not
/// reset to the fresh-mount default (cursor 0 / Series pane).
#[test]
fn tv_workspace_stays_mounted_and_preserves_pane_cursor_across_resize() {
    let mut model = mounted_tv_model();
    assert!(model.library_panel_has_owner(&model.test_tv_owner_key()));

    // Move the tree selection to movie-second and translate the stable show
    // target through the existing shell row-selection request.
    let move_request = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        move_request,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
            hit: mbv_ui_msg::TvHit::SeriesRow(target)
        } if target == "movie-second")));
    model
        .app
        .handle_mouse_single_click_tv(0, mbv_ui_msg::TvHit::SeriesRow("movie-second".into()));
    model.push_tv_workspace_content();
    let selected_id = |model: &mut Model| {
        model
            .test_tv_owner()
            .selected_tree_show()
            .map(|item| item.id)
    };
    assert_eq!(selected_id(&mut model), Some("movie-second".into()));

    // Seed detail for the selected series (movie-second, the row the
    // component cursor sits on after Down) and Enter it so the component
    // enters the Episodes pane (episode_cursor becomes Some(0)).
    let mut season = mbv_emby_model::test_support::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = mbv_emby_model::test_support::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    episode.series_id = "movie-second".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    model.app.seed_series_detail_for_test(
        "movie-second",
        mbv_ui_model::browse::SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    let enter = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        enter,
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { .. })));
    assert_eq!(
        model
            .test_tv_owner()
            .selected_episode_item()
            .map(|episode| episode.id),
        Some("episode-1".into()),
        "Enter must put the component in the Episodes pane for its selected series"
    );

    // Narrow: the merged owner stays the same mounted component (task 8.1,
    // design D12) -- the pointer never clears, unlike the pre-merge
    // two-component hand-off.
    model.terminal_width = 80;
    model.terminal_height = 24;
    model.sync_tv_content();
    model.sync_active_destination();
    assert!(
        model.library_panel_has_owner(&model.test_tv_owner_key()),
        "the TV owner stays installed across the narrow resize"
    );

    // Wide again: still the same component.
    model.terminal_width = 160;
    model.terminal_height = 40;
    model.sync_tv_content();
    model.sync_active_destination();
    assert!(
        model.library_panel_has_owner(&model.test_tv_owner_key()),
        "the TV owner stays installed across the wide resize"
    );
    assert_eq!(
        selected_id(&mut model),
        Some("movie-second".into()),
        "the component cursor must survive the wide→narrow→wide round trip"
    );
    assert_eq!(
        model
            .test_tv_owner()
            .selected_episode_item()
            .map(|episode| episode.id),
        Some("episode-1".into()),
        "the Episodes pane must survive the wide→narrow→wide round trip"
    );
}

mod activation;

/// Contract (2026-10-15): after the eager Upcoming fetch fails through all
/// its retries, the sync pass maps the `Failed` fetch state into the owner's
/// `upcoming_failed` and delivers an empty, not-loading placeholder slot,
/// leaving the `Failed` state marked for the shell's retry paths. The
/// placeholder's wording is the component test's contract
/// (`flat_upcoming_placeholder_tests`).
#[test]
fn the_failed_upcoming_fetch_projects_into_the_panel_placeholder() {
    use mbv_components::library_panel::ListSlot;

    let http = MockHttp::new();
    let mut model = mounted_tv_model_with_mock_emby(&http);
    let library_id = model
        .app
        .emby_library(0)
        .map(|tab| tab.library.id.as_str())
        .expect("the TV library tab")
        .to_string();
    model
        .app
        .set_library_tv_content_mode_for_test(0, mbv_queue::TvContentMode::Upcoming);
    model
        .app
        .mark_tv_upcoming_fetch_failed_for_test(&library_id);

    model.sync_tv_content();

    assert!(
        model.test_tv_owner().upcoming_failed(),
        "the Failed fetch state must project into the owner's context"
    );
    match model.test_tv_owner_mut().panel_content().list {
        ListSlot::Empty { loading, .. } => {
            assert!(!loading, "the failed fetch is no longer in flight");
        }
        other => panic!("a failed Upcoming fetch must show the placeholder slot, got {other:?}"),
    }
    assert!(
        model.app.tv_upcoming_fetch_failed(&library_id),
        "the failed state stays marked for the shell's retry paths",
    );
}
