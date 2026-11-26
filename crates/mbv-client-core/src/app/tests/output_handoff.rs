//! The shared continue-playback gate's cancel and local-target contracts
//! (confirm-output-handoff): No sends nothing and leaves the output alone,
//! and the local target asks while a remote output holds an active slot.
//! This file also owns the thin-client send-target contracts: every Yes
//! answer acts on the home Owner's slot the prompt named, never on the
//! peer's output. The ask-until-Yes and nothing-active-skip contracts live
//! with the selection flows that exercise them (`session_connect`,
//! `tick_integration::sessions`, `input::gui::tests::target_chooser`).

use std::sync::mpsc;

use crate::app::state::panel_targets::PanelTarget;
use crate::app::tests::*;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

fn key(code: Key) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// An owner attached as the local Owner that advertises `playback-output`
/// and, when `output_switch` is set, `output-switch`, holding one cast
/// receiver, with `active` marking the stub slot active.
fn cast_output_owner_app(
    active: bool,
    output_switch: bool,
) -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut app = make_app_stub();
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_playback_output = true;
    compatibility.supports_output_switch = output_switch;
    remote.set_ctrl_compatibility_for_test(compatibility);
    remote.update_status(|status| status.active = active);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = player_rx;
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.cast_receivers = vec![mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }];
    app.rebuild_panel_targets();
    (app, cmd_rx)
}

/// The three target kinds the capable continue-playback prompt can name.
#[derive(Clone, Copy, Debug)]
enum SwitchTargetKind {
    Cast,
    EmbySession,
    Local,
}

/// The Cast receiver row every Cast case selects.
fn switch_cast_receiver() -> mbv_cast::discovery::CastReceiver {
    mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }
}

/// An Emby Session row that offers no direct endpoint: the site that raises
/// the capable prompt for an Emby target.
fn no_direct_endpoint_session() -> mbv_emby::SessionInfo {
    let mut sess = make_session("Bedroom TV", "Emby");
    sess.id = "sess-2".to_string();
    sess.device_id = "device-2".to_string();
    sess
}

/// A local Owner that advertises the output switch, holds an active slot,
/// and offers `kind` as the target its prompt names.
fn switch_capable_app(
    kind: SwitchTargetKind,
) -> (crate::app::App, mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    match kind {
        SwitchTargetKind::Cast => cast_output_owner_app(true, true),
        SwitchTargetKind::EmbySession => {
            let mut app = make_app_stub();
            let (mut remote, player_rx, cmd_rx) =
                mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
            let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
            compatibility.supports_emby_session_output = true;
            compatibility.supports_output_switch = true;
            remote.set_ctrl_compatibility_for_test(compatibility);
            remote.update_status(|status| status.active = true);
            app.player = mbv_player::PlayerProxy::from_remote(remote, false);
            app.player_rx = player_rx;
            app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
            app.panel_targets = vec![PanelTarget::Emby(Box::new(no_direct_endpoint_session()))];
            (app, cmd_rx)
        }
        SwitchTargetKind::Local => {
            let (mut app, cmd_rx) = cast_output_owner_app(true, true);
            let output = mbv_ctrl::OutputState::Cast {
                receiver_id: "receiver-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::OutputConnection::Connected,
            };
            app.player
                .remote()
                .update_playback_output_for_test(output.clone());
            app.playback_output = Some(output);
            (app, cmd_rx)
        }
    }
}

/// Raise the capable prompt by selecting `kind`'s target through the site
/// that raises it.
fn raise_switch_prompt(app: &mut crate::app::App, kind: SwitchTargetKind) {
    match kind {
        SwitchTargetKind::Cast => {
            app.select_panel_target(PanelTarget::Cast(switch_cast_receiver()));
        }
        SwitchTargetKind::EmbySession => {
            app.activate_session_target(&no_direct_endpoint_session());
        }
        SwitchTargetKind::Local => app.detach_to_local(),
    }
}

/// The output selection both answers send for `kind`'s target.
fn expected_selection(kind: SwitchTargetKind) -> mbv_ctrl::OutputSelection {
    match kind {
        SwitchTargetKind::Cast => mbv_ctrl::OutputSelection::Cast {
            receiver_id: "receiver-1".to_string(),
            name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        },
        SwitchTargetKind::EmbySession => mbv_ctrl::OutputSelection::EmbySession {
            device_id: "device-2".to_string(),
            name: "Bedroom TV".to_string(),
        },
        SwitchTargetKind::Local => mbv_ctrl::OutputSelection::Local,
    }
}

/// The confirm modal the gate raised.
fn pending_confirm_modal(app: &crate::app::App) -> &mbv_ui_model::confirm::ConfirmModal {
    match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal,
        other => panic!("the gate must raise the confirm modal, got {other:?}"),
    }
}

