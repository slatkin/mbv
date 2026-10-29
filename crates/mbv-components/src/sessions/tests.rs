use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use tuirealm::event::{KeyModifiers, MouseButton};

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn test_session_disconnect_key_detaches_cast_only_attachment() {
    let mut component = SessionsComponent::new();
    component.can_disconnect = false;
    component.cast_attachment_id = Some("cast-1".to_string());
    assert_eq!(
        component.handle_key(&key(Key::Char('d'))),
        Some(Msg::Shell(Box::new(ShellRequest::DetachSessions)))
    );
}

#[test]
fn test_session_cross_boundary_keys_are_typed() {
    let mut component = SessionsComponent::new();
    assert_eq!(
        component.handle_key(&key(Key::Esc)),
        Some(Msg::Shell(Box::new(ShellRequest::DismissSessions)))
    );
    assert_eq!(
        component.handle_key(&key(Key::Char('r'))),
        Some(Msg::Shell(Box::new(ShellRequest::RefreshSessions)))
    );
    component.list.set_content(vec![ThreeLineItem::new(
        SessionTargetKey::Cast("cast-1".into()),
        std::array::from_fn(|_| Vec::new()),
    )]);
    assert_eq!(
        component.handle_key(&key(Key::Enter)),
        Some(Msg::Shell(Box::new(ShellRequest::SelectSession(
            SessionTargetKey::Cast("cast-1".into())
        ))))
    );
}

fn emby_target(id: &str) -> SessionTargetRow {
    SessionTargetRow::Emby {
        id: id.to_string(),
        device_id: format!("device-{id}"),
        device_name: format!("device-{id}"),
        client: "mbv".to_string(),
        user_name: "user".to_string(),
        host: "host".to_string(),
        now_playing: None,
        is_paused: false,
        position_s: 0,
        runtime_s: 0,
        has_direct_endpoint: false,
    }
}

/// An mbv peer row with the design D6 gating inputs the shell projects.
fn peer_target(
    id: &str,
    now_playing: Option<&str>,
    is_paused: bool,
    has_direct_endpoint: bool,
) -> SessionTargetRow {
    SessionTargetRow::Emby {
        id: id.to_string(),
        device_id: format!("device-{id}"),
        device_name: format!("device-{id}"),
        client: "mbv".to_string(),
        user_name: "user".to_string(),
        host: "host".to_string(),
        now_playing: now_playing.map(str::to_string),
        is_paused,
        position_s: 0,
        runtime_s: 0,
        has_direct_endpoint,
    }
}

fn component_with_rows(
    rows: &[SessionTargetRow],
    home_is_local_daemon: bool,
    local_owner_plays: bool,
) -> SessionsComponent {
    let mut component = SessionsComponent::new();
    component.set_content(
        rows,
        false,
        None,
        None,
        None,
        false,
        mbv_ui_model::panel_targets::SessionTransferGates {
            home_is_local_daemon,
            local_owner_plays,
        },
        Some(Rect::new(0, 0, 40, 12)),
    );
    component.view_for_test();
    component
}

/// Design D6: `h` emits `HandOffSession` with the selected row's stable
/// key only while the home is the local Owner, the local Owner plays,
/// and the row carries a direct endpoint; every closed gate emits
/// nothing.
#[test]
fn hand_off_key_emits_the_request_only_when_its_conditions_hold() {
    let peer = peer_target("peer", Some("Song"), false, true);
    let own_row = peer_target("self", Some("Song"), false, false);

    let mut component = component_with_rows(std::slice::from_ref(&peer), true, true);
    assert_eq!(
        component.handle_key(&key(Key::Char('h'))),
        Some(Msg::Shell(Box::new(ShellRequest::HandOffSession(
            SessionTargetKey::Emby("peer".into())
        ))))
    );

    // The local Owner paused: no Hand off.
    let mut component = component_with_rows(std::slice::from_ref(&peer), true, false);
    assert_eq!(component.handle_key(&key(Key::Char('h'))), None);

    // Not the local Owner's home: no Hand off.
    let mut component = component_with_rows(std::slice::from_ref(&peer), false, true);
    assert_eq!(component.handle_key(&key(Key::Char('h'))), None);

    // This device's own row carries no direct endpoint: no Hand off.
    let mut component = component_with_rows(std::slice::from_ref(&own_row), true, true);
    assert_eq!(component.handle_key(&key(Key::Char('h'))), None);

    // No selection: no request.
    let mut component = SessionsComponent::new();
    assert_eq!(component.handle_key(&key(Key::Char('h'))), None);
}

