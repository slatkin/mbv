use crate::app::QueueLink;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{
    QueueViewTestExt, emby_unified_state, make_items, make_local_daemon_app_stub,
};
use mbv_ctrl::player::CONNECTION_LOST_MESSAGE;
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};
use mbv_ui_msg::{Msg, ShellRequest};
use std::sync::mpsc;

fn reconnect_local_daemon(
    _endpoint: &DaemonEndpoint,
) -> (
    RemotePlayer,
    std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
) {
    RemotePlayer::stub(make_items(2), 0)
}

#[test]
fn suspended_home_snapshot_updates_local_queue_while_remote_is_viewed() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.switch_to_direct_remote_for_test(
        "remote-owner",
        make_items(2),
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.set_suspended_local_events_for_test(event_rx);
    app.seed_playing_position_for_test(1, 9876, None);
    app.set_player_title_for_test("Remote status".to_string());
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
    assert_eq!(app.playing_queue_link(), QueueLink::Peer);
    let home_queue = app.queue_for_link(QueueLink::Local);
    assert_eq!(home_queue.emby_items(), make_items(3));
    assert!(matches!(
        home_queue.source(),
        mbv_queue::QueueSource::Series
    ));
    let status = app.player_status_snapshot();
    assert_eq!(status.position_ticks, 9876);
    assert_eq!(status.current_idx, 1);
    assert!(status.active);
    assert_eq!(status.title, "Remote status");
}

