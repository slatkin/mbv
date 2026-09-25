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

#[test]
fn settings_mouse_click_on_a_service_selects_and_activates_it() {
    let mut component = painted_settings(SettingsDestination::Services);
    let (rect, cursor) = component.test_rows().regions()[0];
    assert_eq!(
        component.on(&mouse_down(rect.x, rect.y)),
        Some(Msg::Service(ServiceRequest::ActivateService(cursor)))
    );
    assert_eq!(component.services_cursor, cursor);
}

#[test]
fn settings_mouse_click_outside_the_painted_panel_dismisses() {
    let mut component = painted_settings(SettingsDestination::Main);
    let (x, width) = (
        component.geometry.panel_area.x,
        component.geometry.panel_area.width,
    );
    assert_eq!(
        component.on(&mouse_down(x + width + 5, 1)),
        Some(Msg::Shell(Box::new(ShellRequest::DismissSettings)))
    );
}

#[test]
fn settings_mouse_wheel_moves_one_line_inside_content() {
    let mut component = painted_settings(SettingsDestination::Main);
    component.geometry.content_area = Rect::new(0, 0, 40, 12);
    component.geometry.cursor_lines = vec![0, 20];
    component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.scroll, 1);
}

#[test]
fn settings_mouse_wheel_moves_off_panel_content() {
    let mut component = painted_settings(SettingsDestination::Main);
    component.geometry.panel_area = Rect::new(2, 2, 10, 4);
    component.geometry.content_area = Rect::new(2, 2, 10, 4);
    component.geometry.cursor_lines = vec![0, 20];
    component.scroll = 2;
    component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.scroll, 3);
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
        setup: Some(SetupDraft::Emby {
            fields: ["https://server".into(), "user".into(), String::new()],
            focus: 2,
            busy: false,
            error: String::new(),
        }),
        area: Rect::new(0, 0, 40, 12),
    });
    component.on(&key(Key::Char('x')));
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Service(ServiceRequest::SubmitEmbySetup { password, .. }))
            if password == "x"
    ));
}

#[test]
fn settings_renders_without_app_state() {
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Main,
        rows: vec![SettingsRow {
            label: "Stay alive".into(),
            value: "off".into(),
            section: false,
            cursor: Some(0),
        }],
        services: Vec::new(),
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("SETTINGS"));
    assert!(output.contains("Stay alive"));
}
