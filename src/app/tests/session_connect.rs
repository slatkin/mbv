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
            connection: mbv_ctrl::CastConnection::Connected,
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
            connection: mbv_ctrl::CastConnection::Disconnected,
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
