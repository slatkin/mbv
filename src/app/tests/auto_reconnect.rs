use super::*;

#[test]
fn try_auto_reconnect_falls_back_to_local_when_route_no_longer_configured() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    // No `library_routes` entry for "music" this time -- config changed
    // since the last exit.

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_is_a_no_op_when_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.config.lock().unwrap().auto_reconnect);
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn new_remote_restores_a_persisted_route_when_attached_to_the_local_daemon() {
    // #fix-local-daemon-auto-reconnect: a local-daemon-attach launch
    // (`App::new_remote(..., is_local_daemon: true)`) must restore a saved
    // auto-reconnect target during construction itself, without a separate
    // manual `try_auto_reconnect()` call -- mirrors bare mode's `App::new`.
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

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
    let client = mbv_core::api::EmbyClient::new(config.clone());
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);

    let app = App::new_remote_with_config(
        client,
        remote,
        player_rx,
        &mbv_core::remote_player::DaemonEndpoint::Local,
        config,
    );

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

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
    let client = mbv_core::api::EmbyClient::new(config.clone());
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);

    let app = App::new_remote_with_config(
        client,
        remote,
        player_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );

    assert!(app.active_route.is_none());
}
