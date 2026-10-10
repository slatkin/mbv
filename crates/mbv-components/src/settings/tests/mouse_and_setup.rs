use super::*;

#[test]
fn settings_mouse_click_selects_and_activates_a_row() {
    let mut component = painted_settings(SettingsDestination::Main);
    let (rect, cursor) = component.test_rows().regions()[0];
    assert!(matches!(
        component.on(&mouse_down(rect.x, rect.y)),
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::SettingsIntent(
            SettingsIntent::Activate(c)
        ) if *c == cursor)));
    assert_eq!(component.cursor, cursor);
}

fn setup_draft() -> SetupDraft {
    SetupDraft::Emby {
        fields: ["https://server".into(), "user".into(), String::new()],
        focus: 2,
        busy: false,
        error: String::new(),
    }
}

#[test]
fn setup_edits_are_local_and_submit_is_typed_service_request() {
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Services,
        rows: Vec::new(),
        services: vec![ServiceRow {
            name: "Emby".into(),
            detail: "Not configured".into(),
            muted: false,
        }],
        keys: Vec::new(),
        setup: Some(setup_draft()),
        area: Rect::new(0, 0, 40, 12),
    });
    component.on(&key(Key::Char('x')));
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Service(ServiceRequest::SubmitEmbySetup { password, .. }))
            if password == "x"
    ));
}

/// A Main-destination document tall enough that the 12-row panel cannot
/// show it all, so a wheel step has room to move the shared scroll.
fn tall_painted_settings() -> SettingsComponent {
    let rows = (0..24)
        .map(|i| SettingsRow {
            label: format!("Setting {i}"),
            value: "on".into(),
            section: false,
            cursor: Some(i),
            kind: SettingValueKind::Text,
        })
        .collect();
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Main,
        rows,
        services: Vec::new(),
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component
}

/// Contract: mouse-input "Wheel behavior is verified for each scrollable
/// surface" (task 5.5, design D7). Settings is a variable-height document
/// with three cursors sharing one scroll: one wheel step moves the shared
/// document `scroll` and moves none of the cursors.
#[test]
fn settings_wheel_steps_the_document_and_leaves_the_cursors_alone() {
    let mut component = tall_painted_settings();
    let wheel = Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        component.on(&wheel),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(component.scroll, 3, "one wheel step = three document lines");
    assert_eq!(component.cursor, 0, "the wheel leaves the cursor");
    assert_eq!(
        component.services_cursor, 0,
        "the wheel leaves the services cursor"
    );
    assert_eq!(component.keys_cursor, 0, "the wheel leaves the keys cursor");
}