/// The confirm action the gate raised.
fn pending_confirm_action(app: &crate::app::App) -> mbv_ui_model::confirm::ConfirmAction {
    pending_confirm_modal(app).on_confirm.clone()
}

/// Assert that the switch answer sent exactly the output switch for `kind`'s
/// target and no output selection.
fn assert_switch_sent(cmd_rx: &mpsc::Receiver<mbv_ctrl::CtrlCmd>, kind: SwitchTargetKind) {
    match cmd_rx
        .try_recv()
        .expect("the switch answer sends one command")
    {
        mbv_ctrl::CtrlCmd::SwitchPlaybackOutput(selection) => {
            assert_eq!(selection, expected_selection(kind));
        }
        other => panic!("the switch answer must send an output switch, got {other:?}"),
    }
    assert!(
        cmd_rx.try_recv().is_err(),
        "the switch answer sends no output selection"
    );
}

/// Assert that the continue answer sent exactly `expected` and no switch.
fn assert_selection_sent(
    cmd_rx: &mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    expected: &mbv_ctrl::OutputSelection,
) {
    match cmd_rx
        .try_recv()
        .expect("the continue answer sends one command")
    {
        mbv_ctrl::CtrlCmd::SelectPlaybackOutput(selection) => assert_eq!(&selection, expected),
        other => panic!("the continue answer must send an output selection, got {other:?}"),
    }
    assert!(
        cmd_rx.try_recv().is_err(),
        "the continue answer sends no output switch"
    );
}

/// A Cast selection with a slot active on the current output asks first, and
/// No cancels: no command reaches the Owner and the current output keeps
/// playing (confirm-output-handoff).
#[test]
fn a_declined_cast_hand_off_sends_nothing_and_keeps_the_output() {
    let (mut app, cmd_rx) = cast_output_owner_app(true, false);

    app.select_panel_target(PanelTarget::Cast(mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }));

    let modal = pending_confirm_modal(&app);
    assert!(
        modal.message.contains("Living Room"),
        "the prompt names the target, got {:?}",
        modal.message
    );
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: Some(mbv_ui_model::panel_targets::SessionTargetKey::Cast(
                "receiver-1".to_string()
            )),
            name: "Living Room".to_string(),
            switch_only: false,
        }
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "raising the gate sends nothing: the prior playback continues while the user answers"
    );

    app.apply_confirm_action(modal.on_confirm.clone(), key(Key::Esc));

    assert!(
        cmd_rx.try_recv().is_err(),
        "No cancels: the Owner receives nothing and the output keeps playing"
    );
}

/// The continue-playback prompt's buttons and stored flag follow the local
/// Owner's `output-switch` capability at raise time (design D6 of
/// `switch-output-without-hand-off`): an Owner that advertises it gets
/// Continue / Just switch, and an Owner that predates it keeps Yes / No, so
/// the old answer still cancels and no command the Owner cannot decode is
/// ever sent.
#[rstest]
#[case::switch_capable(true, "Continue", "Just switch")]
#[case::without_capability(false, "Yes", "No")]
fn the_continue_prompt_follows_the_owner_output_switch_capability(
    #[case] output_switch: bool,
    #[case] affirmative: &str,
    #[case] cancel: &str,
) {
    let (mut app, _cmd_rx) = cast_output_owner_app(true, output_switch);

    app.select_panel_target(PanelTarget::Cast(mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }));

    let modal = pending_confirm_modal(&app);
    assert_eq!(modal.buttons[0].label, affirmative);
    assert_eq!(modal.buttons[1].label, cancel);
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: Some(mbv_ui_model::panel_targets::SessionTargetKey::Cast(
                "receiver-1".to_string()
            )),
            name: "Living Room".to_string(),
            switch_only: output_switch,
        }
    );
}

/// The capable prompt's switch answer sends the output switch for the same
/// target the continue answer selects, and no output selection (design D2,
/// D7 of `switch-output-without-hand-off`).
#[rstest]
#[case::cast(SwitchTargetKind::Cast)]
#[case::emby_session(SwitchTargetKind::EmbySession)]
#[case::local(SwitchTargetKind::Local)]
fn the_switch_answer_sends_the_output_switch_for_the_target(#[case] kind: SwitchTargetKind) {
    let (mut app, cmd_rx) = switch_capable_app(kind);
    raise_switch_prompt(&mut app, kind);

    let action = pending_confirm_action(&app);
    assert!(
        matches!(
            &action,
            mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
                switch_only: true,
                ..
            }
        ),
        "the capable prompt stores the switch answer, got {action:?}"
    );

    app.apply_confirm_action(action, key(Key::Esc));

    assert_switch_sent(&cmd_rx, kind);
}

