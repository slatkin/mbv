use super::*;

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

/// Renders the wide TV workspace through the shell paint path and returns the
/// painted buffer with the component-owned right series-rail rect.
fn render_wide_tv(model: &mut Model) -> (ratatui::buffer::Buffer, Rect) {
    let area = model.app.layout.main.tv_wide_area;
    assert!(
        area.width > 0 && area.height > 0,
        "wide TV geometry must be primed by the sync pass: {area:?}"
    );
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 40)).unwrap();
    terminal
        .draw(|f| model.render_tv_workspace_component(f))
        .unwrap();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let rail = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .unwrap()
        .test_layout()
        .tv_wide_list_area;
    (terminal.backend().buffer().clone(), rail)
}

/// Whether the accent selected-row marker glyph is painted anywhere in the
/// right series rail band.
fn rail_has_selection_marker(buf: &ratatui::buffer::Buffer, rail: Rect) -> bool {
    (rail.y..rail.bottom()).any(|y| {
        (rail.x.saturating_sub(4)..rail.right()).any(|x| buf[(x, y)].symbol() == "\u{258e}")
    })
}

fn tv_selected_id(model: &Model, id: &ComponentId) -> Option<String> {
    model
        .application
        .get_component(id)
        .unwrap()
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .unwrap()
        .selected_item_id()
}

/// Through the real shell synchronisation order: moving Panel focus to Queue
/// drops the wide TV right rail's focused surface and selected-row marker on
/// the next frame, without losing the selected series identity.
#[test]
fn wide_tv_focus_to_queue_drops_right_rail_treatment_via_shell_sync() {
    let mut model = mounted_tv_model();
    model.sync_mounted_surfaces();
    let id = model
        .tv_workspace_id
        .clone()
        .expect("wide TV workspace mounted");
    assert_eq!(model.application.focus(), Some(&id));
    let selected = tv_selected_id(&model, &id);
    assert!(selected.is_some(), "a series row must be selected");

    let (focused_buf, rail) = render_wide_tv(&mut model);
    assert_eq!(
        focused_buf[(rail.x.saturating_sub(1), rail.y.saturating_sub(1))].bg,
        crate::app::palette::resolve_surface_focus(true),
        "focused right rail paints the focused surface"
    );
    assert!(
        rail_has_selection_marker(&focused_buf, rail),
        "focused right rail paints the selected-row marker"
    );

    // Panel focus moves to Queue via the production sync sequence.
    model.app.panel_focus = crate::app::PanelFocus::Queue;
    model.sync_mounted_surfaces();
    assert_eq!(model.application.focus(), Some(&ComponentId::Queue));

    let (blurred_buf, rail) = render_wide_tv(&mut model);
    assert_eq!(
        blurred_buf[(rail.x.saturating_sub(1), rail.y.saturating_sub(1))].bg,
        crate::app::palette::resolve_surface_focus(false),
        "blurred right rail drops the focused surface"
    );
    assert!(
        !rail_has_selection_marker(&blurred_buf, rail),
        "blurred right rail drops the selected-row marker"
    );

    assert_eq!(
        tv_selected_id(&model, &id),
        selected,
        "selected series identity survives the focus change"
    );
}
