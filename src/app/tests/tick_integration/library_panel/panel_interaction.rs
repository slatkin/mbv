use super::*;

#[test]
fn library_panel_focus_follows_the_active_library() {
    let (mut harness, _log) = migrated_home();
    assert_eq!(
        harness.model().application.focus().cloned(),
        Some(ComponentId::Library),
        "the migrated tab's focus is the Library panel"
    );

    // Music is migrated too: its Service owner remains behind the Library panel.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );

    // Back to the migrated tab: the panel takes focus again.
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );
}

/// Mouse eligibility follows the painted panel: the migrated surface is
/// subscribed, an un-migrated tab unsubscribes it, and a click on a painted
/// selector pill delivers `SelectorPicked` to the active owner through the
/// real subscription path.
#[test]
fn library_panel_mouse_eligibility_and_pill_slot_events() {
    let (mut harness, log) = migrated_home();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    // Music is migrated: the panel remains the painted and subscribed boundary.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    // Draw, then click the first painted selector pill through the real
    // subscription.
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "All").expect("the migrated owner's selector pill paints");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let log = log.borrow();
    assert!(
        log.events
            .iter()
            .any(|event| matches!(event, LibrarySlotEvent::SelectorPicked(0))),
        "the painted pill's slot event reached the active owner"
    );
    assert!(
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))),
        "the owner's claim marker flows through the tick"
    );
}

/// The Wide split-boundary drag is owned by the panel:
/// through the real subscription, a press inside the panel's painted gap
/// arms only the split gesture and the drag resolves the live width.
#[test]
fn library_panel_split_drag_resolves_the_live_width() {
    let (mut harness, _log) = migrated_home();
    let terminal = draw_frame(&mut harness);
    drop(terminal);

    // The panel's painted gap, read from its retained geometry.
    let gap = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_split_gap())
        .expect("the migrated surface paints a Wide split");
    assert!(gap.width > 0 && gap.height > 0);

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
        outcome.raw_messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(crate::app::components::msg::ShellRequest::ResizeListPaneLive(_))
        )),
        "the panel's split drag resolves the live width through the tick"
    );

    // Dispatch the drag's request the way the run loop does, then the shell
    // has stored the width and the panel receives it on the next sync.
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.list_pane_width.is_some());
}

/// An inactive owner keeps its cursor/scroll across a tab change: the
/// selection made on the migrated tab survives the round trip through an
/// un-migrated tab (the owner map's retention rule, design D2).
#[test]
fn inactive_owner_keeps_cursor_scroll_across_a_tab_change() {
    let (mut harness, log) = migrated_home();

    // Click the second row: the fixture's carrier selects "beta".
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

    // Switch to the un-migrated Emby library, then back: the owner stays.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().library_panel_has_owner(&home_key()));
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();

    // A wheel move from the retained selection lands on the NEXT row — only
    // true if the selection survived the tab change as "beta".
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
        "the wheel step continues from the selection made before the tab change"
    );
}