/// The capable prompt's Enter answer keeps today's continue hand-off: the
/// output selection reaches the Owner and no switch does.
#[test]
fn the_capable_prompt_enter_still_sends_the_continue_selection() {
    let (mut app, cmd_rx) = switch_capable_app(SwitchTargetKind::Cast);
    raise_switch_prompt(&mut app, SwitchTargetKind::Cast);

    app.apply_confirm_action(pending_confirm_action(&app), key(Key::Enter));

    assert_selection_sent(&cmd_rx, &expected_selection(SwitchTargetKind::Cast));
}

/// An Owner without the output switch keeps today's cancel: Esc sends
/// nothing and the current output keeps playing (design D6).
#[test]
fn an_owner_without_the_output_switch_keeps_the_cancel() {
    let (mut app, cmd_rx) = cast_output_owner_app(true, false);
    raise_switch_prompt(&mut app, SwitchTargetKind::Cast);

    app.apply_confirm_action(pending_confirm_action(&app), key(Key::Esc));

    assert!(
        cmd_rx.try_recv().is_err(),
        "No cancels: the Owner receives nothing and the output keeps playing"
    );
}

/// The local switch answer reports the switch: the remote output stops and
/// local playback becomes the output with nothing playing.
#[test]
fn the_local_switch_answer_reports_the_switch() {
    let (mut app, _cmd_rx) = switch_capable_app(SwitchTargetKind::Local);
    raise_switch_prompt(&mut app, SwitchTargetKind::Local);

    app.apply_confirm_action(pending_confirm_action(&app), key(Key::Esc));

    assert_eq!(app.newest_toast_text(), "Switched to local playback");
}

/// A Cast selection with no active slot selects without asking
/// (confirm-output-handoff "No prompt when nothing is active").
#[test]
fn a_cast_selection_with_no_active_slot_selects_without_asking() {
    let (mut app, cmd_rx) = cast_output_owner_app(false, false);

    app.select_panel_target(PanelTarget::Cast(mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }));

    assert!(
        !matches!(
            app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
        ),
        "with no active slot there is nothing to hand off, so no prompt is raised"
    );
    assert!(matches!(
        cmd_rx.try_recv().unwrap(),
        mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast { .. })
    ));
}

/// Selecting the local target while a remote output holds an active slot
/// asks first: No cancels and the remote output keeps playing, Yes hands the
/// output back to Local (confirm-output-handoff).
#[test]
fn detaching_to_local_with_an_active_remote_output_asks_first() {
    let (mut app, cmd_rx) = cast_output_owner_app(true, false);
    let output = mbv_ctrl::OutputState::Cast {
        receiver_id: "receiver-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    };
    app.player
        .remote()
        .update_playback_output_for_test(output.clone());
    app.playback_output = Some(output);

    app.detach_to_local();

    let action = pending_confirm_action(&app);
    assert_eq!(
        action,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: None,
            name: mbv_emby::device_name(),
            switch_only: false,
        }
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "raising the gate sends nothing: the receiver keeps playing while the user answers"
    );

    app.apply_confirm_action(action.clone(), key(Key::Esc));

    assert!(
        cmd_rx.try_recv().is_err(),
        "No cancels: the receiver keeps playing"
    );

    app.apply_confirm_action(action, key(Key::Enter));

    assert!(matches!(
        cmd_rx.try_recv().unwrap(),
        mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Local)
    ));
}

/// A thin-client app on a packaged `mbvd` peer: the home Owner is the
/// local-daemon stub, which advertises both output capabilities as the Local
/// Owner does, suspended behind a direct-remote peer link whose hello
/// carries neither (the packaged hello). Commands the peer receives arrive
/// on the peer receiver; commands the home Owner receives arrive on the home
/// receiver.
fn thin_client_app_on_mbvd() -> (
    crate::app::App,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut home_compat = mbv_ctrl::CtrlCompatibility::current();
    home_compat.supports_playback_output = true;
    home_compat.supports_emby_session_output = true;
    let (mut app, home_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), home_compat);
    let (mut peer, _peer_rx, peer_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    peer.set_ctrl_compatibility_for_test(mbv_ctrl::CtrlCompatibility::current());
    let sess = make_session("mbvd-host", "mbv");
    let (_dropped_tx, attach_rx) = mpsc::channel();
    app.switch_to_direct_remote(
        &sess,
        peer,
        attach_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    while home_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}
    (app, home_cmd_rx, peer_cmd_rx)
}

