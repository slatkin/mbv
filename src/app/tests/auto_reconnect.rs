use super::*;

#[test]
fn new_remote_does_not_auto_reconnect_for_an_explicit_remote_daemon() {
    // Regression guard for design.md's Decision 1 gating: an explicit
    // `--connect-daemon`/`daemon_client_endpoint` launch (`is_local_daemon:
    // false`) is the user stating a target directly, so it must not
    // silently override that with a different auto-reconnect target, even
    // when one is saved and `auto_reconnect` is enabled.
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut config = crate::config::Config {
        auto_reconnect: true,
        ..Default::default()
    };
    config
        .library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let client = mbv_emby::EmbyClient::new(config.clone());
    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);

    let app = App::new_remote_with_config(
        client,
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );

    assert!(app.active_route.is_none());
}

/// emby-session-owner-output row 7.3, design D12, owner-playback-output spec
/// "Saved record names a plain Session": launch auto-reconnect connects only
/// when the found Session offers a direct endpoint. A saved plain-Session
/// record selects no output and stays local.
#[test]
fn saved_plain_session_record_stays_local_without_an_output_selection() {
    use mbv_net::mock_http::MockHttp;

    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::DirectSession {
            device_name: "plain-tv".to_string(),
        },
    ));
    let mut app = make_app_stub();
    let command_rx = live_owner_channel(&mut app);
    app.config.lock().unwrap().auto_reconnect = true;
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let config = app.config.lock().unwrap().clone();
    install_test_emby(&mut app, config);
    let http = MockHttp::new();
    http.respond(
        200,
        r#"[{"Id":"sess-1","DeviceId":"device-1","DeviceName":"plain-tv","Client":"Emby","UserName":"user"}]"#,
    );
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    app.try_auto_reconnect();

    assert!(
        app.connected_session_id.is_none() && app.connected_session_state.is_none(),
        "a plain Session record attaches no Session"
    );
    assert!(
        !app.remote.direct_remote_connected,
        "a plain Session record connects no Direct remote control"
    );
    assert!(
        command_rx
            .try_iter()
            .all(|command| !matches!(command, mbv_ctrl::CtrlCmd::SelectPlaybackOutput(_))),
        "a plain Session record sends no output selection"
    );
}
