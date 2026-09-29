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
