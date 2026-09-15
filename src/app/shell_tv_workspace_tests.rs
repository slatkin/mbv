use super::*;
use crate::app::components::{Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::make_movie_app;
use crate::app::types_browse::BrowseResting;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

#[path = "shell_tv_workspace_group_tests.rs"]
mod group_tests;

#[path = "shell_tv_workspace_selection_tests.rs"]
mod selection_tests;

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

#[test]
fn push_tv_workspace_projects_uncached_and_cached_series_image_state() {
    let mut model = mounted_tv_model();
    model.app.image_protocol_enabled = true;
    model.push_tv_workspace_content();
    let paint = model.test_paint_library_panel(Rect::new(0, 0, 100, 30));
    assert!(paint.is_none());

    model.app.card_image_states.insert(
        crate::app::images::series_image_cache_key(
            "movie-focused",
            crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES,
        ),
        crate::app::images::CachedImage::empty(),
    );
    model.push_tv_workspace_content();
    let paint = model.test_paint_library_panel(Rect::new(0, 0, 100, 30));
    assert!(paint.is_none());
}

#[test]
fn push_tv_workspace_projects_ready_narrow_series_image_paint() {
    use crate::app::images::series_image_cache_key;
    use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;

    // Narrow TV: width 70 is below `MINI_VIEW_THRESHOLD`, so the mini view
    // decides the Panel mode and the library owns it here; the size is set
    // before `Model::new` so the sync pass's resize handling (task 1.2) never
    // sees a drift and clears the planted image state.
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    app.mini_view_focus = crate::app::PanelFocus::Library;
    app.terminal_width = 70;
    app.terminal_height = 30;
    app.image_protocol_enabled = true;
    let mut model = Model::new(app);

    // Height 30 admits the shared inline hero's 14-row detail block (the
    // panel's list area is `terminal height - 10`; the deleted component's
    // tests painted the whole frame), so the block whose image box the paint
    // seam then retains is re-derived, not loosened.
    {
        let backend = TestBackend::new(70, 30);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
    model.sync_mounted_surfaces();
    // The first projection issued the Series fetch (resolved synchronously by
    // the fixture); the drain clears the key's reservation, then the planted
    // entry is what the ready state reads.
    let _ = model.drain_card_image_completions();
    let expected_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    let entry = model
        .app
        .build_cached_image(&expected_key, Some(image::DynamicImage::new_rgb8(64, 64)));
    model
        .app
        .card_image_states
        .insert(expected_key.clone(), entry);
    model.sync_mounted_surfaces();

    let area = model
        .library_panel_content_area()
        .expect("library placement after the frame publishes root_frame");
    let paint = model
        .test_paint_library_panel(area)
        .expect("ready projected Narrow hero must retain image paint");
    assert_eq!(paint.cache_key, expected_key);
    assert!(paint.area.width > 0 && paint.area.height > 0);
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
fn push_tv_workspace_prefetch_warms_the_painted_series_key() {
    use crate::app::images::series_image_cache_key;
    use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;

    let mut model = mounted_tv_model();
    model.app.image_protocol_enabled = true;
    // The sync pass is the production projection seam (task 5.10's central
    // hero projection owns the fetch for every migrated owner, TV included
    // since task 8.4); one throwaway draw publishes the `RootFrame` placement
    // it reads.
    {
        let backend = TestBackend::new(160, 40);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
    model.sync_mounted_surfaces();

    let expected_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    assert!(
        model.app.card_image_loading.contains(&expected_key),
        "prefetch must reserve the painted key: {expected_key}"
    );
    let loading = model.app.card_image_loading.clone();
    let active = model.app.image_fetches_active;
    let pending = model.app.pending_image_fetches.len();

    let paint = model.test_paint_library_panel(Rect::new(0, 0, 100, 30));
    assert!(paint.is_none(), "loading projection paints no pixels yet");
    assert_eq!(
        model.app.card_image_loading, loading,
        "painting must leave the prefetch reservation untouched: {expected_key}"
    );
    assert_eq!(model.app.image_fetches_active, active);
    assert_eq!(model.app.pending_image_fetches.len(), pending);
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

#[test]
fn push_tv_workspace_content_projects_selected_series_on_mount() {
    let model = mounted_tv_model();
    assert_eq!(
        model.test_tv_owner().selected_item_id(),
        Some("movie-focused".into())
    );
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

    // Move the component cursor to row 1 (movie-second) and Enter to the
    // Episodes pane: both are non-default state that a remount would reset.
    let move_request = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        move_request,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    let selected_id = |model: &mut Model| model.test_tv_owner().selected_item_id();
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
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
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
#[test]
fn tv_breakpoint_resize_round_trip_keeps_selected_series() {
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::{PanelFocus, PanelMode};

    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    // Wide/narrow is driven synchronously by terminal size now
    // (`prime_wide_tv_geometry`, the narrow→wide flash fix).
    let widen = |model: &mut Model, wide: bool| {
        model.app.terminal_width = if wide { 160 } else { 80 };
    };
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);

    // Wide: move the TV workspace selection to row 1 (movie-second).
    model.sync_mounted_surfaces();
    let moved = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        &moved,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    if let Some(Msg::Shell(request)) = moved {
        model.handle_tv_request(request);
    }
    model.sync_active_destination();
    let mut initial_wide_terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
    initial_wide_terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    model.sync_mounted_surfaces();

    let wide_target = model
        .test_tv_owner()
        .selected_item_id()
        .expect("wide TV workspace has a selected series");
    assert_eq!(wide_target, "movie-second");

    // Flip to narrow: the same owner stays mounted and focused, and its
    // shared carrier keeps the same selected target across the
    // presentation switch.
    widen(&mut model, false);
    model.sync_mounted_surfaces();
    assert!(model.library_panel_has_owner(&model.test_tv_owner_key()));
    let mut narrow_terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
    narrow_terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let narrow_target = model
        .test_tv_owner()
        .selected_item_id()
        .expect("narrow TV workspace has a selected series");
    assert_eq!(
        narrow_target, wide_target,
        "wide→narrow flip must preserve the selected series target"
    );

    // Narrow: move the selection back to row 0 (movie-focused).
    let up = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Up,
        modifiers: KeyModifiers::NONE,
    });
    let Some(Msg::Shell(request)) = up else {
        panic!("narrow Up must emit a typed shell request");
    };
    model.handle_emby_library_request(request);
    let narrow_return_target = model
        .test_tv_owner()
        .selected_item_id()
        .expect("narrow TV workspace has a selected series after move");
    assert_eq!(narrow_return_target, "movie-focused");

    // Flip back to wide: the same owner keeps the series selected while
    // narrow.
    widen(&mut model, true);
    model.sync_mounted_surfaces();
    assert!(model.library_panel_has_owner(&model.test_tv_owner_key()));
    let final_wide_target = model
        .test_tv_owner()
        .selected_item_id()
        .expect("wide TV workspace has a selected series after return");
    assert_eq!(
        final_wide_target, narrow_return_target,
        "narrow→wide flip must preserve the selected series target"
    );
}

/// Entering a wide TV library must route straight to the TV owner
/// on the *first* `sync_mounted_surfaces()` after the tab flips — no
/// one-frame narrow `BrowserComponent` flash. `App::wide_tv_library_area`
/// alone is a previous-frame paint signal; `prime_wide_tv_geometry` publishes
/// the wide geometry synchronously from terminal size so the mount gate is
/// correct immediately.
#[test]
fn entering_wide_tv_library_does_not_flash_the_narrow_browser() {
    use crate::app::{PanelFocus, PanelMode};

    let mut app = make_movie_app();
    // Second library is the wide TV one; start focused on the first (Movies).
    let mut tv_app = make_movie_app();
    let mut tv = tv_app.libs.remove(0);
    tv.library.collection_type = "tvshows".into();
    tv.library.id = "tv-library".into();
    for item in &mut tv.nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.libs.push(tv);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.tab = TabSelection::EmbyLibrary(0);
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert!(
        !model.library_panel_has_owner(&model.test_tv_owner_key_at(0)),
        "Movies tab: no TV owner"
    );

    // Flip to the wide TV library — the very next sync must land on the
    // panel-hosted TV owner, never the narrow browser.
    model.app.tab = TabSelection::EmbyLibrary(1);
    model.sync_mounted_surfaces();

    assert!(
        model.library_panel_has_owner(&model.test_tv_owner_key()),
        "the wide TV owner installs on the first sync after entry"
    );
    assert_eq!(model.application.focus(), Some(&ComponentId::Library));
}

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
        music_grouping: None,
    });
    let mut mirror_target = crate::app::tests::make_item("E", "Episode");
    mirror_target.id = "movie-focused".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
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
        music_grouping: None,
    });
    model
}

#[path = "shell_tv_workspace_activation_tests.rs"]
mod activation_tests;
