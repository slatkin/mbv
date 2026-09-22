use super::*;

#[test]
fn tv_keyboard_uses_typed_requests_and_routes_brackets_by_pane() {
    let mut owner = TvContent::new();

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
    };
    let mut series_a = make_item("Series A", "Series");
    series_a.id = "series-a".into();
    let mut series_b = make_item("Series B", "Series");
    series_b.id = "series-b".into();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series_a, series_b], 0),
        None,
        Some(detail),
        0,
        None,
        true,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        owner.on_key(&key(Key::Down)),
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert!(matches!(
        owner.on_key(&key(Key::Char('['))),
        Some(Msg::Shell(ShellRequest::TvCycleLetterPill { delta: -1 }))
    ));
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvActivate { item }))
            if item.name == "Series B" && item.item_type == "Series"
    ));
    assert!(matches!(
        owner.on_key(&key(Key::Up)),
        Some(Msg::Shell(ShellRequest::TvEpisodeMove { delta: -1 }))
    ));
    assert!(matches!(
        owner.on_key(&key(Key::Char(']'))),
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    assert!(matches!(
        owner.on_key(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::TvBack))
    ));

    owner.on_key(&key(Key::Enter));
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvEpisodeActivate { .. }))
    ));
}

#[test]
fn dot_emits_library_context_menu() {
    let mut owner = TvContent::new();

    let series = make_item("Series", "Series");
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    assert!(matches!(
        owner.on_key(&KeyEvent { code: Key::Char('.'), modifiers: KeyModifiers::NONE }),
        Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(items), _))) if items.len() == 1 && items[0].name == "Series"
    ));
}

#[test]
fn slash_emits_open_inline_search() {
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    assert_eq!(
        owner.on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE
        }),
        Some(Msg::Shell(ShellRequest::OpenInlineSearch))
    );
}

/// The Wide panel paints exactly one shared Inline Search presentation in
/// the browser pane (design.md D3), replacing the ordinary letter-selector
/// row (task 8.4: the panel hosts the owner and its session).
#[test]
fn wide_tv_search_paints_in_browser_pane_not_hero_pane() {
    let mut owner = TvContent::new();
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series], 0),
        None,
        None,
        0,
        None,
        true,
    ));
    let mut panel = panel_with(owner, true);
    tv_mut(&mut panel).on_key(&KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    assert!(tv(&panel).inline_search().is_active());
    tv_mut(&mut panel)
        .inline_search_mut()
        .set_pool(SearchPool::Items(vec![make_item(
            "Search Result Alpha",
            "Series",
        )]));
    // An empty query shows no results; score one so the row paints.
    tv_mut(&mut panel)
        .inline_search_mut()
        .restore_query("Alpha".into());

    let terminal = paint(&mut panel, 100, 20);

    let list_area = panel.test_wide_geometry().unwrap().list_area;
    let geometry = panel.test_wide_geometry().unwrap();
    let browser_pane = geometry.browser;
    let hero_pane = geometry.hero;
    assert!(list_area.width > 0 && list_area.height > 0);
    assert!(
        list_area.x >= browser_pane.x
            && list_area.x + list_area.width <= browser_pane.x + browser_pane.width,
        "search paints inside the browser pane: list_area={list_area:?} browser={browser_pane:?}"
    );

    let buffer = terminal.backend().buffer();
    let rendered: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    assert_eq!(rendered.matches("SEARCH:").count(), 1);
    assert!(
        panel.test_selector_hits().regions().is_empty(),
        "selector hit regions are unavailable while search is active"
    );
    assert!(
        !rendered.contains("A–C"),
        "ordinary selector must be replaced"
    );
    let mut found_in_rail = false;
    let mut found_in_hero_pane = false;
    for y in list_area.y..list_area.y + list_area.height {
        let rail_row: String = (list_area.x..list_area.x + list_area.width)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if rail_row.contains("Search Result Alpha") {
            found_in_rail = true;
        }
        let hero_row: String = (hero_pane.x..hero_pane.x + hero_pane.width)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if hero_row.contains("Search Result Alpha") {
            found_in_hero_pane = true;
        }
    }
    assert!(
        found_in_rail,
        "search result row painted in the browser pane"
    );
    assert!(
        !found_in_hero_pane,
        "search result must not paint in the hero pane"
    );
}

/// A right click on an Inline Search result row moves the search cursor there
/// and asks the host to open its ordinary item-based context menu for that
/// result (P1: context-menu actions stay available while search is open).
#[test]
fn wide_tv_search_right_click_on_result_opens_context_menu() {
    let mut owner = TvContent::new();
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series], 0),
        None,
        None,
        0,
        None,
        true,
    ));
    let mut panel = panel_with(owner, true);
    tv_mut(&mut panel).on_key(&KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    tv_mut(&mut panel)
        .inline_search_mut()
        .set_pool(SearchPool::Items(vec![make_item(
            "Search Result Alpha",
            "Series",
        )]));
    // An empty query shows no results; score one so the row is a target.
    tv_mut(&mut panel)
        .inline_search_mut()
        .restore_query("Alpha".into());

    paint(&mut panel, 100, 20);
    let list_area = panel.test_wide_geometry().unwrap().list_area;

    let message = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Right),
        list_area.x,
        list_area.y,
    ));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(ref items), _)))
                if items.len() == 1 && items[0].name == "Search Result Alpha"
        ),
        "right click on a result row opens its context menu: {message:?}"
    );
}

#[test]
fn dot_with_episode_focus_targets_series() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode", "Episode");
    episode.id = "episode-id".into();
    episode.series_id = series.id.clone();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));
    owner.on_key(&KeyEvent {
        code: Key::Right,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        owner.on_key(&KeyEvent { code: Key::Char('.'), modifiers: KeyModifiers::NONE }),
        Some(Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Emby(items), _)))
            if items.len() == 1 && items[0].id == "series-id" && items[0].item_type == "Series"
    ));
}

#[test]
fn ctrl_r_emits_library_rescan() {
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        None,
        false,
    ));

    let message = owner.on_key(&KeyEvent {
        code: Key::Char('r'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(message, Some(Msg::Shell(ShellRequest::EmbyLibraryRescan)));
}

#[test]
fn ctrl_w_emits_library_toggle_watched() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-id".into();
    episode.series_id = series.id.clone();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail),
        0,
        None,
        false,
    ));

    // Enter moves focus into the Episodes pane; legacy library actions
    // still target the selected series-list item rather than the
    // highlighted episode.
    assert!(matches!(
        owner.on_key(&KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }),
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
    let message = owner.on_key(&KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::EmbyLibraryToggleWatched { item }))
            if item.id == "series-id" && item.item_type == "Series"
    ));
}
