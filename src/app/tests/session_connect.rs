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
