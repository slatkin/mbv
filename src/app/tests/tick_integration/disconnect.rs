use crate::app::QueueScope;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{emby_unified_state, make_items, make_local_daemon_app_stub};
use mbv_ctrl::player::CONNECTION_LOST_MESSAGE;
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};
use std::sync::mpsc;

#[test]
fn suspended_home_snapshot_updates_local_queue_while_remote_is_viewed() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (remote, _) = RemotePlayer::stub(make_items(2), 0);
    app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("remote-owner", "mbv"),
        remote,
        mpsc::channel().1,
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = event_rx;
    {
        let mut status = app.player.status.lock().unwrap();
        status.position_ticks = 9876;
        status.current_idx = 1;
        status.active = true;
        status.title = "Remote status".into();
    };
    let snapshot = |items: Vec<mbv_emby_model::EmbyItem>, source: mbv_queue::QueueSource| {
        let mut state = emby_unified_state(&items, 0);
        state.source = source;
        state
    };
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::UnifiedQueueUpdated(
            Box::new(snapshot(make_items(2), mbv_queue::QueueSource::Album)),
        ))
        .unwrap();
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(1),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(Box::new(snapshot(
                make_items(3),
                mbv_queue::QueueSource::Series,
            ))),
        })
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
    assert_eq!(app.player_tab.emby_items(), make_items(3));
    assert!(matches!(&app.queue_source, mbv_queue::QueueSource::Series));
    let status = app.player.status.lock().unwrap();
    assert_eq!(status.position_ticks, 9876);
    assert_eq!(status.current_idx, 1);
    assert!(status.active);
    assert_eq!(status.title, "Remote status");
}

#[test]
fn suspended_home_disconnect_raises_owner_lost_modal() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (remote, _) = RemotePlayer::stub(make_items(2), 0);
    app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("remote-owner", "mbv"),
        remote,
        mpsc::channel().1,
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = event_rx;
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::RemoteDisconnected(
            "home owner lost".into(),
        ))
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    assert!(
        harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Modal(
                mbv_ui_msg::ModalId::DaemonLost,
            ))
    );
}

#[test]
fn tick_remote_disconnect_restores_local_daemon_queue_and_surfaces_toast() {
    fn reconnect_local_daemon(
        _endpoint: &DaemonEndpoint,
    ) -> crate::app::test_seams::DaemonRouteConnectOutcome {
        let (remote, events) = RemotePlayer::stub(make_items(2), 0);
        crate::app::test_seams::DaemonRouteConnectOutcome::Connected(remote, events)
    }

    let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local_daemon);

    let local_items = make_items(2);
    let mut app = make_local_daemon_app_stub(local_items.clone());
    // This test covers the disconnect projection, not an unrelated failed
    // Emby refresh that `refresh_after_stop` may perform.
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::default();
    let (remote, _) = RemotePlayer::stub(make_items(3), 0);
    let (event_tx, event_rx) = mpsc::channel();
    let endpoint = DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("remote-owner", "mbv"),
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
        .send(mbv_ctrl::player::PlayerEvent::RemoteDisconnected(
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
