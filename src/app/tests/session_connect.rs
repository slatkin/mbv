use crate::app::state::panel_targets::PanelTarget;
use crate::app::tests::*;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

#[test]
fn session_direct_endpoint_prefers_advertised_tcp_port() {
    let app = make_app_stub();
    let mut sess = make_session("remote-host", "mbv");
    sess.host = "192.168.1.20".into();
    sess.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        ))
    );
}

/// Design D6 (review qht-6r P1): this device's own row never counts as a
/// direct-endpoint transfer target, even when the local daemon advertises
/// `mbv-direct-tcp-port` — `session_direct_endpoint` prefers the advertised
/// TCP endpoint over the Local device-name match, so the projection keys on
/// the own row itself, not on the endpoint kind.
#[test]
fn the_own_row_has_no_direct_endpoint_even_when_the_local_daemon_advertises_a_port() {
    let mut app = make_app_stub();
    let mut client = mbv_emby::EmbyClient::new(crate::config::Config::default());
    client.device_name = "this-host".into();
    app.emby_runtime.client = Some(std::sync::Arc::new(std::sync::Mutex::new(client)));

    let mut own = make_session("this-host", "mbv");
    own.host = "127.0.0.1".into();
    own.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    assert!(
        !app.session_has_direct_endpoint(&own),
        "the own row is never a transfer target, whatever it advertises"
    );

    // The same advertisement on another device's row is a peer.
    let mut peer = make_session("other-host", "mbv");
    peer.host = "192.168.1.20".into();
    peer.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    assert!(
        app.session_has_direct_endpoint(&peer),
        "a different device advertising the port is a peer"
    );
}

#[test]
fn playback_output_event_stores_cast_state_including_disconnected() {
    // cast-owner-output design D13 (rows 8.1/8.2 A): the App mirrors the
    // last `CtrlEvent::PlaybackOutput`, and a `Disconnected` Cast still
    // counts as Cast (the cast-session-control spec keeps the target
    // presented as disconnected and the selection usable).
    let mut app = make_app_stub();

    app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
        mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
    ));
    assert!(
        app.output_is_cast(),
        "a connected Cast report makes the output Cast"
    );

    app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
        mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Disconnected,
        },
    ));
    assert!(
        app.output_is_cast(),
        "a disconnected Cast target is still presented as the output"
    );

    app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
        mbv_ctrl::OutputState::Local,
    ));
    assert!(
        !app.output_is_cast(),
        "a Local report clears the Cast output"
    );
}

/// User decision (session-switch playback continuity): connecting to a new
/// session severs only the control link — the local Owner's Cast output
/// keeps playing, so no `Stop` and no output change are sent. A hand-off is
/// always the owner's own output selection, asked for separately.
#[test]
fn connecting_to_a_session_leaves_the_local_owners_cast_output_playing() {
    // cast-owner-output row 8.2, design D13, spec scenario "A session or
    // direct remote is connected while a cast target is attached", reversed
    // by the session-switch playback-continuity decision: the Cast state
    // here is seeded the way the handshake delivers it — onto the
    // connection itself, before any PlayerEvent drain — exactly the
    // launch-auto-reconnect shape.
    let mut app = make_app_stub();
    let cast_rx = live_owner_channel(&mut app);
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        });
    app.connect_to_session(&make_session("remote-host", "mbv"));
    assert!(
        cast_rx.try_recv().is_err(),
        "a connect sends nothing to the local Owner: the receiver keeps playing"
    );
}

/// The same continuity contract for the Emby Session output: connecting to
/// a session sends no `Stop` and no `SelectPlaybackOutput(Local)` — the
/// Session keeps playing behind the new attachment.
#[test]
fn connecting_to_a_session_leaves_the_local_owners_emby_session_output_playing() {
    let mut app = make_app_stub();
    let cmd_rx = live_owner_channel(&mut app);
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::EmbySession {
            device_id: "device-1".to_string(),
            name: "Living Room TV".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
            audio_only: false,
            now_playing: None,
        });
    app.connect_to_session(&make_session("remote-host", "mbv"));
    assert!(
        cmd_rx.try_recv().is_err(),
        "a connect sends nothing to the local Owner: the Session keeps playing"
    );
}

/// The session-switch hand-off fork's shared setup: an owner that advertises
/// both output capabilities, attached as the local Owner, whose retained
/// output is the Emby Session of device-1.
fn session_output_owner_app(
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
    compatibility.supports_emby_session_output = true;
    remote.set_ctrl_compatibility_for_test(compatibility);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = player_rx;
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::EmbySession {
            device_id: "device-1".to_string(),
            name: "Living Room TV".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
            audio_only: false,
            now_playing: None,
        });
    app.player
        .remote()
        .update_status(|status| status.active = active);
    (app, cmd_rx)
}

/// A direct-endpoint session row: another device's mbv Owner advertising its
/// TCP endpoint.
fn direct_endpoint_session(device_id: &str, device_name: &str) -> mbv_emby::SessionInfo {
    let mut sess = make_session(device_name, "mbv");
    sess.id = format!("sess-{device_id}");
    sess.device_id = device_id.to_string();
    sess.host = "192.168.1.20".into();
    sess.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    sess
}

