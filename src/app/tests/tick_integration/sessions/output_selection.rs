//! The tick tests that assert what a target activation sends to the Owner
//! (confirm-output-handoff): the shared continue-playback gate asks first
//! when a slot is active on the Owner's output, and the Yes answer then
//! sends the selection the unasked path sent.

use super::*;

/// cast-owner-output row 8.1, design D13, cast-session-control spec
/// "Selecting a cast target makes it the owner's output": activating a cast
/// row sends `CtrlCmd::SelectPlaybackOutput(Cast)` to the owner whose
/// `playback-output` capability gated the row, and no `Stop` precedes it —
/// the owner performs the Cast-to-Cast hand-off, and a prior stop would
/// clear the active slot the new receiver must continue.
#[test]
fn tick_sessions_cast_row_activation_sends_the_owner_output_selection() {
    let (mut app, cmd_rx) = playback_output_owner_app();
    app.terminal_width = 100;
    app.terminal_height = 14;
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.panel_targets = vec![cast_target("r-1")];
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Cast("r-1".into()));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );

    // The stub owner's status is active, so the shared continue-playback
    // gate asks first (confirm-output-handoff); the Yes answer then sends
    // the selection.
    let modal = match &harness.model().app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal.clone(),
        other => panic!("the gate must be raised first, got {other:?}"),
    };
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: Some(SessionTargetKey::Cast("r-1".into())),
            name: "Living Room".to_string(),
        }
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "raising the gate sends nothing: the prior playback continues while the user answers"
    );
    harness.model_mut().app.apply_confirm_action(
        modal.on_confirm,
        mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Enter, mbv_ui_msg::KeyModifiers::NONE),
    );

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast {
                ref receiver_id,
                ref name,
                ref host,
                port,
            }) if receiver_id == "r-1" && name == "Living Room"
                && host == "192.168.0.5" && port == 8009
        ),
        "the activation selects that receiver as the owner's output"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "no command precedes the selection: the owner performs the hand-off"
    );
}

/// emby-session-owner-output row 7.1, design D12, cast-session-control spec
/// "An Emby Session is selected while a cast target is attached": activating
/// an Emby row without a direct endpoint severs the previous Client target
/// and sends `SelectPlaybackOutput(EmbySession)` to the owner, with no `Stop`
/// — the owner performs the Cast-to-Session hand-off and continues the
/// active slot.
#[test]
fn tick_sessions_emby_row_without_direct_endpoint_selects_the_owner_output() {
    let (mut app, cmd_rx) = both_output_capabilities_owner_app();
    app.terminal_width = 100;
    app.terminal_height = 14;
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    let mut session = mbv_emby::test_support::make_session("Living Room TV", "Emby");
    session.id = "sess-tv".into();
    session.device_id = "device-tv".into();
    app.panel_targets = vec![PanelTarget::Emby(Box::new(session))];
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("sess-tv".into()));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );

    // The stub owner's status is active, so the shared continue-playback
    // gate asks first (confirm-output-handoff); the Yes answer then sends
    // the selection.
    let modal = match &harness.model().app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal.clone(),
        other => panic!("the gate must be raised first, got {other:?}"),
    };
    assert!(matches!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: Some(SessionTargetKey::Emby(ref id)),
            ..
        } if id == "sess-tv"
    ));
    assert!(
        cmd_rx.try_recv().is_err(),
        "raising the gate sends nothing: the prior playback continues while the user answers"
    );
    harness.model_mut().app.apply_confirm_action(
        modal.on_confirm,
        mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Enter, mbv_ui_msg::KeyModifiers::NONE),
    );

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ref name,
            }) if device_id == "device-tv" && name == "Living Room TV"
        ),
        "the activation selects that Session as the owner's output"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "no command precedes the selection: the owner performs the hand-off"
    );
}

/// emby-session-owner-output row 7.1, design D12: a Session that advertises a
/// direct endpoint reports a failed direct connect and then, when the owner
/// advertises `emby-session-output`, selects the same Session as its output.
/// With a slot active on the owner's output the activation first raises the
/// hand-off fork; declining it (Esc) runs the plain switch whose failed
/// connect falls back to the output selection.
#[test]
fn tick_sessions_emby_row_with_failed_direct_connect_reports_and_selects_the_owner_output() {
    use crate::app::{DIRECT_CONNECT_OVERRIDE, DIRECT_CONNECT_TEST_LOCK};

    let _guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(failed_direct_connect);
    let (mut app, cmd_rx) = both_output_capabilities_owner_app();
    app.terminal_width = 100;
    app.terminal_height = 14;
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    let mut session = mbv_emby::test_support::make_session("remote-host", "mbv");
    session.device_id = "device-direct".into();
    session.host = "192.168.1.20".into();
    session.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    app.panel_targets = vec![PanelTarget::Emby(Box::new(session.clone()))];
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("sess-1".into()));
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );

    // The stub owner's status is active, so the fork is raised first.
    let modal = match &harness.model().app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal.clone(),
        other => panic!("the fork must be raised first, got {other:?}"),
    };
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
            device_id: "device-direct".into(),
            name: "remote-host".into(),
        }
    );
    harness.model_mut().app.apply_confirm_action(
        modal.on_confirm.clone(),
        mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Esc, mbv_ui_msg::KeyModifiers::NONE),
    );
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(
        harness
            .model()
            .app
            .newest_toast_text()
            .contains("Direct mbv control failed"),
        "the failed direct connect is reported"
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ref name,
            }) if device_id == "device-direct" && name == "remote-host"
        ),
        "the failed direct connect falls back to the Session output selection"
    );
}