/// Design D6: `t` emits `TakeSession` with the selected row's stable key
/// only while the home is the local Owner, the row shows the peer
/// playing, and the row carries a direct endpoint. The local Owner's own
/// playback state does not gate Take.
#[test]
fn take_key_emits_the_request_only_when_its_conditions_hold() {
    let playing = peer_target("peer", Some("Song"), false, true);
    let paused = peer_target("peer", Some("Song"), true, true);
    let idle = peer_target("peer", None, false, true);
    let own_row = peer_target("self", Some("Song"), false, false);

    // The local Owner's playback state is irrelevant to Take.
    let mut component = component_with_rows(std::slice::from_ref(&playing), true, false);
    assert_eq!(
        component.handle_key(&key(Key::Char('t'))),
        Some(Msg::Shell(Box::new(ShellRequest::TakeSession(
            SessionTargetKey::Emby("peer".into())
        ))))
    );

    // The row shows the peer paused: no Take.
    let mut component = component_with_rows(std::slice::from_ref(&paused), true, true);
    assert_eq!(component.handle_key(&key(Key::Char('t'))), None);

    // The row shows the peer idle: no Take.
    let mut component = component_with_rows(std::slice::from_ref(&idle), true, true);
    assert_eq!(component.handle_key(&key(Key::Char('t'))), None);

    // Not the local Owner's home: no Take.
    let mut component = component_with_rows(std::slice::from_ref(&playing), false, true);
    assert_eq!(component.handle_key(&key(Key::Char('t'))), None);

    // This device's own row carries no direct endpoint: no Take.
    let mut component = component_with_rows(std::slice::from_ref(&own_row), true, true);
    assert_eq!(component.handle_key(&key(Key::Char('t'))), None);
}

/// The transfer arms match unmodified keys only, mirroring the policy
/// layer's exact-chord scope (review qht-6r P2): a modifier-bearing
/// press is not a transfer request.
#[test]
fn modifier_bearing_transfer_presses_emit_nothing() {
    let peer = peer_target("peer", Some("Song"), false, true);
    let mut component = component_with_rows(std::slice::from_ref(&peer), true, true);
    let ctrl_h = KeyEvent {
        code: Key::Char('h'),
        modifiers: KeyModifiers::CONTROL,
    };
    assert_eq!(component.handle_key(&ctrl_h), None);
    let ctrl_t = KeyEvent {
        code: Key::Char('t'),
        modifiers: KeyModifiers::CONTROL,
    };
    assert_eq!(component.handle_key(&ctrl_t), None);
}

fn emby_targets(count: usize) -> Vec<SessionTargetRow> {
    (0..count)
        .map(|index| emby_target(&format!("s-{index}")))
        .collect()
}

fn painted_component() -> SessionsComponent {
    let targets = vec![emby_target("a"), emby_target("b")];
    let mut component = SessionsComponent::new();
    component.set_content(
        &targets,
        false,
        None,
        None,
        None,
        false,
        mbv_ui_model::panel_targets::SessionTransferGates::default(),
        Some(Rect::new(0, 0, 40, 12)),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component
}

fn wheel_down() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }
}

fn left_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn test_session_mouse_click_on_selected_item_connects() {
    let mut component = painted_component();
    let content = component.content_area_for_test().unwrap();
    assert_eq!(
        component.handle_mouse(left_down(content.x, content.y)),
        Some(Msg::Shell(Box::new(ShellRequest::SelectSession(
            SessionTargetKey::Emby("a".into())
        ))))
    );
}

#[test]
fn test_session_mouse_click_on_unselected_item_selects_then_reclick_connects() {
    let mut component = painted_component();
    let point = Position {
        x: 10,
        y: component.content_area_for_test().unwrap().y + 4,
    };
    // The second item starts immediately after the first item's three lines.
    assert_eq!(component.handle_mouse(left_down(point.x, point.y)), None);
    assert_eq!(
        component.list.selected_target(),
        Some(&SessionTargetKey::Emby("b".into()))
    );
    component.view_for_test();
    component.reset_mouse_gestures_for_test();
    assert_eq!(
        component.handle_mouse(left_down(point.x, point.y)),
        Some(Msg::Shell(Box::new(ShellRequest::SelectSession(
            SessionTargetKey::Emby("b".into())
        ))))
    );
}

impl SessionsComponent {
    fn view_for_test(&mut self) {
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| self.view(frame, frame.area()))
            .unwrap();
    }
}

/// wheel-scrolls-viewport 5.2: the Sessions wheel applies the shared
/// viewport scroll to the gesture delta and never moves the selection.
#[test]
fn a_wheel_event_scrolls_sessions_and_keeps_the_selection() {
    let targets = emby_targets(10);
    let mut component = SessionsComponent::new();
    component.set_content(
        &targets,
        false,
        None,
        None,
        None,
        false,
        mbv_ui_model::panel_targets::SessionTransferGates::default(),
        Some(Rect::new(0, 0, 40, 12)),
    );
    component.view_for_test();
    let content = component.content_area_for_test().unwrap();
    assert_eq!(content.height, 7);

    assert_eq!(
        component.handle_mouse(wheel_down()),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    );

    let (selection, offset) = component.selection_and_offset_for_test();
    assert_eq!(selection, Some(SessionTargetKey::Emby("s-0".into())));
    assert_eq!(offset, 3, "the wheel moved the viewport by one step");
}
