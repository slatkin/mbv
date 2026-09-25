use super::*;

#[test]
fn tick_clears_multi_selection_when_library_destination_changes() {
    let (mut harness, _log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .select_multiple_for_test();
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets(),
        vec!["alpha", "beta"]
    );
    let terminal = draw_frame(&mut harness);
    let tab = harness
        .model()
        .application
        .get_component(&ComponentId::TabPanel)
        .expect("tab panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::TabPanel>()
        .expect("tab panel component")
        .hit_regions()
        .iter()
        .find(|(_, position)| *position == 1)
        .map(|(rect, _)| *rect)
        .expect("library tab painted");
    drop(terminal);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: tab.x,
        row: tab.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets()
            .is_empty(),
        "switching destination through the tick clears selection"
    );
}

#[test]
fn tick_context_menu_overlay_does_not_clear_multi_selection() {
    let (mut harness, _log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .select_multiple_for_test();

    // Dispatch the same typed request produced by a selected-row context
    // click. Opening the overlay changes TuiRealm's active component, but it
    // must not run the destination-identity clearing hook.
    let request = Msg::Shell(Box::new(
        crate::app::components::msg::ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Emby(vec![
                crate::app::tests::make_item("context", "Movie"),
            ]),
            Some((10, 10)),
        ),
    ));
    let mut music_resize = false;
    let mut tv_resize = false;
    // The shell resolves Emby context targets only for an active Emby tab;
    // restore Home before the sync pass so this test isolates overlay focus
    // from the destination-identity boundary under test.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness
        .model_mut()
        .handle_terminal_message(request, &mut music_resize, &mut tv_resize);
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    let menu_id = ComponentId::Overlay(crate::app::components::OverlayId::ContextMenu);
    assert!(harness.model().application.mounted(&menu_id));
    assert_eq!(harness.model().application.focus(), Some(&menu_id));

    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets(),
        vec!["alpha", "beta"]
    );
}
