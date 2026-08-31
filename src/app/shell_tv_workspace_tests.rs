use super::*;
use crate::app::components::{
    browser_narrow::NarrowInlineHero, Msg, ShellRequest, TvWorkspaceComponent,
};
use crate::app::render::make_movie_app;
use crate::app::types_browse::BrowseResting;
use ratatui::layout::Rect;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[path = "shell_tv_workspace_group_tests.rs"]
mod group_tests;

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
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
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
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
        "movie-focused:ser_primary".into(),
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

    // Narrow: the mount gate (is_wide_tv_active) returns None, so the
    // pointer is cleared but the component stays mounted (keep-mounted).
    model.app.layout.main.tv_wide_right_area = Rect::default();
    model.sync_tv_workspace();
    assert_eq!(model.tv_workspace_id, None);
    assert!(
        model.application.mounted(&id),
        "the TV workspace must stay mounted across the narrow resize"
    );

    // Wide again: the same component is re-pointed, not remounted.
    model.app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
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
    let wide = Rect::new(40, 0, 60, 20);
    app.layout.main.tv_wide_right_area = wide;
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

    // Flip to narrow: the pointer moves to the BrowserComponent, which must
    // adopt the series the wide workspace had selected (row 1).
    model.app.layout.main.tv_wide_right_area = Rect::default();
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

    // Flip back to wide: the kept-mounted workspace must re-anchor to the
    // resting position the narrow browser left (row 0), not its stale
    // local cursor (row 1).
    model.app.layout.main.tv_wide_right_area = wide;
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
}

#[test]
fn typed_tv_requests_keep_component_cursor_authoritative() {
    // Each cursor-moving request is driven from a *fresh* mount so no
    // action's side effects can leak into the next: a chained sequence
    // (Down, End, ']', Right) would let a later assertion pass because of
    // the preceding action's state (e.g. End after Down both land on the
    // last row, and ']' clears the pill-cycled list). A fresh mount
    // guarantees component cursor 0, pane Series, and App browse cursor 0
    // before every key, isolating exactly one request per model.
    fn drive(code: Key) -> (Model, ShellRequest) {
        let mut model = mounted_tv_model();
        // Letter pills need a captured total for TvCycleLetterPill to run.
        model.app.libs[0].library_total = Some(1000);
        let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
        let request = model
            .application
            .get_component_mut(&id)
            .expect("TV workspace component mounted")
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(request)) = request else {
            panic!("TV key {code:?} must produce a typed shell request");
        };
        (model, request)
    }

    // TvMoveRows (Down): the component cursor moves 0 -> 1 (movie-second)
    // and the emitted request carries rows: 1; App's browse cursor stays 0
    // — the removed mirror's former effect would have written 1 here.
    let (mut model, request) = drive(Key::Down);
    assert!(matches!(request, ShellRequest::TvMoveRows { rows: 1 }));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "TvMoveRows must not write the component cursor into App's browse level"
    );
    let selected_id = model
        .application
        .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_item_id);
    assert_eq!(
        selected_id,
        Some("movie-second".into()),
        "the component cursor must have moved while App's cursor stayed put"
    );

    // TvJumpCursor (End): fresh mount again — the component jumps to the
    // last row; the request carries to_end: true (distinct from Home's
    // to_end: false); App's browse cursor still stays 0.
    let (mut model, request) = drive(Key::End);
    assert!(matches!(
        request,
        ShellRequest::TvJumpCursor { to_end: true }
    ));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "TvJumpCursor must not write the component cursor into App's browse level"
    );
    let selected_id = model
        .application
        .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_item_id);
    assert_eq!(
        selected_id,
        Some("movie-second".into()),
        "the component cursor must have jumped while App's cursor stayed put"
    );

    // TvCycleLetterPill (']' in the Series pane): fresh mount with a
    // captured total — the pill advances the letter filter; the request
    // carries delta: 1 (distinct from '[''s delta: -1); App's browse
    // cursor stays 0 (select_letter_pill's own reset is not a mirror).
    let (mut model, request) = drive(Key::Char(']'));
    assert!(matches!(
        request,
        ShellRequest::TvCycleLetterPill { delta: 1 }
    ));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "TvCycleLetterPill must not write the component cursor into App's browse level"
    );

    // TvMoveColumn (Right): fresh mount again — the pane moves to
    // Episodes; the request carries delta: 1 (distinct from Left's
    // delta: -1); App's browse cursor still stays 0.
    let (mut model, request) = drive(Key::Right);
    assert!(matches!(request, ShellRequest::TvMoveColumn { delta: 1 }));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "TvMoveColumn must not write the component cursor into App's browse level"
    );
}

