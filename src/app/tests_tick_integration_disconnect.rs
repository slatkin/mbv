use crate::app::tests::{make_items, make_local_daemon_app_stub, make_session};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::QueueScope;
use mbv_core::player::CONNECTION_LOST_MESSAGE;
use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
use std::sync::mpsc;

#[test]
fn tick_remote_disconnect_restores_local_daemon_queue_and_surfaces_toast() {
    fn reconnect_local_daemon(
        _endpoint: &DaemonEndpoint,
    ) -> Result<(RemotePlayer, mpsc::Receiver<mbv_core::player::PlayerEvent>), String> {
        Ok(RemotePlayer::stub(make_items(2), 0))
    }

    let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local_daemon);

    let local_items = make_items(2);
    let mut app = make_local_daemon_app_stub(local_items.clone());
    // This test covers the disconnect projection, not an unrelated failed
    // Emby refresh that `refresh_after_stop` may perform.
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::default();
    let (remote, _) = RemotePlayer::stub(make_items(3), 0);
    let (event_tx, event_rx) = mpsc::channel();
    let endpoint = DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(
        &make_session("remote-owner", "mbv"),
        remote,
        event_rx,
        &endpoint,
    );

    assert!(app.home_is_local_daemon);
    assert!(app.remote_player_tab.is_some());
    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.displayed_queue().emby_items().len(), 3);

    let mut harness = TickHarness::new(app);
    event_tx
        .send(mbv_core::player::PlayerEvent::RemoteDisconnected(
            CONNECTION_LOST_MESSAGE.to_string(),
        ))
        .expect("send remote disconnect event");
    harness.step();

    let app = &harness.model().app;
    assert!(app.home_is_local_daemon);
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.displayed_queue().emby_items().len(), local_items.len());
    assert_eq!(app.displayed_queue().emby_items(), local_items);
    assert_eq!(app.status, CONNECTION_LOST_MESSAGE);

    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
}
