use crate::tui::test_support::{Terminal, TestBackend};
use rstest::rstest;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::dispatch::action::NavigationIntent;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::tests::make_app_stub;
use crate::tui::tests::tick_integration::harness::{StepOutcome, TickHarness};
use mbv_cast::discovery::CastReceiver;
use mbv_components::SessionsComponent;
use mbv_ui_model::panel_targets::SessionTargetKey;
use mbv_ui_msg::{ComponentId, Msg, OverlayId, ShellRequest, UserEvent};

mod output_selection;
fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn sessions(ids: &[&str]) -> Vec<crate::app::state::panel_targets::PanelTarget> {
    ids.iter()
        .map(|id| {
            let mut session = mbv_emby::test_support::make_session(id, "mbv");
            session.id = (*id).into();
            session.device_id = format!("device-{id}");
            crate::app::state::panel_targets::PanelTarget::Emby(Box::new(session))
        })
        .collect()
}

fn cast_target(id: &str) -> PanelTarget {
    PanelTarget::Cast(CastReceiver {
        id: id.to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    })
}

/// An app whose owner advertises the `playback-output` capability and whose
/// owner-bound commands arrive on the returned receiver (design D13 rows
/// 8.1/8.2; same pattern as `live_owner_channel` plus the capability).
fn playback_output_owner_app() -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    capability_owner_app(true, false)
}

/// An app whose owner advertises `playback-output` and, when `emby_session`
/// is set, `emby-session-output` (emby-session-owner-output design D3/D12).
fn capability_owner_app(
    supports_playback_output: bool,
    emby_session: bool,
) -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut app = make_app_stub();
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_playback_output = supports_playback_output;
    compatibility.supports_emby_session_output = emby_session;
    let cmd_rx = app.set_player_with_compatibility_for_test(compatibility);
    (app, cmd_rx)
}

/// An app whose owner advertises neither output capability. With a Local
/// endpoint it is a local Owner that cannot host a remote output; with a
/// peer endpoint it is the packaged `mbvd` hello, whose capabilities a Session
/// activation never consults (it severs to the local Owner first).
fn capabilityless_owner_app() -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    capability_owner_app(false, false)
}

/// An app whose owner advertises both output capabilities — the Local
/// owner's compatibility (emby-session-owner-output design D3/D12).
fn both_output_capabilities_owner_app() -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    capability_owner_app(true, true)
}

/// Injected direct-connect failure for row 7.1: the Session advertises a
/// direct endpoint, but the connect itself fails.
fn failed_direct_connect(
    _endpoint: &mbv_remote_player::DaemonEndpoint,
) -> Result<
    (
        mbv_remote_player::RemotePlayer,
        std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ),
    mbv_remote_player::RemotePlayerError,
> {
    Err(
        mbv_remote_player::RemotePlayerError::owner_mismatch_for_test(mbv_ctrl::BuildInfo {
            app_version: "test".to_string(),
            build_id: "test-build".to_string(),
            wire_id: String::new(),
        }),
    )
}

fn apply_messages(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

fn open_sessions(harness: &mut TickHarness) {
    harness.inject(key(Key::Function(3)));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        crate::app::input::router::RouterOutcome::Command(NavigationIntent::OpenSessions.into())
    );
    harness
        .model_mut()
        .dispatch_router_command(&NavigationIntent::OpenSessions.into());
    apply_messages(harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Overlay(OverlayId::Sessions))
    );
    let (width, height) = harness.model().terminal_size();
    draw(harness, width, height);
}

fn sessions_component_mut(harness: &mut TickHarness) -> &mut SessionsComponent {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Overlay(OverlayId::Sessions))
        .expect("Sessions sidebar mounted")
        .as_any_mut()
        .downcast_mut::<SessionsComponent>()
        .expect("Sessions component")
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) {
    crate::tui::test_support::set_terminal_size(harness.model_mut(), width, height);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
}