#[test]
fn tv_series_enter_carries_the_component_selected_item() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    // Park the App browse cursor somewhere other than the component's
    // selection: the emitted TvActivate must carry the component's own
    // selected Series, not the (mirrored) App cursor's item.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let request = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(ShellRequest::TvActivate { item })) = request else {
        panic!("series Enter must emit TvActivate carrying the selected item");
    };
    assert_eq!(
        item.id, "movie-focused",
        "TvActivate must carry the component's selected Series, not the stale App cursor"
    );
    assert_eq!(item.item_type, "Series");
}

#[test]
fn push_tv_workspace_content_uses_component_selection_over_stale_app_cursor() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    // Seed detail for the second series so the pushed snapshot's target is
    // observable via the component's selected_series_snapshot().
    model.app.series_detail_cache.insert(
        "movie-second".into(),
        crate::app::SeriesDetail {
            seasons: vec![],
            episodes: std::collections::HashMap::new(),
        },
    );

    // Component-local selection: move the component cursor onto the second
    // series (index 1) while the App browse cursor stays at 0 — the
    // divergence the removed mirror used to hide.
    let moved = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        moved,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "App browse cursor must stay stale (no mirror)"
    );

    // The push must derive the Series snapshot from the component's
    // authoritative selection, not the stale App cursor.
    model.push_tv_workspace_content();
    let pushed = model
        .application
        .get_component(&id)
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_series_snapshot)
        .map(|item| item.id.clone());
    assert_eq!(
        pushed,
        Some("movie-second".into()),
        "pushed TV detail must follow the component selection, not the stale App cursor"
    );
}

#[test]
fn tv_season_move_fetches_uncached_episodes_for_component_selection() {
    let mut model = mounted_tv_model();
    let mut season_one = crate::app::tests::make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = crate::app::tests::make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![]);
    model.app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    let enter = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(request)) = enter else {
        panic!("series Enter must produce a typed request");
    };
    model.handle_tv_request(request);

    // Diverge the legacy App cursor: the component's selected series remains authoritative.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let season = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        season,
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));

    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model.handle_tv_request(ShellRequest::TvSeasonMove { delta: 1 });

    assert!(model
        .app
        .series_season_loading
        .contains(&("movie-focused".into(), "season-2".into())));
    assert!(model.app.series_detail_loading.contains("movie-focused"));
}

#[test]
fn tv_episode_activation_uses_component_cursors_and_cached_season_id() {
    let mut model = mounted_tv_model();
    let mut season_one = crate::app::tests::make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = crate::app::tests::make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let mut episode = crate::app::tests::make_item("Episode 2", "Episode");
    episode.id = "episode-2".into();
    episode.series_id = "movie-focused".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-2".into(), vec![episode]);
    model.app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    let enter_series = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(enter_series)) = enter_series else {
        panic!("series Enter must produce a typed request");
    };
    model.handle_tv_request(enter_series);

    let season = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        season,
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    // Make the App library cursor stale after the component has selected
    // the series; episode activation must not consult that cursor.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);

    let episode_request = model
        .application
        .get_component(&id)
        .expect("TV workspace component mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .episode_activation_selection();
    assert_eq!(episode_request, Some(("movie-focused".into(), 1, 0)));
    model.handle_tv_request(ShellRequest::TvEpisodeActivate);
    assert!(model.app.play_tv_episode("movie-focused", 1, 0));
    assert!(!model.app.play_tv_episode("movie-focused", 0, 0));
    assert!(!model.app.play_tv_episode("movie-focused", 1, 1));
    assert!(!model.app.play_tv_episode("missing-series", 1, 0));

    // TvBack after activation must restore the parent series-list cursor
    // via go_back's own parent_id lookup — not via any mirror. The stale
    // App cursor (1, "movie-second") diverges from the component's
    // selection ("movie-focused" at row 0). Append a third series so the
    // child's parent_id can target a *discriminating nonzero row*: the
    // seasons child's parent_id "movie-third" restores the series cursor
    // to row 2, so a reset-to-0 implementation (0), a child-cursor
    // implementation (99), and a stale-mirror implementation (1) all
    // fail.
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        1,
        "the stale App cursor must still diverge before TvBack"
    );
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![],
        total_count: 0,
        resting: BrowseResting::new(99, 0),
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    model.handle_tv_request(ShellRequest::TvBack);
    assert_eq!(
        model.app.libs[0].nav_stack.len(),
        1,
        "TvBack must pop the seasons child level"
    );
    assert_eq!(
            model.app.libs[0].nav_stack[0].resting().cursor(), 2,
            "TvBack restores the series cursor by parent_id (row of movie-third), not a reset 0, the popped child cursor 99, or the stale 1"
        );
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
    model.app.layout.main.tv_wide_right_area = Rect::default();
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
    model.app.layout.main.tv_wide_right_area = ratatui::layout::Rect::default();
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
