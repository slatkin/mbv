use super::*;
use crate::app::components::msg::TvHit;

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
        let Some(Msg::Shell(shell_boxed)) = request else {
            panic!("TV key {code:?} must produce a typed shell request");
        };
        (model, *shell_boxed)
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
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand {
           target: crate::app::components::tv_tree_target::TvTreeTarget::Show(_)
       })));
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 0);
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
    model.handle_tv_request(*enter_series);

    let season = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        season,
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvSeasonMove { delta: 1 })));
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
        model.app.libs[0].nav_stack[0].resting().cursor(),
        2,
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
            .unwrap()
    };
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
