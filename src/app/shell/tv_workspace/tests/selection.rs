use super::*;
use crate::app::components::msg::TvHit;
use rstest::rstest;

#[rstest]
#[case::small_forward(
    300,
    mbv_core::config::TvContentMode::All,
    1,
    mbv_core::config::TvContentMode::Latest
)]
#[case::small_backward(300, mbv_core::config::TvContentMode::Latest, -1, mbv_core::config::TvContentMode::All)]
#[case::large_forward(
    301,
    mbv_core::config::TvContentMode::Range(2),
    1,
    mbv_core::config::TvContentMode::Latest
)]
#[case::large_backward(301, mbv_core::config::TvContentMode::Latest, -1, mbv_core::config::TvContentMode::Range(2))]
fn tv_mode_cycle_wraps_over_the_painted_row(
    #[case] library_total: usize,
    #[case] current: mbv_core::config::TvContentMode,
    #[case] delta: i64,
    #[case] expected: mbv_core::config::TvContentMode,
) {
    let mut model = mounted_tv_model();
    model.app.libs[0].library_total = Some(library_total);
    model.app.libs[0].nav_stack[0].tv_content_mode = Some(current.clone());
    model.app.libs[0].tv_content_mode = Some(current);

    model.app.cycle_letter_pill(0, delta);

    assert_eq!(
        model.app.libs[0].nav_stack[0].tv_content_mode,
        Some(expected.clone())
    );
    assert_eq!(model.app.libs[0].tv_content_mode, Some(expected));
}

#[rstest]
#[case::small_latest(300, 0, mbv_core::config::TvContentMode::Latest)]
#[case::small_upcoming(300, 1, mbv_core::config::TvContentMode::Upcoming)]
#[case::small_all(300, 2, mbv_core::config::TvContentMode::All)]
#[case::large_latest(301, 0, mbv_core::config::TvContentMode::Latest)]
#[case::large_upcoming(301, 1, mbv_core::config::TvContentMode::Upcoming)]
#[case::large_ai(301, 2, mbv_core::config::TvContentMode::Range(0))]
#[case::large_jr(301, 3, mbv_core::config::TvContentMode::Range(1))]
#[case::large_sz(301, 4, mbv_core::config::TvContentMode::Range(2))]
fn tv_mouse_selection_selects_each_painted_mode(
    #[case] library_total: usize,
    #[case] pill_index: usize,
    #[case] expected: mbv_core::config::TvContentMode,
) {
    let mut model = mounted_tv_model();
    model.app.libs[0].library_total = Some(library_total);
    model.app.libs[0].nav_stack[0].tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);
    model.app.libs[0].tv_content_mode = Some(mbv_core::config::TvContentMode::Latest);

    model
        .app
        .handle_mouse_single_click_tv(0, TvHit::LetterPill(pill_index));

    assert_eq!(
        model.app.libs[0].nav_stack[0].tv_content_mode,
        Some(expected.clone())
    );
    assert_eq!(model.app.libs[0].tv_content_mode, Some(expected));
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
        let request = model.test_tv_owner_mut().test_key(&KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        });
        let Some(Msg::Shell(request)) = request else {
            panic!("TV key {code:?} must produce a typed shell request");
        };
        (model, request)
    }

    // Tree navigation resolves the selected stable target. Moving from the
    // current show to movie-second updates the existing shell-owned browse
    // selection through its typed row-click request; it never copies a flat
    // carrier cursor or numeric index.
    let (mut model, request) = drive(Key::Down);
    assert!(matches!(
        request,
        ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target)
        } if target == "movie-second"
    ));
    model
        .app
        .handle_mouse_single_click_tv(0, TvHit::SeriesRow("movie-second".into()));
    model.push_tv_workspace_content();
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 1);
    assert_eq!(
        model
            .test_tv_owner()
            .selected_tree_show()
            .map(|item| item.id),
        Some("movie-second".into())
    );

    // End also resolves through the tree's stable target and sends the
    // selected show, not a cursor delta.
    let (mut model, request) = drive(Key::End);
    assert!(matches!(
        request,
        ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target)
        } if target == "movie-second"
    ));
    model
        .app
        .handle_mouse_single_click_tv(0, TvHit::SeriesRow("movie-second".into()));
    model.push_tv_workspace_content();
    assert_eq!(
        model
            .test_tv_owner()
            .selected_tree_show()
            .map(|item| item.id),
        Some("movie-second".into())
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

    // Right expands the selected show through the existing lazy-load request;
    // it does not move focus to the Wide Workspace.
    let mut model = mounted_tv_model();
    model.app.libs[0].library_total = Some(1000);
    let request = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Right,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        request,
        Some(Msg::Shell(ShellRequest::TvTreeExpand {
            target: crate::app::components::tv_tree_target::TvTreeTarget::Show(_)
        }))
    ));
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 0);
}

