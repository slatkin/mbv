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