/// The session-switch hand-off fork (user decision): with a slot active on
/// the local Owner's Emby Session output, activating a different Session
/// that advertises a direct endpoint raises the shared confirm fork and
/// sends nothing — neither a `Stop` against the prior playback nor a
/// connect.
#[test]
fn activating_a_session_with_active_playback_raises_the_hand_off_fork() {
    let (mut app, cmd_rx) = session_output_owner_app(true);
    let target = direct_endpoint_session("device-2", "Bedroom mbv");

    app.activate_session_target(&target);

    let modal = match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => modal,
        other => panic!("the fork must raise the confirm modal, got {other:?}"),
    };
    assert!(
        modal.message.contains("Bedroom mbv"),
        "the fork names the target, got {:?}",
        modal.message
    );
    assert_eq!(
        modal.on_confirm,
        mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
            device_id: "device-2".to_string(),
            name: "Bedroom mbv".to_string(),
        }
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "raising the fork sends nothing: the prior playback continues while the user answers"
    );
}

/// The fork's Enter answer: the Session becomes the local Owner's playback
/// output, and the Owner performs the hand-off — the only command is the
/// output selection, with no `Stop` before it.
#[test]
fn the_fork_enter_answer_selects_the_session_as_the_owner_output() {
    let (mut app, cmd_rx) = session_output_owner_app(true);
    app.panel_targets = vec![
        PanelTarget::Emby(Box::new(direct_endpoint_session(
            "device-1",
            "Living Room TV",
        ))),
        PanelTarget::Emby(Box::new(direct_endpoint_session("device-2", "Bedroom mbv"))),
    ];

    app.apply_confirm_action(
        mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
            device_id: "device-2".to_string(),
            name: "Bedroom mbv".to_string(),
        },
        KeyEvent::new(Key::Enter, KeyModifiers::NONE),
    );

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::EmbySession {
                ref device_id,
                ..
            }) if device_id == "device-2"
        ),
        "Enter selects the Session as the owner's output; the owner performs the hand-off"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the answer sends exactly the output selection"
    );
}

/// The fork's Esc answer: the plain switch runs — the direct endpoint
/// becomes the attachment and the local Owner receives nothing, so the
/// prior Session keeps playing.
#[test]
fn the_fork_esc_answer_switches_without_stopping_the_prior_playback() {
    use crate::app::{DIRECT_CONNECT_OVERRIDE, DIRECT_CONNECT_TEST_LOCK};

    let _guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(|_endpoint| {
        let (remote, rx, _cmds) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        Ok((remote, rx))
    });
    let (mut app, cmd_rx) = session_output_owner_app(true);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(direct_endpoint_session(
        "device-2",
        "Bedroom mbv",
    )))];

    app.apply_confirm_action(
        mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
            device_id: "device-2".to_string(),
            name: "Bedroom mbv".to_string(),
        },
        KeyEvent::new(Key::Esc, KeyModifiers::NONE),
    );
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert_eq!(
        app.player_endpoint,
        Some(mbv_remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        )),
        "Esc switches to the Session's direct endpoint"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the plain switch sends nothing to the local Owner: the prior playback continues"
    );
}

/// Nothing is active on the owner's output: activating the Session skips
/// the fork entirely and runs the plain switch — with no slot to move, the
/// activation just switches the attachment.
#[test]
fn activating_a_session_with_no_active_slot_skips_the_fork() {
    use crate::app::{DIRECT_CONNECT_OVERRIDE, DIRECT_CONNECT_TEST_LOCK};

    let _guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(|_endpoint| {
        let (remote, rx, _cmds) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        Ok((remote, rx))
    });
    let (mut app, cmd_rx) = session_output_owner_app(false);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(direct_endpoint_session(
        "device-2",
        "Bedroom mbv",
    )))];

    app.activate_session_target(&direct_endpoint_session("device-2", "Bedroom mbv"));
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(
        !matches!(
            app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
        ),
        "with no active slot there is nothing to hand off, so no fork is raised"
    );
    assert_eq!(
        app.player_endpoint,
        Some(mbv_remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        )),
        "the plain switch runs"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the switch sends nothing to the local Owner"
    );
}

/// The target device already being the owner's output skips the fork: that
/// selection is the owner's no-op hand-off, so the activation runs the plain
/// switch.
#[test]
fn activating_the_already_output_device_skips_the_fork() {
    use crate::app::{DIRECT_CONNECT_OVERRIDE, DIRECT_CONNECT_TEST_LOCK};

    let _guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(|_endpoint| {
        let (remote, rx, _cmds) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        Ok((remote, rx))
    });
    let (mut app, cmd_rx) = session_output_owner_app(true);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(direct_endpoint_session(
        "device-1",
        "Living Room mbv",
    )))];

    app.activate_session_target(&direct_endpoint_session("device-1", "Living Room mbv"));
    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(
        !matches!(
            app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
        ),
        "the output's own device never forks: that selection is the owner's no-op hand-off"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the switch sends nothing to the local Owner"
    );
}
