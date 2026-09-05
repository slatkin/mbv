use super::*;
use crate::app::components::{
    browser_narrow::NarrowInlineHero, Msg, ShellRequest, TerminalObserverEvent,
    TvWorkspaceComponent,
};
use crate::app::render::make_movie_app;
use crate::app::types_browse::BrowseResting;
use ratatui::layout::Rect;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[path = "shell_tv_workspace_group_tests.rs"]
mod group_tests;

#[path = "shell_tv_workspace_selection_tests.rs"]
mod selection_tests;

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_library_area`), not this previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_workspace();
    model
}

#[test]
fn sync_projects_inline_search_visibility_before_tv_content() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].library_total = Some(1000);
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    let mut second_app = make_movie_app();
    let mut second = second_app.libs.remove(0);
    second.library.collection_type = "tvshows".into();
    second.library.id = "tv-library-b".into();
    second.library_total = Some(1000);
    for item in &mut second.nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.libs.push(second);
    // Wide breakpoint is now driven synchronously by terminal size
    // (`prime_wide_tv_geometry`), not a previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);

    // Simulate the stale projection left by a previous tick while A was active.
    let search_id =
        crate::app::components::ComponentId::InlineSearch(crate::app::components::BrowserKey {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: model.app.libs[0].library.id.clone(),
            kind: crate::app::components::BrowserKind::TvShows,
        });
    model
        .application
        .mount(
            search_id,
            Box::new(crate::app::components::InlineSearchComponent::new()),
            vec![],
        )
        .unwrap();
    model.app.inline_search_active = true;
    model.app.tab = TabSelection::EmbyLibrary(1);
    model.sync_mounted_surfaces();

    let id = model
        .tv_workspace_id
        .as_ref()
        .expect("TV workspace mounted");
    let component = model
        .application
        .get_component(id)
        .expect("TV workspace component mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component type");
    assert!(component.show_letter_pills());
    assert!(!model.app.inline_search_active);
}

#[test]
fn push_tv_workspace_projects_uncached_and_cached_series_image_state() {
    let mut model = mounted_tv_model();
    model.app.image_protocol_enabled = true;
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    model.push_tv_workspace_content();
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    assert!(matches!(
        component.take_image_paint(),
        Some(crate::app::render::HomeImagePaint::Series {
            show_placeholder: true,
            ..
        })
    ));

    model.app.card_image_states.insert(
        crate::app::images::series_image_cache_key(
            "movie-focused",
            crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES,
        ),
        crate::app::images::CachedImage::empty(),
    );
    model.push_tv_workspace_content();
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    assert!(matches!(
        component.take_image_paint(),
        Some(crate::app::render::HomeImagePaint::Series {
            show_placeholder: false,
            ..
        })
    ));
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
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    model.push_tv_workspace_content();

    let expected_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    assert!(
        model.app.card_image_loading.contains(&expected_key),
        "prefetch must reserve the painted key: {expected_key}"
    );
    let loading = model.app.card_image_loading.clone();
    let active = model.app.image_fetches_active;
    let pending = model.app.pending_image_fetches.len();

    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    let paint = {
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .unwrap();
        terminal.draw(|f| component.view(f, f.area())).unwrap();
        component.take_image_paint()
    };
    let Some(crate::app::render::HomeImagePaint::Series { image_types, .. }) = &paint else {
        panic!("expected a Series image request");
    };
    assert_eq!(
        *image_types, SERIES_LANDSCAPE_IMAGE_TYPES,
        "painter must declare the canonical chain"
    );

    terminal
        .draw(|f| model.app.paint_home_image(f, paint))
        .unwrap();

    assert!(
        model.app.card_image_loading.contains(&expected_key)
            || model.app.card_image_states.contains_key(&expected_key),
        "paint must keep the prefetched key reserved: {expected_key}"
    );
    assert_eq!(
        model.app.card_image_loading, loading,
        "paint must leave the prefetch's reservation set untouched: {expected_key}"
    );
    assert_eq!(
        model.app.image_fetches_active, active,
        "paint must start no additional fetch"
    );
    assert_eq!(
        model.app.pending_image_fetches.len(),
        pending,
        "paint must queue no additional fetch"
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

#[test]
fn push_tv_workspace_content_projects_selected_series_on_mount() {
    let model = mounted_tv_model();
    let id = model
        .tv_workspace_id
        .as_ref()
        .expect("TV workspace mounted");
    let component = model
        .application
        .get_component(id)
        .expect("TV workspace component mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component type");
    assert_eq!(component.selected_item_id(), Some("movie-focused".into()));
}

/// keep-destination-components-mounted task 3.1: the TV workspace stays
/// mounted across wide→narrow→wide layout resizes (keep-mounted, D1).
/// Because the component is never unmounted/remounted, its private
/// pane/cursor state survives the round trip: after re-point it is not
/// reset to the fresh-mount default (cursor 0 / Series pane).
#[test]
fn tv_workspace_stays_mounted_and_preserves_pane_cursor_across_resize() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let mounted_before = model.application.mounted(&id);
    assert!(mounted_before);

    // Move the component cursor to row 1 (movie-second) and Enter to the
    // Episodes pane: both are non-default state that a remount would reset.
    let move_request = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        move_request,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    let selected_id = |model: &mut Model| {
        model
            .application
            .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item_id)
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
    let enter = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        enter,
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
    assert_eq!(
        model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .unwrap()
            .episode_activation_selection(),
        Some(("movie-second".into(), 0, 0)),
        "Enter must put the component in the Episodes pane for its selected series"
    );

    // Narrow: the mount gate (wide_tv_library_area) returns None, so the
    // pointer is cleared but the component stays mounted (keep-mounted).
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.sync_tv_workspace();
    assert_eq!(model.tv_workspace_id, None);
    assert!(
        model.application.mounted(&id),
        "the TV workspace must stay mounted across the narrow resize"
    );

    // Wide again: the same component is re-pointed, not remounted.
    model.app.terminal_width = 160;
    model.app.terminal_height = 40;
    model.sync_tv_workspace();
    assert_eq!(
        model.tv_workspace_id.as_ref(),
        Some(&id),
        "re-point must restore the same component id"
    );
    assert!(model.application.mounted(&id));
    assert_eq!(
        selected_id(&mut model),
        Some("movie-second".into()),
        "the component cursor must survive the wide→narrow→wide round trip"
    );
    assert_eq!(
        model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .unwrap()
            .episode_activation_selection(),
        Some(("movie-second".into(), 0, 0)),
        "the Episodes pane must survive the wide→narrow→wide round trip"
    );
}

/// migrate-narrow-browse task 2.3 (D5): resizing across the wide TV
/// breakpoint and back keeps the visually-selected series. The
/// active-destination pointer flips between `TvWorkspaceComponent` (wide)
/// and the narrow `BrowserComponent`, each owning its own cursor; the
/// breakpoint hand-off carries the selection across both flips.
#[test]
fn tv_breakpoint_resize_round_trip_keeps_selected_series() {
    use crate::app::components::{BrowserComponent, Msg, ShellRequest};
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
    let tv_id = model.tv_workspace_id.clone().expect("wide TV workspace id");
    let moved = model
        .application
        .get_component_mut(&tv_id)
        .expect("TV workspace mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        moved,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));

    let (wide_anchor, wide_scroll) = model
        .application
        .get_component(&tv_id)
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .map(|component| {
            (
                component
                    .viewport_anchor(component.painted_viewport_height())
                    .expect("wide TV workspace has a selected series"),
                component.scroll(),
            )
        })
        .expect("TV workspace component");

    // Flip to narrow: the pointer moves to the BrowserComponent, which must
    // adopt the series the wide workspace had selected (row 1).
    widen(&mut model, false);
    model.sync_mounted_surfaces();
    let browser_id = model.emby_browser_id.clone().expect("narrow TV browser id");
    let browser_cursor = model
        .application
        .get_component(&browser_id)
        .and_then(|comp| comp.as_any().downcast_ref::<BrowserComponent>())
        .map(BrowserComponent::cursor);
    assert_eq!(
        browser_cursor,
        Some(1),
        "narrow browser must adopt the series selected in the wide workspace"
    );
    let (narrow_anchor, narrow_scroll) = model
        .application
        .get_component(&browser_id)
        .and_then(|component| component.as_any().downcast_ref::<BrowserComponent>())
        .map(|component| {
            (
                component
                    .viewport_anchor(component.painted_viewport_height())
                    .expect("narrow browser has a selected series"),
                component.scroll(),
            )
        })
        .expect("browser component");
    assert_eq!(
        narrow_anchor.selected_target, wide_anchor.selected_target,
        "wide→narrow hand-off must preserve the selected series target"
    );
    assert_eq!(
        narrow_anchor.selected_row_offset, wide_anchor.selected_row_offset,
        "wide→narrow hand-off must preserve the selected row offset"
    );
    assert_eq!(
        narrow_scroll, wide_scroll,
        "wide→narrow hand-off must preserve the list scroll"
    );

    // Narrow: move the browser selection back to row 0 (movie-focused).
    let up = model
        .application
        .get_component_mut(&browser_id)
        .expect("narrow browser mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Up,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(request)) = up else {
        panic!("browser Up must emit a typed shell request");
    };
    model.handle_browser_request(request);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 0);
    let (narrow_return_anchor, narrow_return_scroll) = model
        .application
        .get_component(&browser_id)
        .and_then(|component| component.as_any().downcast_ref::<BrowserComponent>())
        .map(|component| {
            (
                component
                    .viewport_anchor(component.painted_viewport_height())
                    .expect("narrow browser has a selected series after move"),
                component.scroll(),
            )
        })
        .expect("browser component");

    // Flip back to wide: the kept-mounted workspace must re-anchor to the
    // resting position the narrow browser left (row 0), not its stale
    // local cursor (row 1).
    widen(&mut model, true);
    model.sync_mounted_surfaces();
    assert_eq!(model.tv_workspace_id.as_ref(), Some(&tv_id));
    let tv_cursor = model
        .application
        .get_component(&tv_id)
        .and_then(|comp| comp.as_any().downcast_ref::<TvWorkspaceComponent>())
        .map(TvWorkspaceComponent::cursor);
    assert_eq!(
        tv_cursor,
        Some(0),
        "wide workspace must re-anchor to the series selected while narrow"
    );
    let (final_wide_anchor, final_wide_scroll) = model
        .application
        .get_component(&tv_id)
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .map(|component| {
            (
                component
                    .viewport_anchor(component.painted_viewport_height())
                    .expect("wide TV workspace has a selected series after return"),
                component.scroll(),
            )
        })
        .expect("TV workspace component");
    assert_eq!(
        final_wide_anchor.selected_target, narrow_return_anchor.selected_target,
        "narrow→wide hand-off must preserve the selected series target"
    );
    assert_eq!(
        final_wide_anchor.selected_row_offset, narrow_return_anchor.selected_row_offset,
        "narrow→wide hand-off must preserve the selected row offset"
    );
    assert_eq!(
        final_wide_scroll, narrow_return_scroll,
        "narrow→wide hand-off must preserve the list scroll"
    );
}

/// Entering a wide TV library must route straight to `TvWorkspaceComponent`
/// on the *first* `sync_mounted_surfaces()` after the tab flips — no
/// one-frame narrow `BrowserComponent` flash. `App::wide_tv_library_area`
/// alone is a previous-frame paint signal; `prime_wide_tv_geometry` publishes
/// the wide geometry synchronously from terminal size so the mount gate is
/// correct immediately.
#[test]
fn entering_wide_tv_library_does_not_flash_the_narrow_browser() {
    use crate::app::PanelMode;

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
    assert_eq!(model.tv_workspace_id, None, "Movies tab: no TV workspace");

    // Flip to the wide TV library — the very next sync must land on the
    // workspace, never the narrow browser.
    model.app.tab = TabSelection::EmbyLibrary(1);
    model.sync_mounted_surfaces();

    let tv_id = model
        .tv_workspace_id
        .clone()
        .expect("wide TV workspace mounted on the first sync after entry");
    assert!(matches!(tv_id, ComponentId::TvWorkspace(_)));
    assert_eq!(
        model.emby_browser_id, None,
        "no narrow browser flash for the wide TV library"
    );
    assert_eq!(model.application.focus(), Some(&tv_id));
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

#[test]
fn wide_tv_handoff_does_not_fetch_empty_series_id() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].nav_stack[0].items[0].item_type = "Series".into();
    app.libs[0].nav_stack[0].items[0].id.clear();
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    let mut model = Model::new(app);
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    // This is the first wide-TV handoff, so the mounted component cannot have
    // already captured a valid ID before the guard is exercised.
    model.sync_tv_workspace();

    assert!(model.app.series_detail_loading.is_empty());
    assert!(model.app.series_detail_cache.is_empty());
}

#[test]
fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.sync_tv_workspace();
    model.sync_emby_browser();

    // Divergence: the mounted BrowserComponent selects index 0 while App's
    // mirrored BrowseLevel cursor is stale at index 1. Resolve narrow extras
    // using the component-owned cursor, as the production seam does.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let component_cursor = model
        .application
        .get_component(&model.emby_browser_id.clone().expect("browser mounted"))
        .expect("browser mounted")
        .as_any()
        .downcast_ref::<crate::app::components::BrowserComponent>()
        .expect("browser component")
        .cursor();
    assert_eq!(component_cursor, 0);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 1);

    let component_extras = model.app.narrow_browse_extras(0, component_cursor);
    match component_extras.inline_hero {
        Some(NarrowInlineHero::Series { item, .. }) => {
            assert_eq!(item.id, "movie-focused");
        }
        _ => panic!("component cursor must resolve the focused Series"),
    }
    let stale_extras = model
        .app
        .narrow_browse_extras(0, model.app.libs[0].nav_stack[0].resting().cursor());
    match stale_extras.inline_hero {
        Some(NarrowInlineHero::Series { item, .. }) => {
            assert_eq!(item.id, "movie-second");
        }
        _ => panic!("stale App cursor must resolve the other Series"),
    }

    model.app.libs[0].nav_stack[0].set_resting_cursor(component_cursor);

    // Wide TV layout => enter_series_selection targets the component's
    // Series (asserted by the resolved target, not merely the bool).
    let wide_target = model
        .app
        .selected_series_item(0, model.app.libs[0].nav_stack[0].resting().cursor())
        .expect("series");
    assert_eq!(wide_target.id, "movie-focused");
    assert!(model.app.activate_selected_series(0));

    // Narrow layout => open_series_selection_modal targets the same
    // Series, proven by the modal's Series source id.
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.app.libs[0].nav_stack[0].set_resting_cursor(0);
    model.app.activate_selected_series(0);
    match model.app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::SelectionModal(modal)) => {
            if let crate::app::types_selection_modal::SelectionModalSource::Series { series_id } =
                &modal.source
            {
                assert_eq!(
                    series_id.as_str(),
                    "movie-focused",
                    "narrow activation must target the component's selected Series"
                );
            } else {
                panic!("narrow activation must open a Series selection modal");
            }
        }
        _ => panic!("narrow layout must open the series selection modal"),
    }
    model.app.pending_overlay = None;

    // Guard 1: a non-tvshows collection_type rejects.
    model.app.libs[0].library.collection_type = "movies".into();
    assert!(!model.app.activate_selected_series(0));

    // Guard 2: a selected item that is not a Series rejects.
    model.app.libs[0].library.collection_type = "tvshows".into();
    model.app.libs[0].nav_stack[0].items[0].item_type = "Movie".into();
    assert!(!model.app.activate_selected_series(0));
}

/// replace-wide-paint-inference completion gate (6.3): `activate_selected_series`
/// gates on `App::wide_tv_library_area`, a paint-free predicate driven solely
/// by terminal size. Resizing narrow -> wide through the real
/// `Msg::TerminalEvent(Resize)` path must flip the activation branch on that
/// same tick, before any repaint refreshes `tv_wide_left_area`/
/// `tv_wide_right_area`.
#[test]
fn tv_series_activation_branch_flips_on_resize_tick_before_repaint() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    model.sync_tv_workspace();
    assert!(model.app.wide_tv_library_area(0).is_none());

    // Narrow: activation opens the Series selection modal, never the
    // persistent workspace fetch.
    assert!(model.app.activate_selected_series(0));
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(crate::app::types_overlay::OverlayRequest::SelectionModal(_))
        ),
        "narrow activation must open the series selection modal"
    );
    model.app.pending_overlay = None;

    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        Msg::TerminalEvent(TerminalObserverEvent::Resize {
            width: 160,
            height: 40,
        }),
        None,
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 160);
    assert!(model.app.wide_tv_library_area(0).is_some());

    // Wide, on this same tick: activation must enter the persistent
    // workspace, not the narrow modal -- with no intervening repaint.
    assert!(model.app.activate_selected_series(0));
    assert!(
        model.app.pending_overlay.is_none(),
        "wide activation right after the resize tick must not open the series modal"
    );
}
