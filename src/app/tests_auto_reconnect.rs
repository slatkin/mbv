use super::*;
use crate::app::tests::*;

#[test]
fn try_auto_reconnect_restores_a_persisted_library_route() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
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
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());

    app.try_auto_reconnect();

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

#[test]
fn try_auto_reconnect_falls_back_to_local_when_route_no_longer_configured() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    // No `library_routes` entry for "music" this time -- config changed
    // since the last exit.

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_restores_a_persisted_direct_session() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
    fn sessions_with_living_room(
        _client: &mbv_core::api::EmbyClient,
    ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        Ok(vec![make_session("living-room-mbv", "mbv")])
    }
    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_with_living_room);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.connected_session_id.as_deref(), Some("sess-1"));
}

#[test]
fn try_auto_reconnect_falls_back_to_local_when_device_not_found() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
    fn sessions_without_living_room(
        _client: &mbv_core::api::EmbyClient,
    ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        Ok(vec![])
    }
    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_without_living_room);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
    assert!(app.connected_session_id.is_none());
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
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_is_a_no_op_when_nothing_was_persisted() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}