fn enter_request(harness: &mut TickHarness) -> SessionTargetKey {
    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    outcome
        .raw_messages
        .iter()
        .find_map(|message| match message {
            Msg::Shell(shell_boxed) => match shell_boxed.as_ref() {
                ShellRequest::SelectSession(key) => Some((*key).clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("Enter emits an identity-bearing activation")
}

#[rstest]
#[case::narrow(42)]
#[case::ordinary(100)]
fn tick_sessions_focus_selection_and_responsive_clamp(#[case] width: u16) {
    let mut app = make_app_stub();
    app.seed_panel_targets_for_test(sessions(&["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"]));
    let mut harness = TickHarness::new(app);
    crate::tui::test_support::set_terminal_size(harness.model_mut(), width, 14);
    open_sessions(&mut harness);
    draw(&mut harness, width, 14);

    for _ in 0..6 {
        harness.inject(key(Key::Down));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::TerminalEvent(mbv_ui_msg::TerminalObserverEvent::KeyClaimed)
        )));
    }
    assert_eq!(
        enter_request(&mut harness),
        SessionTargetKey::Emby("s6".into())
    );

    // A resize reprojects the existing selection and clamps its item viewport.
    let resized_width = if width == 42 { 100 } else { 42 };
    draw(&mut harness, resized_width, 14);
    assert_eq!(
        sessions_component_mut(&mut harness).selection_and_offset_for_test(),
        (Some(SessionTargetKey::Emby("s6".into())), 4),
        "selection remains keyed and scroll stays within the list after resize"
    );
}

#[test]
fn tick_sessions_pointer_selects_then_reclick_activates() {
    let mut app = make_app_stub();
    app.seed_panel_targets_for_test(sessions(&["first", "second"]));
    let mut harness = TickHarness::new(app);
    crate::tui::test_support::set_terminal_size(harness.model_mut(), 100, 24);
    open_sessions(&mut harness);
    draw(&mut harness, 100, 24);

    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(ComponentId::Overlay(OverlayId::Sessions)).collect()
    );
    let click = |harness: &mut TickHarness, item: u16| {
        let content = sessions_component_mut(harness)
            .content_area_for_test()
            .expect("painted content geometry");
        let point = ratatui::layout::Position {
            x: content.x + 1,
            y: content.y + item * 3,
        };
        assert_eq!(
            sessions_component_mut(harness).target_at_for_test(point),
            Some(SessionTargetKey::Emby(
                if item == 0 { "first" } else { "second" }.into()
            ))
        );
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: point.x,
            row: point.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    let first_click_event = click(&mut harness, 1);
    harness.inject(first_click_event);
    let first_click = harness.step();
    assert_eq!(
        first_click.pre_fold_focus,
        Some(ComponentId::Overlay(OverlayId::Sessions))
    );
    assert!(first_click
        .messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(SessionTargetKey::Emby("second".into())),
        "first click raw messages: {:?}",
        first_click.raw_messages
    );

    sessions_component_mut(&mut harness).reset_mouse_gestures_for_test();
    draw(&mut harness, 100, 24);
    let second_click_event = click(&mut harness, 1);
    harness.inject(second_click_event);
    let second_click = harness.step();
    assert!(second_click.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(SessionTargetKey::Emby(id)) if id == "second"))));
}

#[test]
fn tick_sessions_changed_snapshot_invalidates_stale_hits_until_repaint() {
    let mut app = make_app_stub();
    app.seed_panel_targets_for_test(sessions(&["removed", "kept"]));
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let old_point = {
        let content = sessions_component_mut(&mut harness)
            .content_area_for_test()
            .expect("painted content geometry");
        ratatui::layout::Position::new(content.x + 1, content.y)
    };
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        Some(SessionTargetKey::Emby("removed".into()))
    );

    harness
        .model_mut()
        .app
        .seed_panel_targets_for_test(sessions(&["replacement", "kept"]));
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        None,
        "changed target keys invalidate the previously painted hit before repaint"
    );

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: old_point.x,
        row: old_point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let stale_click = harness.step();
    assert!(stale_click
        .raw_messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));

    draw(&mut harness, 100, 24);
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        Some(SessionTargetKey::Emby("replacement".into())),
        "repaint publishes geometry for the new snapshot"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: old_point.x,
        row: old_point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let repainted_click = harness.step();
    assert!(repainted_click.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(SessionTargetKey::Emby(id)) if id == "replacement"))));
}

