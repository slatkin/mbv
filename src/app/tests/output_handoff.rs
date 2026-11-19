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

fn key(code: Key) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// An owner attached as the local Owner that advertises `playback-output`,
/// holding one cast receiver, with `active` marking the stub slot active.
fn cast_output_owner_app(
    active: bool,
) -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut app = make_app_stub();
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_playback_output = true;
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

/// A Cast selection with a slot active on the current output asks first, and
/// No cancels: no command reaches the Owner and the current output keeps
/// playing (confirm-output-handoff).
#[test]
fn a_declined_cast_hand_off_sends_nothing_and_keeps_the_output() {
    let (mut app, cmd_rx) = cast_output_owner_app(true);

    app.select_panel_target(PanelTarget::Cast(mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }));

    let modal = match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal,
        other => panic!("the gate must raise the confirm modal, got {other:?}"),
    };
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

/// A Cast selection with no active slot selects without asking
/// (confirm-output-handoff "No prompt when nothing is active").
#[test]
fn a_cast_selection_with_no_active_slot_selects_without_asking() {
    let (mut app, cmd_rx) = cast_output_owner_app(false);

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
    let (mut app, cmd_rx) = cast_output_owner_app(true);
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

    let action = match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal.on_confirm.clone(),
        other => panic!("the gate must raise the confirm modal, got {other:?}"),
    };
    assert_eq!(
        action,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: None,
            name: mbv_emby::device_name(),
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

    let modal = match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal,
        other => panic!("the hand-off fork must be raised, got {other:?}"),
    };
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

    let modal = match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal,
        other => panic!("the gate must raise the confirm modal, got {other:?}"),
    };
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target: None,
            name: mbv_emby::device_name(),
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
        app.status.contains("Detached from cast target"),
        "the detach reports the hand-back, got {:?}",
        app.status
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
        app.status
    );
    assert!(
        app.is_local_daemon() && app.suspended_local.is_none(),
        "the activation severed mbvd and restored the home Owner"
    );
}
