use super::*;
use mbv_remote_player::DaemonEndpoint;

fn reconnect_local_daemon(
    _endpoint: &DaemonEndpoint,
) -> (
    mbv_remote_player::RemotePlayer,
    std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
) {
    mbv_remote_player::RemotePlayer::stub(make_items(2), 0)
}

pub(super) fn stub_endpoint() -> DaemonEndpoint {
    DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap())
}

// Row 4.1/design D1 deleted Bare: every launch attaches, so RemoteSlotState::Off has no test.

#[test]
fn app_stub_starts_with_no_active_library_route() {
    let app = make_app_stub();
    assert!(app.active_route.is_none());
    assert!(app.library_routes.is_empty());
    assert!(app.library_route_cache.is_empty());
}

#[test]
fn remote_slot_state_is_attached_session_when_connected_to_remote_session() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());

    assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
    assert!(app.can_disconnect_remote());
}

#[test]
fn remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect() {
    let app = make_remote_app_stub(make_items(2), make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
}

// Connected to an mbv daemon: the peer's queue is the displayed queue,
// empty or not — a connect must never leave the client's own queue on
// screen (that was the behavior that hid the daemon's queue until the
// client pushed content).
#[test]
fn direct_remote_connect_shows_the_peer_queue_even_when_empty() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
    assert!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .is_empty()
    );
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

#[test]
fn direct_remote_connect_switches_to_remote_scope_when_remote_queue_has_items() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(remote_items.clone(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().emby_items()[0].id,
        remote_items[0].id
    );
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

#[test]
fn switch_to_library_route_sets_remote_queue_scope_when_daemon_has_items() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());

    assert!(app.has_direct_remote_queue());
}

#[test]
fn library_route_connect_shows_the_peer_queue_even_when_empty() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
    assert!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .is_empty()
    );
}

#[test]
fn switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap() {
    // #233: a second Direct Remote upgrade must disconnect the old remote.
    // The claim is shell-owned bookkeeping; the peer's socket reaching EOF is
    // the in-memory oracle (no real listener).
    let mut app = make_app_stub();
    let (remote_a, rx_a, daemon_a) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_direct_remote(
        &make_session("daemon-a", "mbv"),
        remote_a,
        rx_a,
        &stub_endpoint(),
    );

    let (remote_b, rx_b, _daemon_b) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_direct_remote(
        &make_session("daemon-b", "mbv"),
        remote_b,
        rx_b,
        &stub_endpoint(),
    );

    daemon_a
        .join()
        .expect("old direct-remote client socket must be shut down after the swap");
}

#[test]
fn switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap() {
    // #233: route-to-route swap must disconnect the old RemotePlayer's socket.
    let mut app = make_app_stub();
    let (remote_a, rx_a, daemon_a) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("music", remote_a, rx_a, &stub_endpoint());
    assert!(!app.player.is_remote_disconnected());

    let (remote_b, rx_b, _daemon_b) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("movies", remote_b, rx_b, &stub_endpoint());

    daemon_a
        .join()
        .expect("old library route's client socket must be shut down after the swap");
}

#[test]
fn restore_local_mode_disconnects_the_remote_before_restoring_local() {
    // #233: restore_local_mode must disconnect the old remote before restoring local.
    let mut app = make_app_stub();
    let (remote, rx, daemon) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("music", remote, rx, &stub_endpoint());
    assert!(!app.player.is_remote_disconnected());

    app.restore_local_mode("test: ending library route session");

    daemon
        .join()
        .expect("old remote's client socket must be shut down after restore_local_mode");
}

// Owns player-target-locality "Daemon target transitions update classification".
#[test]
fn restore_local_mode_reconnects_when_suspended_home_is_disconnected() {
    use crate::app::{DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK};

    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local_daemon);
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (route, route_rx) = mbv_remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_direct_remote(
        &make_session("remote-owner", "mbv"),
        route,
        route_rx,
        &stub_endpoint(),
    );
    let (dead_home, dead_home_rx, dead_home_peer) =
        mbv_remote_player::connect_stub_daemon_pair().unwrap();
    app.suspended_local.as_mut().unwrap().player =
        mbv_player::PlayerProxy::remote(dead_home.clone(), false);
    app.suspended_local.as_mut().unwrap().player_rx = dead_home_rx;
    dead_home.disconnect();
    app.suspended_local
        .as_mut()
        .unwrap()
        .player_rx
        .recv()
        .unwrap();
    dead_home_peer.join().unwrap();

    app.restore_local_mode("Local playback restored");

    assert!(app.suspended_local.is_none());
    assert!(app.is_local_daemon());
    assert_eq!(
        app.player
            .as_remote()
            .unwrap()
            .unified_queue_state()
            .unwrap()
            .slots
            .len(),
        2
    );
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
}

// local-daemon-thin-client: the daemon's Bound queue is the client's
// displayed Local queue in every state. The runtime attach paths must adopt
// it into the unified tab (as the App-construction path does) instead of
// parking it in `remote_player_tab`, where the unified view never reads it.
#[test]
fn local_daemon_attach_adopts_the_owner_queue_as_the_unified_local_view() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let daemon_items = make_items(1);
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(daemon_items.clone(), 0);

    app.switch_to_direct_remote(
        &make_session("local-daemon", "mbv"),
        remote,
        remote_rx,
        &DaemonEndpoint::Local,
    );

    assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.player_tab.emby_items()[0].id, daemon_items[0].id);
}

#[test]
fn local_daemon_route_attach_adopts_the_owner_queue_as_the_unified_local_view() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let daemon_items = make_items(1);
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(daemon_items.clone(), 0);

    app.switch_to_library_route("music", remote, remote_rx, &DaemonEndpoint::Local);

    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.player_tab.emby_items()[0].id, daemon_items[0].id);
}

#[test]
fn local_route_while_home_link_is_suspended_reinstates_it() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (route, route_rx) = mbv_remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", route, route_rx, &stub_endpoint());
    let (unused_local, unused_local_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);

    app.switch_to_library_route(
        "local",
        unused_local.clone(),
        unused_local_rx,
        &DaemonEndpoint::Local,
    );

    assert!(app.suspended_local.is_none());
    assert!(app.is_local_daemon());
    assert!(!app.player.is_remote_disconnected());
}

#[test]
fn switching_away_with_stay_alive_disabled_sends_stop_to_home_link() {
    let (mut app, home_commands) = make_local_daemon_app_stub_with_cmd_rx(make_items(1));
    app.config.lock().unwrap().stay_alive = false;
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());

    assert!(app.suspended_local.is_some());
    assert!(home_commands.try_iter().any(|command| matches!(
        command,
        mbv_ctrl::CtrlCmd::PlaybackIntent(intent)
            if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
    )));
}