/// `thin_client_app_on_mbvd` whose suspended home link reports an active
/// slot on a Cast output — the slot a hand-off prompt offers to move.
fn thin_client_app_with_cast_home_output() -> (
    crate::app::App,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let (app, home_cmd_rx, peer_cmd_rx) = thin_client_app_on_mbvd();
    let home = app.suspended_local.as_ref().unwrap();
    home.player
        .remote()
        .update_status(|status| status.active = true);
    home.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::Cast {
            receiver_id: "receiver-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        });
    while home_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}
    (app, home_cmd_rx, peer_cmd_rx)
}

/// While thin-client on a peer, Yes on the direct-endpoint hand-off fork
/// severs to the home Owner and moves the home Owner's slot: the peer
/// receives no command and the home Owner's link carries the selection
/// (confirm-output-handoff "Yes runs the hand-off unchanged" — the prompt
/// was raised for the home Owner's slot, so its Owner performs the
/// hand-off).
#[test]
fn a_confirmed_hand_off_fork_while_thin_client_sends_the_selection_to_the_home_owner() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = thin_client_app_with_cast_home_output();
    let mut target = make_session("remote-host", "mbv");
    target.host = "192.168.1.20".into();
    target.device_id = "device-direct".into();
    target.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    app.panel_targets = vec![PanelTarget::Emby(Box::new(target.clone()))];

    app.activate_session_target(&target);

    let modal = pending_confirm_modal(&app);
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
            device_id: "device-direct".to_string(),
            name: "remote-host".to_string(),
        }
    );
    assert!(
        peer_cmd_rx.try_recv().is_err() && home_cmd_rx.try_recv().is_err(),
        "raising the fork sends nothing to either owner"
    );

    app.apply_confirm_action(modal.on_confirm.clone(), key(Key::Enter));

    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "Yes leaves the peer's output alone"
    );
    assert!(
        matches!(
            home_cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ..
            }) if device_id == "device-direct"
        ),
        "Yes hands the home Owner's slot to the Session as its output"
    );
    assert!(
        app.is_local_daemon() && app.suspended_local.is_none(),
        "the answer severed back to the home Owner before selecting"
    );
}

/// While thin-client on a peer, Yes on the local target's continue-playback
/// prompt hands the home Owner back to local: the peer receives no playback
/// command and the home Owner's Cast output is handed back to Local
/// (confirm-output-handoff "Selecting local playback asks while a remote
/// output plays" — the gate and the detach body read the same fact, the
/// home Owner's output, not the peer's mirror).
#[test]
fn a_confirmed_local_detach_while_thin_client_hands_the_home_owner_back_to_local() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = thin_client_app_with_cast_home_output();

    app.detach_to_local();

    let modal = pending_confirm_modal(&app);
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: None,
            name: mbv_emby::device_name(),
            switch_only: false,
        }
    );
    assert!(
        peer_cmd_rx.try_recv().is_err() && home_cmd_rx.try_recv().is_err(),
        "raising the gate sends nothing: the receiver keeps playing while the user answers"
    );

    app.apply_confirm_action(modal.on_confirm.clone(), key(Key::Enter));

    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "Yes leaves the peer's output alone"
    );
    assert!(
        matches!(
            home_cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Local)
        ),
        "Yes hands the home Owner's remote output back to Local"
    );
    assert!(
        app.is_local_daemon() && app.suspended_local.is_none(),
        "the detach restored the home Owner"
    );
    assert!(
        app.newest_toast_text()
            .contains("Detached from cast target"),
        "the detach reports the hand-back, got {:?}",
        app.newest_toast_text()
    );
}

/// Regression: while this Client controls packaged `mbvd`, activating an
/// Emby Session without a direct endpoint was refused with "This Player
/// owner cannot select an Emby Session", because the gate read mbvd's hello
/// instead of the local Owner's. The switch severs to the local Owner, so the
/// local Owner's capability decides: the selection reaches the home Owner and
/// mbvd receives nothing.
#[test]
fn a_session_activation_while_controlling_mbvd_selects_it_on_the_home_owner() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = thin_client_app_on_mbvd();
    // Nothing plays on the home Owner, so no hand-off prompt comes first.
    app.suspended_local
        .as_ref()
        .unwrap()
        .player
        .remote()
        .update_status(|status| status.active = false);
    let mut target = make_session("Living Room TV", "Emby");
    target.device_id = "device-tv".into();
    app.panel_targets = vec![PanelTarget::Emby(Box::new(target.clone()))];

    app.activate_session_target(&target);

    assert!(peer_cmd_rx.try_recv().is_err(), "mbvd receives no command");
    assert!(
        matches!(
            home_cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ..
            }) if device_id == "device-tv"
        ),
        "the home Owner selects the Session as its output, status {:?}",
        app.newest_toast_text()
    );
    assert!(
        app.is_local_daemon() && app.suspended_local.is_none(),
        "the activation severed mbvd and restored the home Owner"
    );
}
