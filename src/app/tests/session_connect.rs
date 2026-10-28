use crate::app::tests::*;

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

#[test]
fn session_connect_hands_a_cast_output_back_to_the_local_owner() {
    // cast-owner-output row 8.2, design D13, spec scenario "A session or
    // direct remote is connected while a cast target is attached" — and the
    // 8.2 review fix: at launch, auto-reconnect connects (and severs)
    // before the shell drains any PlayerEvent, so the handback decision
    // reads the owner connection's RETAINED output state, not the drained
    // mirror. The Cast state here is seeded the way the handshake now
    // delivers it — onto the connection itself — while the mirror is still
    // None: exactly the launch-auto-reconnect shape.
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
    assert!(
        !app.output_is_cast(),
        "no PlayerEvent has been drained yet — the mirror is still None, and the handback must fire anyway (launch auto-reconnect)"
    );
    app.connect_to_session(&make_session("remote-host", "mbv"));
    assert!(
        matches!(cast_rx.try_recv().unwrap(), mbv_ctrl::CtrlCmd::Stop),
        "the handback stops the receiver first"
    );
    assert!(
        matches!(
            cast_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Local)
        ),
        "the handback then selects Local as the owner's output"
    );
    assert!(
        cast_rx.try_recv().is_err(),
        "the handback sends exactly the two commands"
    );

    let mut app = make_app_stub();
    let local_rx = live_owner_channel(&mut app);
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    app.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::Local);
    app.connect_to_session(&make_session("remote-host", "mbv"));
    assert!(
        local_rx.try_recv().is_err(),
        "a connect on a Local output sends no handback command"
    );
}

/// emby-session-owner-output row 7.1, design D12, cast-session-control spec
/// "A session or direct remote is connected while a cast target is attached",
/// extended to the Emby Session output: `hand_remote_output_back_to_local`
/// sends `Stop` first and then `SelectPlaybackOutput(Local)`.
#[test]
fn session_connect_hands_an_emby_session_output_back_to_the_local_owner() {
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
        matches!(cmd_rx.try_recv().unwrap(), mbv_ctrl::CtrlCmd::Stop),
        "the handback stops the Emby Session first"
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Local)
        ),
        "the handback then selects Local as the owner's output"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the handback sends exactly the two commands"
    );
}