#[test]
fn tick_sessions_unchanged_sync_keeps_painted_pointer_target_without_redraw() {
    let mut app = make_app_stub();
    app.seed_panel_targets_for_test(sessions(&["first", "second"]));
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let point = {
        let content = sessions_component_mut(&mut harness)
            .content_area_for_test()
            .expect("painted content geometry");
        ratatui::layout::Position::new(content.x + 1, content.y + 3)
    };
    let second = SessionTargetKey::Emby("second".into());
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(point),
        Some(second.clone())
    );

    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(point),
        Some(second.clone()),
        "an unchanged shell sync must retain the last painted hit"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: point.x,
        row: point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let click = harness.step();
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(second)
    );
    assert!(click
        .raw_messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));
}

#[test]
fn tick_sessions_reordered_snapshot_activates_selected_identity() {
    let (mut app, cmd_rx) = both_output_capabilities_owner_app();
    app.seed_panel_targets_for_test(sessions(&["first", "selected"]));
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    harness.inject(key(Key::Down));
    harness.step();
    harness
        .model_mut()
        .app
        .seed_panel_targets_for_test(sessions(&["selected", "first"]));
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(SessionTargetKey::Emby("selected".into()))
    );

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("selected".into()));
    harness
        .model_mut()
        .app
        .seed_panel_targets_for_test(sessions(&["selected", "first"]));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ..
            }) if device_id == "device-selected"
        ),
        "the activation selects the reordered row's device identity"
    );
}

#[test]
fn tick_sessions_removed_target_request_is_noop_not_former_index_replacement() {
    let mut app = make_app_stub();
    app.seed_panel_targets_for_test(sessions(&["selected", "replacement"]));
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("selected".into()));
    harness
        .model_mut()
        .app
        .seed_panel_targets_for_test(sessions(&["replacement"]));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );
}

#[test]
fn tick_sessions_selection_delivers_through_focused_tick_after_f3() {
    let mut harness = TickHarness::new(make_app_stub());
    open_sessions(&mut harness);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert_eq!(
        outcome.pre_fold_focus,
        Some(ComponentId::Overlay(OverlayId::Sessions))
    );
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(mbv_ui_msg::TerminalObserverEvent::KeyClaimed)
    )));
}

/// Contract: spec scenario "Client connected to mbvd" — with an owner
/// compatibility that advertises neither output capability, the target
/// rebuild offers the Emby rows but no Cast row; the row-level Cast gate
/// reads the negotiated compatibility, not a bare flag.
#[test]
fn a_capabilityless_owner_rebuild_offers_no_cast_row() {
    let (mut app, _cmd_rx) = capabilityless_owner_app();
    let mut session = mbv_emby::test_support::make_session("Living Room TV", "mbv");
    session.id = "sess-tv".into();
    app.seed_sessions_and_receivers_for_test(
        vec![session],
        vec![CastReceiver {
            id: "r-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        }],
    );

    app.rebuild_panel_targets_for_test();

    let targets = app.panel_targets();
    assert_eq!(targets.len(), 1);
    assert!(
        matches!(&targets[0], PanelTarget::Emby(session) if session.id == "sess-tv"),
        "only the Emby row is offered"
    );
}

/// Contract: a local Owner that advertises neither output capability is
/// offered no Emby Session output: activating an Emby row without a direct
/// endpoint warns and sends the owner nothing. (A peer's missing capability
/// does not refuse the activation — see
/// `a_session_activation_while_controlling_mbvd_selects_it_on_the_home_owner`.)
#[test]
fn tick_sessions_emby_row_on_a_capabilityless_owner_selects_no_output() {
    let (mut app, cmd_rx) = capabilityless_owner_app();
    app.set_player_endpoint_for_test(mbv_remote_player::DaemonEndpoint::Local);
    let mut session = mbv_emby::test_support::make_session("Living Room TV", "Emby");
    session.id = "sess-tv".into();
    session.device_id = "device-tv".into();
    app.seed_panel_targets_for_test(vec![PanelTarget::Emby(Box::new(session))]);
    let mut harness = TickHarness::new(app);
    crate::tui::test_support::set_terminal_size(harness.model_mut(), 100, 14);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("sess-tv".into()));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );

    assert!(
        cmd_rx.try_recv().is_err(),
        "a capability-less owner is offered no output selection: the activation sends nothing"
    );
    assert!(
        harness
            .model()
            .app
            .newest_toast_text()
            .contains("cannot select an Emby Session"),
        "the activation reports why nothing was selected: {}",
        harness.model().app.newest_toast_text()
    );
}