#[test]
fn tv_series_enter_carries_the_component_selected_item() {
    let mut model = mounted_tv_model();

    // Park the App browse cursor somewhere other than the component's
    // selection: the emitted TvActivate must carry the component's own
    // selected Series, not the (mirrored) App cursor's item.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let request = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
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
    let moved = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        moved,
        Some(Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target)
        })) if target == "movie-second"
    ));
    model
        .app
        .handle_mouse_single_click_tv(0, TvHit::SeriesRow("movie-second".into()));

    // The stable tree selection is translated through the existing shell
    // request, so the pushed detail follows the selected show.
    model.push_tv_workspace_content();
    let pushed = model
        .test_tv_owner()
        .selected_series_snapshot()
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

    let enter = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    let Some(Msg::Shell(request)) = enter else {
        panic!("series Enter must produce a typed request");
    };
    model.handle_tv_request(request);

    // Diverge the legacy App cursor: the component's selected series remains authoritative.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let season = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    });
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

    let enter_series = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    let Some(Msg::Shell(enter_series)) = enter_series else {
        panic!("series Enter must produce a typed request");
    };
    model.handle_tv_request(enter_series);

    let season = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        season,
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    // Make the App library cursor stale after the component has selected
    // the series; episode activation must not consult that cursor.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);

    let episode = model
        .test_tv_owner()
        .selected_episode_item()
        .expect("component-selected episode");
    assert_eq!(episode.id, "episode-2");
    model.handle_tv_request(ShellRequest::TvEpisodeActivate { episode });

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
        fetched_rows: 0,
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
        tv_content_mode: None,
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

/// Renders the wide TV workspace through the library panel's paint path and
/// returns the painted buffer with the panel-retained right series-rail rect.
fn render_wide_tv(model: &mut Model) -> (ratatui::buffer::Buffer, Rect) {
    let area = model
        .app
        .wide_tv_library_area(0)
        .expect("wide TV area must be derived from the current frame");
    assert!(area.width > 0 && area.height > 0);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 40)).unwrap();
    {
        let panel = model
            .application
            .get_component_mut(&ComponentId::Library)
            .expect("library panel mounted")
            .as_any_mut()
            .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
            .expect("LibraryPanel");
        terminal
            .draw(|f| tuirealm::component::Component::view(panel, f, area))
            .unwrap();
    }
    let rail = model
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
        .test_wide_geometry()
        .expect("Wide skeleton geometry")
        .list_area;
    (terminal.backend().buffer().clone(), rail)
}

/// Whether any marker glyph (U+258E) is painted anywhere in the right series
/// rail band. This is a negative guard — it must never appear in either focus
/// state; the selected row's background alone marks selection.
fn rail_has_marker_glyph(buf: &ratatui::buffer::Buffer, rail: Rect) -> bool {
    (rail.y..rail.bottom()).any(|y| {
        (rail.x.saturating_sub(4)..rail.right()).any(|x| buf[(x, y)].symbol() == "\u{258e}")
    })
}

fn tv_selected_id(model: &Model) -> Option<String> {
    model.test_tv_owner().selected_item_id()
}

/// Through the real shell synchronisation order: moving Panel focus to Queue
/// drops the wide TV right rail's focused surface and selected-row marker on
/// the next frame, without losing the selected series identity.
#[test]
fn wide_tv_focus_to_queue_drops_right_rail_treatment_via_shell_sync() {
    let mut model = mounted_tv_model();
    model.sync_mounted_surfaces();
    assert_eq!(model.application.focus(), Some(&ComponentId::Library));
    let selected = tv_selected_id(&model);
    assert!(selected.is_some(), "a series row must be selected");

    let (focused_buf, rail) = render_wide_tv(&mut model);
    assert_eq!(
        focused_buf[(rail.x.saturating_sub(1), rail.y.saturating_sub(1))].bg,
        crate::app::palette::surface_colors(crate::app::palette::Surface::LibraryPanel, true).fill,
        "focused right rail paints the focused Library panel surface"
    );
    assert!(
        !rail_has_marker_glyph(&focused_buf, rail),
        "focused right rail paints no marker glyph"
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
        !rail_has_marker_glyph(&blurred_buf, rail),
        "blurred right rail paints no marker glyph"
    );

    assert_eq!(
        tv_selected_id(&model),
        selected,
        "selected series identity survives the focus change"
    );
}
