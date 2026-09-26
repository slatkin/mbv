use super::*;
use crate::app::components::{Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::make_movie_app;
use crate::app::state::types::browse::BrowseResting;
use mbv_core::mock_http::MockHttp;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

mod group;

mod selection;

fn mounted_tv_model_with_mock_emby(http: &MockHttp) -> Model {
    let mut model = mounted_tv_model();
    let mut config = model.app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    crate::app::tests::install_test_emby(&mut model.app, config);
    let client = model
        .app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model
}

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_library_area`), not this previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
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
    assert!(model.app.series_detail_loading.is_empty());
    assert!(model.app.series_detail_cache.is_empty());

    model.handle_tv_request(ShellRequest::TvTreeExpand {
        target: crate::app::components::tv_tree_target::TvTreeTarget::Show(
            "tv-id:13:movie-focused".into(),
        ),
    });

    assert_eq!(
        model.app.series_detail_loading,
        std::collections::HashSet::from(["movie-focused".into()]),
        "uncached expansion must arm the detail fetch"
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
        model.app.pending_series_season_expansions,
        std::collections::HashSet::from([("movie-focused".into(), "season-2".into())])
    );
    assert!(model.app.series_detail_loading.contains("movie-focused"));

    let seasons = ["season-1", "season-2", "season-3"]
        .into_iter()
        .map(|id| {
            let mut season = crate::app::tests::make_item(id, "Season");
            season.id = id.into();
            season
        })
        .collect();
    model.app.handle_series_detail_fetched(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons,
            episodes: std::collections::HashMap::new(),
        },
    );

    assert!(model.app.pending_series_season_expansions.is_empty());
    assert_eq!(
        model.app.series_season_loading,
        std::collections::HashSet::from([
            ("movie-focused".into(), "season-1".into()),
            ("movie-focused".into(), "season-2".into()),
        ]),
        "preserve the Hero's first-season fetch and fetch the expanded season, but not every season"
    );
}

#[test]
fn late_series_detail_completion_does_not_replace_cached_detail() {
    let mut app = make_movie_app();
    let mut cached_season = crate::app::tests::make_item("Current", "Season");
    cached_season.id = "current-season".into();
    app.series_detail_cache.insert(
        "show-id".into(),
        crate::app::SeriesDetail {
            seasons: vec![cached_season],
            episodes: std::collections::HashMap::new(),
        },
    );

    let mut stale_season = crate::app::tests::make_item("Stale", "Season");
    stale_season.id = "stale-season".into();
    app.handle_series_detail_fetched(
        "show-id".into(),
        crate::app::SeriesDetail {
            seasons: vec![stale_season],
            episodes: std::collections::HashMap::new(),
        },
    );

    assert_eq!(
        app.series_detail_cache["show-id"].seasons[0].id,
        "current-season"
    );
}

#[test]
fn push_tv_workspace_content_fetches_uncached_selected_series_once() {
    let mut model = mounted_tv_model();
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    model.push_tv_workspace_content();
    assert!(model.app.series_detail_loading.contains("movie-focused"));

    // Re-pushing the same selection does not duplicate the request.
    model.push_tv_workspace_content();
    assert_eq!(model.app.series_detail_loading.len(), 1);

    model.app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: Vec::new(),
            episodes: std::collections::HashMap::new(),
        },
    );
    model.app.series_detail_loading.clear();
    model.push_tv_workspace_content();
    assert!(model.app.series_detail_loading.is_empty());
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
            hit: crate::app::components::msg::TvHit::SeriesRow(ref target)
        } if target == "movie-second")));
    model.app.handle_mouse_single_click_tv(
        0,
        crate::app::components::msg::TvHit::SeriesRow("movie-second".into()),
    );
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
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = crate::app::tests::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    episode.series_id = "movie-second".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    model.app.series_detail_cache.insert(
        "movie-second".into(),
        crate::app::SeriesDetail {
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
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.sync_tv_content();
    model.sync_active_destination();
    assert!(
        model.library_panel_has_owner(&model.test_tv_owner_key()),
        "the TV owner stays installed across the narrow resize"
    );

    // Wide again: still the same component.
    model.app.terminal_width = 160;
    model.app.terminal_height = 40;
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

/// unify-screens-under-panel-components task 8.1 (design D12): resizing
/// across the wide TV breakpoint and back keeps the visually-selected
/// series. The merged `TvContent` owner is the one owner at every
/// breakpoint now; its shared `MediaListCarrier` preserves the selected
/// target across the Wide<->Inline presentation switch on its own -- there
/// is no cross-component hand-off left to carry it.
/// Entering a wide TV library must route straight to the TV owner
/// on the *first* `sync_mounted_surfaces()` after the tab flips — no
/// one-frame narrow `TvContent` flash. `App::wide_tv_library_area`
/// alone is a previous-frame paint signal; `prime_wide_tv_geometry` publishes
/// the wide geometry synchronously from terminal size so the mount gate is
/// correct immediately.
/// Build a two-level stack: a Series parent list whose cursor is parked
/// off the child's parent, plus an empty Seasons child whose `parent_id`
/// points back at parent item 0. Used to prove `go_back` restores the
/// parent cursor by `parent_id`, never by the popped (mirrored) cursor.
/// Build a two-level stack: a Series parent list (three rows, cursor
/// parked off the child's parent) plus a Seasons child whose `parent_id`
/// targets a discriminating nonzero parent row (index 2) and whose items
/// contain the component's selected id so the mirror actually mutates
/// this (last) level's cursor. Used to prove `go_back` restores the parent
/// cursor by `parent_id`, never by the popped (mirrored) cursor.
fn tv_two_level_model() -> Model {
    let mut model = mounted_tv_model();
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    model.app.libs[0].nav_stack[0].set_resting_cursor(0);
    let mut mirror_target = crate::app::tests::make_item("S", "Season");
    mirror_target.id = "movie-focused".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        fetched_rows: 0,
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![
            crate::app::tests::make_item("Season A", "Season"),
            mirror_target,
        ],
        total_count: 2,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    model
}

/// Build a three-level stack: Series parent -> Seasons child -> Episodes
/// grandchild, so a single `go_back` from Episodes must auto-skip the
/// Season level and still restore the Series cursor by `parent_id`.
/// Build a three-level stack: Series parent (three rows) -> Seasons child
/// -> Episodes grandchild, where the Season level's `parent_id` targets a
/// discriminating nonzero parent row and the Episodes level's items
/// contain the component's selected id so the mirror mutates it. A single
/// `go_back` from Episodes must auto-skip the Season level and still
/// restore the Series cursor by `parent_id`.
fn tv_season_skip_model() -> Model {
    let mut model = mounted_tv_model();
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    model.app.libs[0].nav_stack[0].set_resting_cursor(0);
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        fetched_rows: 0,
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![season],
        total_count: 1,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    let mut mirror_target = crate::app::tests::make_item("E", "Episode");
    mirror_target.id = "movie-focused".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        fetched_rows: 0,
        parent_id: "season-1".into(),
        title: "Episodes".into(),
        items: vec![
            crate::app::tests::make_item("Episode 1", "Episode"),
            mirror_target,
        ],
        total_count: 2,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    model
}

mod activation;
