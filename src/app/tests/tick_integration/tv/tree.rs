use super::*;

fn tv_tree_geometry(width: u16, mini: bool) -> TickHarness {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = width;
    if mini {
        harness.model_mut().app.mini_view_focus = PanelFocus::Library;
    }
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    harness
}

fn tv_tree_text_position(harness: &mut TickHarness, text: &str) -> (u16, u16) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let buffer = terminal.backend().buffer();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        if let Some(x) = line.find(text) {
            return (x as u16, y);
        }
    }
    panic!("TV tree row {text:?} was not painted");
}

fn tick_tv_key(harness: &mut TickHarness, code: Key) -> Vec<Msg> {
    harness.inject(Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let messages = outcome.messages.clone();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    messages
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_keyboard_navigation_resolves_show_target_through_shell_sync(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into()))
    );

    let messages = tick_tv_key(&mut harness, Key::Down);
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
                hit: TvHit::SeriesRow(target)
            } if target == "series-1"))),
        "tick must resolve the selected tree target before shell dispatch: {messages:?}"
    );
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the shell sync pass must preserve the resolved stable tree target"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        1
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_show_activation_uses_the_selected_target_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    if width == 160 {
        assert!(
            outcome.messages.iter().any(|message| matches!(
                message,
                Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { item } if item.id == "series-0"))),
            "Wide Enter must carry the selected show's stable identity: {:?}",
            outcome.messages
        );
    } else {
        assert!(
            !outcome
                .messages
                .iter()
                .any(|message| matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { .. }))),
            "non-Wide show activation opens the Library Hero overlay"
        );
    }
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into()))
    );
    if width == 160 {
        assert!(tv(&harness).episode_pane_focused());
    } else {
        assert!(panel(&harness).test_hero_overlay_open());
    }
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_mouse_click_resolves_the_painted_show_target_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    let (column, row) = tv_tree_text_position(&mut harness, "Second Movie");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));

    let outcome = harness.step();
    assert_eq!(
        outcome
            .messages
            .iter()
            .filter(|message| matches!(
                message,
                Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
                    hit: TvHit::SeriesRow(target)
                } if target == "series-1")))
            .count(),
        1,
        "one Library Panel painter must resolve the row exactly once: {:?}",
        outcome.messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the Library Panel click must update the canonical TV tree selection"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_mouse_uses_the_latest_painted_geometry_across_resize(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    let (row_column, row) = tv_tree_text_position(&mut harness, "Second Movie");
    let painted_list = panel(&harness)
        .test_list_rect()
        .expect("the TV tree painted through the Library Panel");
    let column = if width == 160 {
        painted_list.right().saturating_sub(1)
    } else {
        row_column
    };

    // Resize the shell and run its sync pass without drawing. The component
    // must continue to resolve input against the frame it actually painted.
    harness.model_mut().app.terminal_width = if width == 160 { 80 } else { 160 };
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(
        outcome
            .messages
            .iter()
            .filter(|message| matches!(
                message,
                Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
                    hit: TvHit::SeriesRow(target)
                } if target == "series-1")))
            .count(),
        1,
        "mouse delivery must use the last painted tree geometry exactly once: {:?}",
        outcome.messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-1".into())),
        "the stale painted-frame event resolves to the same stable target"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn right_on_expanded_show_activates_its_workspace_through_tick(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    tick_tv_key(&mut harness, Key::Right);

    let messages = tick_tv_key(&mut harness, Key::Right);
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { item } if item.id == "series-0"))),
        "Right on the expanded Show must emit its stable-target activation: {messages:?}"
    );
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into())),
        "Workspace activation must not collapse or move the tree selection"
    );
}

#[rstest]
#[case::wide(160, false)]
#[case::narrow(80, false)]
#[case::mini(crate::app::MINI_VIEW_THRESHOLD - 1, true)]
fn tv_tree_episode_activation_plays_the_resolved_episode_in_every_geometry(
    #[case] width: u16,
    #[case] mini: bool,
) {
    let mut harness = tv_tree_geometry(width, mini);
    tick_tv_key(&mut harness, Key::Right); // Expand the cached show detail.
    assert_eq!(
        tv(&harness).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:8:series-0".into()))
    );
    tick_tv_key(&mut harness, Key::Down); // Select Season 1.
    assert!(matches!(
        tv(&harness).selected_tree_target(),
        Some(TvTreeTarget::Season { show, season, .. })
            if show == "tv-id:8:series-0" && season == "season-1"
    ));
    tick_tv_key(&mut harness, Key::Enter); // Expand Season 1.
    let messages = tick_tv_key(&mut harness, Key::Right);
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { .. }))),
        "Right on an expanded Season must not activate its Show Workspace"
    );
    tick_tv_key(&mut harness, Key::Down); // Still-visible Episode 1 proves Right did not collapse.
    assert!(matches!(
        tv(&harness).selected_tree_target(),
        Some(TvTreeTarget::Episode { show, season, episode, .. })
            if show == "tv-id:8:series-0" && season == "season-1" && episode == "episode-1"
    ));

    let messages = tick_tv_key(&mut harness, Key::Enter);
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { episode } if episode.id == "episode-1"))),
        "Enter must resolve the selected episode identity: {messages:?}"
    );
    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "episode-1",
        "shell dispatch must play the episode carried by the selected tree target"
    );
}
