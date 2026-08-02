use super::*;
use crate::app::tests::*;

#[test]
fn session_direct_endpoint_prefers_advertised_tcp_port() {
    let app = make_app_stub();
    let mut sess = make_session("remote-host", "mbv");
    sess.host = "192.168.1.20".into();
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        ))
    );
}

#[test]
fn session_direct_endpoint_rejects_non_mbv_without_local_fallback() {
    let app = make_app_stub();
    let sess = make_session("other-host", "Emby");
    assert_eq!(app.session_direct_endpoint(&sess), None);
}

#[test]
fn session_direct_endpoint_falls_back_to_local_socket_for_same_host_session() {
    let app = make_app_stub();
    let device_name = app.client.lock().unwrap().device_name.clone();
    let sess = make_session(&device_name, "mbv");
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_core::remote_player::DaemonEndpoint::Local)
    );
}

#[test]
fn f3_direct_upgrade_with_empty_device_name_remains_disconnectable() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        mbv_core::ctrl::ConnectError,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    let mut sess = make_session("", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.remote_player_tab.is_some());
    assert!(app.connected_session_id.is_none());
    assert!(app.direct_remote_label.is_none());
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(!app.player.is_remote());
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.status, "Disconnected from direct remote session");
}

#[test]
fn connect_to_session_preserves_direct_upgrade_failure_status_after_fallback() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        mbv_core::ctrl::ConnectError,
    > {
        Err(mbv_core::ctrl::ConnectError::Other("incompatible daemon protocol version: peer=1 local=3".to_string()))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_failure);
    let mut app = make_app_stub();
    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.connected_session_id.as_deref(), Some("sess-1"));
    assert_eq!(
            app.status,
            "Direct mbv control failed: incompatible daemon protocol version: peer=1 local=3; using attached session remote-mbv"
        );
}

#[test]
fn connect_to_session_tears_down_an_active_library_route_via_restore_local_mode() {
    // Regression guard: `connect_to_session`'s direct-upgrade attempt
    // is itself gated on `!self.player.is_remote()`, so
    // `switch_to_direct_remote`'s already-remote branch is never
    // reached from here -- a bare `self.active_route = None;` right
    // before that call would be dead code for this scenario. The fix
    // is to tear down any active library route through
    // `restore_local_mode` at the top of the function instead, which
    // both clears `active_route` AND restores the suspended local
    // `Player` (via the real `switch_to_library_route` path, not a
    // manually-poked field), so the subsequent `!self.player.is_remote()`
    // check is true and the direct-upgrade attempt actually runs.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        mbv_core::ctrl::ConnectError,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    // Really go through a library route (#223), not a manually-poked
    // field, so `suspended_local` is populated the way it is in
    // production and `restore_local_mode` has real state to restore.
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());

    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    // The direct-upgrade attempt ran (not skipped) because the library
    // route's remote player was properly restored to local first, so
    // the app ends up on the Sessions-panel direct remote, not stuck
    // on the stale library-route connection.
    assert!(app.player.is_remote());
    assert!(app.direct_remote_label.is_some());
}

#[test]
fn connect_to_session_is_a_no_op_teardown_when_no_library_route_is_active() {
    // The new top-of-function teardown must not disturb the existing,
    // already-covered "plain local player" path when there is no
    // library route to tear down.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        mbv_core::ctrl::ConnectError,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());

    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    assert!(app.player.is_remote());
    assert!(app.direct_remote_label.is_some());
}
