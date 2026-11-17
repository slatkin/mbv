//! The shared continue-playback gate's cancel and local-target contracts
//! (confirm-output-handoff): No sends nothing and leaves the output alone,
//! and the local target asks while a remote output holds an active slot.
//! The ask-until-Yes and nothing-active-skip contracts live with the
//! selection flows that exercise them (`session_connect`,
//! `tick_integration::sessions`, `input::gui::tests::target_chooser`).

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
