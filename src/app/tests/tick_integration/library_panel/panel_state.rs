use super::*;

#[test]
fn wide_to_narrow_resize_drops_the_stale_wide_geometry() {
    let (mut harness, log) = migrated_home();
    drop(draw_frame(&mut harness));

    // The Wide frame's painted gap and list rect, read from the panel.
    let gap = panel_of(&harness)
        .and_then(|panel| panel.test_split_gap())
        .expect("the Wide frame paints a split gap");
    let wide_list = panel_of(&harness)
        .and_then(|panel| panel.test_list_rect())
        .expect("the Wide frame paints a list slot");

    // Resize to Narrow (below TWO_COLUMN_THRESHOLD, above the mini view) and
    // draw: the vanished split claims nothing and the list rect is the
    // narrow one.
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_split_gap())
            .is_none(),
        "the Narrow frame must not retain the Wide split's gap"
    );
    let narrow_list = panel_of(&harness)
        .and_then(|panel| panel.test_list_rect())
        .expect("the Narrow frame paints a list slot");

    // A press at the old gutter position no longer arms the split drag, so
    // the drag resolves no live width.
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: gap.x,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: gap.x + 8,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        !outcome.raw_messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::ResizeListPaneLive(_)))),
        "the vanished gutter must not arm the split drag"
    );

    // A click inside the newly painted narrow list, not the stale Wide list
    // rect, reaches the owner.
    let click_x = narrow_list.x + 1;
    assert!(
        !wide_list.contains(ratatui::layout::Position {
            x: click_x,
            y: narrow_list.y + 1,
        }),
        "test setup: the click must sit outside the stale Wide list rect"
    );
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_x,
        row: narrow_list.y + 1,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(
        log.borrow().events.iter().any(|event| matches!(
            event,
            LibrarySlotEvent::List(MediaListSurfaceInput::Click(_))
        )),
        "the narrow list click outside the stale Wide rect reaches the owner"
    );
}

/// Owner state survives a Panel-mode round trip (design D2's retention rule):
/// queue-only hides the library column but must not destroy the owners, so
/// the selection made before the switch still drives the wheel step after it,
/// and the panel still takes focus and resolves clicks.
#[test]
fn owner_state_survives_a_queue_only_round_trip() {
    let (mut harness, log) = migrated_home();

    // Select "beta" on the migrated Home tab.
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the second row paints");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(log.borrow().selections.last(), Some(&Some("beta".into())));

    // Queue-only hides the library column; the panel stays mounted with its
    // owners (mounted ≠ painted) instead of dropping them.
    harness.model_mut().app.panel_mode = PanelMode::QueueOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().library_panel_has_owner(&home_key()),
        "the hidden library column must not destroy the owner map"
    );

    // Back to library-only: the selection survived and the panel routes
    // clicks again.
    harness.model_mut().app.panel_mode = PanelMode::LibraryOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library),
        "the panel takes focus again after the round trip"
    );
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the retained owner's rows repaint");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        log.borrow().selections.last(),
        Some(&Some("gamma".into())),
        "the wheel step continues from the selection made before the mode switch"
    );
}

/// Enter and browser double-click are delivered through the mounted panel
/// after the shell sync pass. Both open the Library-local overlay; the mouse
/// path resolves the clicked row rather than the pre-existing cursor.
#[test]
fn mounted_narrow_activation_opens_overlay_for_leaf_and_workspace() {
    let (mut harness, _log) = migrated_home();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Enter,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome
        .raw_messages
        .iter()
        .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))));
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    let (mut harness, log) = migrated_home_with_workspace();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    let terminal = draw_frame_sized(&mut harness);
    let beta = find_text(terminal.backend().buffer(), "beta").expect("first browser row");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: beta.0,
        row: beta.1,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(log.borrow().selections.last(), Some(&Some("beta".into())));

    let terminal = draw_frame_sized(&mut harness);
    let gamma = find_text(terminal.backend().buffer(), "gamma").expect("second browser row");
    for _ in 0..2 {
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: gamma.0,
            row: gamma.1,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().any(|message| {
            matches!(
                message,
                Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
            )
        }));
    }
    assert_eq!(log.borrow().selections.last(), Some(&Some("gamma".into())));
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    // The 30-row overlay leaves the Workspace one content row: the landscape
    // grid packs the Dune/2021 title/meta entries into a single row, the
    // Workspace box takes the remaining rows, and the cursor on the second
    // track clamps the viewport scroll to 1 so it stays visible.
    let (_, workspace_content) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    assert_eq!(
        workspace_content.height, 1,
        "the Workspace viewport holds a single row"
    );
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        (1, 1, Some("track two".into()))
    );
}

/// The Library overlay is local to its panel: Queue can take focus and handle
/// a normal action without dismissing it, then Library focus restores the
/// retained overlay state.
#[test]
fn mounted_queue_action_preserves_unfocused_library_overlay() {
    let (mut harness, _log) = migrated_home_with_workspace();
    harness.model_mut().app.panel_mode = PanelMode::Both;
    harness.model_mut().sync_mounted_surfaces();
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        })
        .expect("Library panel mounted")
        .test_open_hero_overlay();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .focus_hero_workspace();
    // Baseline the painted steady state: the first draw clamps the
    // Workspace viewport scroll to the cursor (the 1-row viewport keeps
    // row 1 visible), so the Queue round trip below proves input
    // preservation rather than re-proving the paint clamp.
    drop(draw_frame(&mut harness));
    let workspace_state = harness
        .model()
        .library_owner::<FixtureOwner>(&home_key())
        .unwrap()
        .workspace_state();
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Down,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Esc,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "Queue Esc must not dismiss Library overlay"
    );
    drop(draw_frame(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        workspace_state,
        "Queue input preserves the overlay Workspace state"
    );
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        workspace_state,
        "returning to Library restores the Workspace state"
    );
}
