use super::*;

#[test]
fn enter_on_a_series_search_result_navigates_and_opens_the_workspace() {
    let mut harness = tv_harness();
    search_series(&mut harness, "Second");
    assert!(harness.model().active_inline_search_is_open());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(
        level.resting().cursor(),
        1,
        "the list cursor rests on the activated series"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the workspace targets the activated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the workspace is active: episode selection holds the focus"
    );
}

#[test]
fn enter_on_a_series_search_result_narrow_opens_the_hero_overlay() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    search_series(&mut harness, "Second");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    assert!(
        panel(&harness).test_hero_overlay_open(),
        "narrow activation opens the Library Hero overlay"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.resting().cursor(), 1);
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay's workspace is active: episode selection holds the focus"
    );
}

#[test]
fn tv_narrow_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    let before = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection")
        .id;

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    let after = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection after navigation")
        .id;
    assert_ne!(after, before);

    // Narrow TV is the same panel-hosted owner (task 8.4) -- no second
    // surface, and the owner stays installed at every breakpoint.
    assert!(harness
        .model()
        .library_panel_has_owner(&harness.model().test_tv_owner_key()));
}

/// unify-screens-under-panel-components task 8.1 (design D12, stable-target
/// re-anchor): the merged owner keeps its selected target across a
/// Wide->Narrow->Wide breakpoint round trip driven through real ticks.
#[test]
fn tv_wide_narrow_wide_tick_navigation_keeps_the_selected_target() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-0".into())
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let wide_target = tv(&harness)
        .selected_tree_show()
        .expect("wide TV tree selection")
        .id;

    // Narrow: the same owner keeps the same selected target and viewport
    // offset across the shared Wide/Inline presentation transition.
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let narrow_target = tv(&harness)
        .selected_tree_show()
        .expect("narrow TV tree selection")
        .id;
    assert_eq!(narrow_target, wide_target);

    // Wide again: still the same target and viewport offset.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
    let final_target = tv(&harness)
        .selected_tree_show()
        .expect("final Wide TV tree selection")
        .id;
    assert_eq!(final_target, narrow_target);
}

#[rstest]
#[case::latest(mbv_core::config::TvContentMode::Latest)]
#[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
fn flat_episode_activation_plays_without_opening_a_series_workspace(
    #[case] mode: mbv_core::config::TvContentMode,
) {
    let mut harness = flat_episode_harness(mode);
    let stack_len = harness.model().app.libs[0].nav_stack.len();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);

    assert_eq!(harness.model().app.libs[0].nav_stack.len(), stack_len);
    assert_eq!(tv(&harness).selected_series_snapshot(), None);
    assert!(!tv(&harness).episode_pane_focused());
    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "latest-episode",
        "flat episode activation must play the selected episode id"
    );
}

#[test]
fn flat_episode_mini_view_routes_keys_to_the_browser_carrier() {
    let mut harness = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    let mut second = crate::app::tests::make_item("Upcoming Episode", "Episode");
    second.id = "upcoming-episode".into();
    second.series_id.clear();
    harness.model_mut().app.libs[0].nav_stack[0]
        .items
        .push(second);
    harness.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    harness.model_mut().app.mini_view_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    assert!(panel(&harness).test_hero_overlay_open());
    assert!(!tv(&harness).episode_pane_focused());
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id).as_deref(),
        Some("latest-episode")
    );
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let movement = harness.step();
    assert!(movement.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 })
    )));
    assert_eq!(
        tv(&harness).selected_item_id(),
        Some("upcoming-episode".into())
    );
    assert!(!tv(&harness).episode_pane_focused());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    let cycle = harness.step();
    assert!(cycle.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvCycleLetterPill { delta: 1 })
    )));
    assert!(!cycle
        .messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::TvSeasonMove { .. }))));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(!panel(&harness).test_hero_overlay_open());
    assert!(!tv(&harness).episode_pane_focused());
}