/// Regression: F3 activation of a Session that advertises a direct endpoint
/// switches the Client's own attachment even when the current owner is a
/// remote endpoint — the shape of a Client connected to packaged `mbvd`,
/// whose hello carries neither output capability, so the owner-output
/// fallback would otherwise refuse and do nothing. Only a Session with no
/// direct endpoint falls through to that owner-output path
/// (spec `sessions-sidebar` "F3 activates the selected stable target",
/// `cast-session-control` "Playback targets are mutually exclusive").
#[test]
fn tick_sessions_direct_endpoint_activation_switches_away_from_an_mbvd_owner() {
    use crate::app::test_seams::{DIRECT_CONNECT_OVERRIDE, DIRECT_CONNECT_TEST_LOCK};
    use std::sync::Mutex;

    static REQUESTED: Mutex<Option<mbv_remote_player::DaemonEndpoint>> = Mutex::new(None);

    fn connect_to_peer(
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) -> Result<
        (
            mbv_remote_player::RemotePlayer,
            std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
        ),
        mbv_remote_player::RemotePlayerError,
    > {
        // Only the selected peer's advertised endpoint is reachable.
        let peer = mbv_remote_player::DaemonEndpoint::Tcp("192.168.1.20:47788".parse().unwrap());
        if *endpoint != peer {
            return Err(
                mbv_remote_player::RemotePlayerError::owner_mismatch_for_test(
                    mbv_ctrl::BuildInfo {
                        app_version: "test".to_string(),
                        build_id: "test-build".to_string(),
                        wire_id: String::new(),
                    },
                ),
            );
        }
        *REQUESTED.lock().unwrap() = Some(endpoint.clone());
        Ok(mbv_remote_player::RemotePlayer::stub(Vec::new(), 0))
    }

    let _guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    *REQUESTED.lock().unwrap() = None;
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_to_peer);

    // The owner is the packaged daemon this Client is already attached to.
    let (mut app, cmd_rx) = capabilityless_owner_app();
    app.seed_direct_remote_for_test(
        mbv_remote_player::DaemonEndpoint::Tcp("192.168.1.9:47788".parse().unwrap()),
        "sess-mbvd".to_string(),
        "mbv".to_string(),
    );
    let mut session = mbv_emby::test_support::make_session("peer-host", "mbv");
    session.id = "sess-peer".into();
    session.device_id = "device-peer".into();
    session.host = "192.168.1.20".into();
    session.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    app.seed_panel_targets_for_test(vec![PanelTarget::Emby(Box::new(session))]);
    let mut harness = TickHarness::new(app);
    crate::tui::test_support::set_terminal_size(harness.model_mut(), 100, 14);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("sess-peer".into()));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;

    let peer_endpoint =
        mbv_remote_player::DaemonEndpoint::Tcp("192.168.1.20:47788".parse().unwrap());
    assert_eq!(
        REQUESTED.lock().unwrap().clone(),
        Some(peer_endpoint.clone()),
        "the activation connects to the selected Session's direct endpoint"
    );
    let app = &harness.model().app;
    assert_eq!(app.player_endpoint_for_test(), Some(&peer_endpoint));
    assert_eq!(app.direct_remote_session_id_for_test(), Some("sess-peer"));
    assert_eq!(app.direct_remote_label_for_test(), Some("peer-host"));
    assert!(app.direct_remote_connected_for_test());
    assert!(
        cmd_rx.try_recv().is_err(),
        "the previous owner is severed without an output selection: mbvd keeps its own playback"
    );
}
