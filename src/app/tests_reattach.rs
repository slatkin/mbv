use super::*;
use crate::app::tests::*;
use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};

fn remote_tcp_endpoint() -> DaemonEndpoint {
    DaemonEndpoint::Tcp("127.0.0.1:1".parse().unwrap())
}

#[test]
fn reattach_is_a_noop_when_auto_reconnect_is_disabled() {
    let mut app = make_app_stub();
    app.player_endpoint = Some(remote_tcp_endpoint());
    app.config.lock().unwrap().auto_reconnect = false;

    assert!(!app.try_reattach_remote_daemon());
    assert!(!app.player.is_remote());
}

#[test]
fn reattach_skips_local_daemon_endpoint() {
    let mut app = make_app_stub();
    app.player_endpoint = Some(DaemonEndpoint::Local);
    app.config.lock().unwrap().auto_reconnect = true;

    assert!(!app.try_reattach_remote_daemon());
    assert!(!app.player.is_remote());
}

#[test]
fn reattach_reconnects_to_remote_endpoint_and_adopts_live_queue() {
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &DaemonEndpoint,
    ) -> Result<(RemotePlayer, std::sync::mpsc::Receiver<PlayerEvent>), String> {
        Ok(RemotePlayer::stub(make_items(2), 1))
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let mut app = make_app_stub();
    app.player_endpoint = Some(remote_tcp_endpoint());
    app.config.lock().unwrap().auto_reconnect = true;

    assert!(app.try_reattach_remote_daemon());

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.player.is_remote());
    assert_eq!(app.player_endpoint, Some(remote_tcp_endpoint()));
    let tab = app.remote_player_tab.as_ref().expect("remote tab");
    assert_eq!(
        tab.total_queue_len(),
        2,
        "live daemon queue must be adopted"
    );
}

#[test]
fn reattach_falls_back_when_daemon_stays_unreachable() {
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn always_fail(
        _endpoint: &DaemonEndpoint,
    ) -> Result<(RemotePlayer, std::sync::mpsc::Receiver<PlayerEvent>), String> {
        Err("connection refused".to_string())
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(always_fail);

    let mut app = make_app_stub();
    app.player_endpoint = Some(remote_tcp_endpoint());
    app.config.lock().unwrap().auto_reconnect = true;

    assert!(!app.try_reattach_remote_daemon());

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(!app.player.is_remote(), "caller must run local restore");
}