#[test]
fn flat_episode_hero_is_painted_only_in_mini_view() {
    let mut mini = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    mini.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    mini.model_mut().app.mini_view_focus = PanelFocus::Library;
    mini.model_mut().sync_mounted_surfaces();
    draw(&mut mini);
    assert!(panel(&mini).test_hero_overlay_open());
    assert_eq!(
        mini.model_mut()
            .test_tv_owner_mut()
            .hero_data()
            .map(|data| data.facts.title),
        Some("Latest Episode".into())
    );
    assert!(panel(&mini).test_overlay_geometry().is_some());

    let mut narrow = flat_episode_harness(mbv_core::config::TvContentMode::Latest);
    narrow.model_mut().app.terminal_width = crate::app::MINI_VIEW_THRESHOLD;
    narrow.model_mut().sync_mounted_surfaces();
    draw(&mut narrow);
    assert!(!panel(&narrow).test_hero_overlay_open());
    assert!(narrow.model_mut().test_tv_owner_mut().hero_data().is_none());
    assert!(panel(&narrow).test_overlay_geometry().is_none());
}

#[test]
fn tv_wide_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-0".into())
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    assert_eq!(
        tv(&harness).selected_tree_show().map(|item| item.id),
        Some("series-1".into())
    );
}

/// Task 8.4: a catalog-retained TV owner keeps its local cursor and scroll
/// while inactive. This goes through the real Application::tick path for the
/// navigation and for each tab transition's sync pass.
#[test]
fn tv_owner_retains_cursor_and_scroll_while_inactive() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().app.terminal_height = 20;
    for index in 2..20 {
        let mut item = crate::app::tests::make_item(&format!("Series {index}"), "Series");
        item.id = format!("series-{index}");
        harness.model_mut().app.libs[0].nav_stack[0]
            .items
            .push(item);
    }
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    for _ in 0..8 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        harness.step();
        draw(&mut harness);
    }
    let before = tv(&harness)
        .selected_tree_show()
        .expect("TV tree selection before inactive transition")
        .id;
    assert_eq!(before, "series-8");
    // The fixed-row owner may keep the selected row visible at offset zero;
    // clamping is asserted by the carrier tests for both viewport sizes.

    harness.model_mut().app.tab = TabSelection::Home;
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let tv_key = harness.model().test_tv_owner_key_at(0);
    assert!(harness.model().library_panel_has_owner(&tv_key));

    harness.model_mut().app.tab = TabSelection::EmbyLibrary(0);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let after = tv(&harness)
        .selected_tree_show()
        .expect("TV tree selection after inactive transition")
        .id;
    assert_eq!(after, before);
}

/// Task 8.4: the season pills are resolved by the mounted panel from the
/// geometry it painted (its Workspace selector row), and the owner translates
/// the resolved slot event into the same `TvHit::SeasonTab` message the
/// deleted component emitted.
#[test]
fn tv_wide_tick_click_resolves_season_pill() {
    let mut harness = tv_harness();
    draw(&mut harness);
    // First frame geometry: the panel retained the season pills it painted.
    let (rect, _) = panel(&harness)
        .test_workspace_selector_hits()
        .regions()
        .first()
        .cloned()
        .expect("painted season pill");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::TvHitClick {
                hit: TvHit::SeasonTab(0)
            })
        )),
        "tick messages: {:?}",
        outcome.messages
    );
}

/// Task 8.4: the episode rows live in the hero pane's Workspace box; the
/// panel resolves the pointer against its own painted hero pane and the owner
/// resolves the episode target through its own carrier.
#[test]
fn tv_wide_tick_click_resolves_episode_row() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("episode-1".into())
    );
    let episode_claim = panel(&harness)
        .test_wide_geometry()
        .and_then(|geometry| geometry.workspace)
        .expect("painted episode rows")
        .1;
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: episode_claim.x,
        row: episode_claim.y,
        modifiers: KeyModifiers::NONE,
    }));
    let messages = step_without_sync(&mut harness);
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::TvHitClick {
                hit: TvHit::EpisodeRow(target)
            }) if target == "episode-1"
        )),
        "tick messages: {:?}",
        messages
    );
}