#[test]
fn suspended_home_disconnect_raises_owner_lost_modal() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.switch_to_direct_remote_for_test(
        "remote-owner",
        make_items(2),
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.set_suspended_local_events_for_test(event_rx);
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
fn suspended_home_shutdown_announced_exits_cleanly() {
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.switch_to_direct_remote_for_test(
        "remote-owner",
        make_items(2),
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.set_suspended_local_events_for_test(event_rx);
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::DaemonShutdownAnnounced)
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    assert!(
        !harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Modal(
                mbv_ui_msg::ModalId::DaemonLost,
            ))
    );
    assert!(!harness.model().app.home_link_is_suspended_for_test());
    assert_eq!(
        harness
            .model_mut()
            .app
            .take_pending_exit_message()
            .as_deref(),
        Some("mbv: the Owner process was stopped — exiting.")
    );
    assert!(crate::app::QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

// Owns queue-owner-process D4: shutting down a routed remote restores the live home link.
#[test]
fn routed_remote_shutdown_announced_restores_suspended_home_without_quitting() {
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.switch_to_direct_remote_for_test(
        "remote-owner",
        make_items(2),
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.set_active_player_events_for_test(event_rx);
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::DaemonShutdownAnnounced)
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.is_local_daemon());
    assert!(!app.home_link_is_suspended_for_test());
    assert!(!app.player_remote_disconnected());
    assert!(
        harness
            .model_mut()
            .app
            .take_pending_exit_message()
            .is_none()
    );
    assert!(!crate::app::QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn local_home_shutdown_announced_exits_cleanly() {
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (event_tx, event_rx) = mpsc::channel();
    app.set_active_player_events_for_test(event_rx);
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::DaemonShutdownAnnounced)
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    assert!(
        !harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Modal(
                mbv_ui_msg::ModalId::DaemonLost,
            ))
    );
    assert_eq!(
        harness
            .model_mut()
            .app
            .take_pending_exit_message()
            .as_deref(),
        Some("mbv: the Owner process was stopped — exiting.")
    );
    assert!(crate::app::QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn suspended_home_disconnect_clears_link_before_local_fallthrough_reconnects() {
    use crate::app::{DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK};

    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local_daemon);
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.switch_to_direct_remote_for_test(
        "remote-owner",
        make_items(2),
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (event_tx, event_rx) = mpsc::channel();
    app.set_suspended_local_events_for_test(event_rx);
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::RemoteDisconnected(
            "home owner lost".into(),
        ))
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    assert!(!harness.model().app.home_link_is_suspended_for_test());
    let prepared = harness
        .model_mut()
        .app
        .prepare_local_player()
        .unwrap()
        .unwrap();
    assert_eq!(
        prepared
            .player
            .remote()
            .unified_queue_state()
            .unwrap()
            .slots
            .len(),
        2
    );
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn tick_remote_disconnect_restores_local_daemon_queue_and_surfaces_toast() {
    let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local_daemon);

    let local_items = make_items(2);
    let mut app = make_local_daemon_app_stub(local_items.clone());
    // This test covers the disconnect projection, not an unrelated failed
    // Emby refresh that `refresh_after_stop` may perform.
    app.clear_emby_runtime_for_test();
    let (remote, _peer_events, peer_cmd_rx) = RemotePlayer::stub_with_command_rx(make_items(3), 0);
    let (event_tx, event_rx) = mpsc::channel();
    let endpoint = DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.attach_direct_remote_for_test("remote-owner", remote, mpsc::channel().1, &endpoint);
    // The production attach wraps the link in the wake relay (design D7 of
    // `extract-client-core`), which forwards asynchronously; the injected
    // event must drain synchronously inside `step`, so the test drives the
    // raw channel like the other disconnect tests do.
    app.set_active_player_events_for_test(event_rx);

    assert!(app.has_remote_view_for_test());
    assert_eq!(app.playing_queue_link(), QueueLink::Peer);
    assert_eq!(app.displayed_queue().emby_items().len(), 3);

    let mut harness = TickHarness::new(app);
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::RemoteDisconnected(
            CONNECTION_LOST_MESSAGE.to_string(),
        ))
        .expect("send remote disconnect event");
    harness.step();

    let app = &harness.model().app;
    assert!(!app.has_remote_view_for_test());
    assert_eq!(app.playing_queue_link(), QueueLink::Local);
    assert_eq!(app.displayed_queue().emby_items().len(), local_items.len());
    assert_eq!(app.displayed_queue().emby_items(), local_items);
    assert_eq!(app.newest_toast_text(), CONNECTION_LOST_MESSAGE);
    // The Client cannot command a link it already lost, but the contract
    // also forbids a last-moment Stop on the way down: the disconnect
    // handling must tear the link down without sending anything (the peer
    // keeps playing whatever it was playing).
    while let Ok(cmd) = peer_cmd_rx.try_recv() {
        assert!(!matches!(cmd, mbv_ctrl::CtrlCmd::Stop));
    }

    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
}

// Owns queue-handoff-and-take 5.2 (remote-queue-disconnect delta): a user
// disconnect (`d`) shows the local Owner queue and leaves the peer playing —
// the peer link receives no `Stop` ("A user disconnect SHALL NOT stop or
// change the remote playback or queue").
#[test]
fn user_disconnect_shows_local_queue_and_sends_the_peer_no_stop() {
    let local_items = make_items(1);
    let mut app = make_local_daemon_app_stub(local_items.clone());
    let (remote, _peer_events, peer_cmd_rx) = RemotePlayer::stub_with_command_rx(make_items(2), 0);
    app.attach_direct_remote_for_test(
        "remote-owner",
        remote,
        mpsc::channel().1,
        &DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert_eq!(app.playing_queue_link(), QueueLink::Peer);
    // The stub owner plays by default; the no-Stop contract is about a
    // playing peer.
    assert!(app.player_status_snapshot().active);

    let mut harness = TickHarness::new(app);
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::DetachSessions)),
        &mut music_resize,
        &mut tv_resize,
    );

    let app = &harness.model().app;
    assert!(!app.direct_remote_connected_for_test());
    assert_eq!(app.playing_queue_link(), QueueLink::Local);
    assert_eq!(app.displayed_queue().emby_items(), local_items);
    while let Ok(cmd) = peer_cmd_rx.try_recv() {
        assert!(
            !matches!(cmd, mbv_ctrl::CtrlCmd::Stop),
            "a user disconnect must not stop the peer"
        );
    }
}
