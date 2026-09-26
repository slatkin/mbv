use super::*;
use mbv_core::remote_player::DaemonEndpoint;
pub(super) fn stub_endpoint() -> DaemonEndpoint {
    DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap())
}

#[test]
fn remote_slot_state_is_off_for_local_only_app() {
    let app = make_app_stub();
    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
    assert!(!app.can_disconnect_remote());
}

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

#[test]
fn direct_remote_connect_keeps_local_scope_when_remote_queue_is_empty() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert!(app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .emby_items()
        .is_empty());
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

#[test]
fn direct_remote_connect_switches_to_remote_scope_when_remote_queue_has_items() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items.clone(), 0);
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
fn switch_to_direct_remote_rebinds_mpris_to_the_new_remote_status() {
    // #175: before `switch_to_direct_remote` called `mpris::rebind`,
    // MPRIS stayed wired to whatever `PlayerStatus` was live when the
    // D-Bus service was first registered (almost always the initial
    // local `Player`'s), so local desktop MPRIS never picked up a
    // remote daemon's playback after a mid-session "Direct Remote"
    // takeover -- exactly the bug this issue reports. This drives the
    // real `App` method (not just `mpris::rebind` in isolation) to
    // prove the wiring at the call site is actually in place.
    let mut app = make_app_stub();
    let local_status = Arc::clone(&app.player.status);
    app.mpris = Some(crate::mpris::test_handle(
        Arc::clone(&local_status),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = Arc::clone(&remote.status);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());

    let handle = app.mpris.as_ref().expect("mpris handle still present");
    let bound_status = crate::mpris::test_status(handle);
    assert!(
        Arc::ptr_eq(&bound_status, &remote_status),
        "switch_to_direct_remote must rebind MPRIS to the new remote's status"
    );
    assert!(!Arc::ptr_eq(&bound_status, &local_status));
}

#[test]
fn switch_to_library_route_sets_active_route_and_suspends_local() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    assert!(app.connected_session_id.is_none());
    assert!(app.direct_remote_label.is_none());
}

#[test]
fn route_owned_transport_is_not_sessions_panel_disconnectable() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());
    app.status.clear();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());

    app.disconnect_remote();

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    assert_eq!(app.status, "No session selected");
}

#[test]
fn switch_to_library_route_sets_remote_queue_scope_when_daemon_has_items() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(2), 0);

    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());

    assert!(app.has_direct_remote_queue());
}

#[test]
fn switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap() {
    // #233: a second Direct Remote upgrade must disconnect the old remote.
    // The claim is shell-owned bookkeeping; the peer's socket reaching EOF is
    // the in-memory oracle (no real listener).
    let mut app = make_app_stub();
    let (remote_a, rx_a, daemon_a) = mbv_core::remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_direct_remote(
        &make_session("daemon-a", "mbv"),
        remote_a,
        rx_a,
        &stub_endpoint(),
    );

    let (remote_b, rx_b, _daemon_b) = mbv_core::remote_player::connect_stub_daemon_pair().unwrap();
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
    let (remote_a, rx_a, daemon_a) = mbv_core::remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("music", remote_a, rx_a, &stub_endpoint());
    assert!(!app.player.is_remote_disconnected());

    let (remote_b, rx_b, _daemon_b) = mbv_core::remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("movies", remote_b, rx_b, &stub_endpoint());

    daemon_a
        .join()
        .expect("old library route's client socket must be shut down after the swap");
}

#[test]
fn restore_local_mode_disconnects_the_remote_before_restoring_local() {
    // #233: restore_local_mode must disconnect the old remote before restoring local.
    let mut app = make_app_stub();
    let (remote, rx, daemon) = mbv_core::remote_player::connect_stub_daemon_pair().unwrap();
    app.switch_to_library_route("music", remote, rx, &stub_endpoint());
    assert!(!app.player.is_remote_disconnected());

    app.restore_local_mode("test: ending library route session");

    daemon
        .join()
        .expect("old remote's client socket must be shut down after restore_local_mode");
}

#[test]
fn restore_local_mode_rebinds_mpris_back_to_the_suspended_local_status() {
    // #175 follow-through: after a Direct Remote takeover ends (however
    // it ends -- disconnect, user action, etc.), MPRIS must follow
    // playback back to the restored local `Player`, not stay wired to
    // the now-defunct remote session.
    let mut app = make_app_stub();
    let local_status = Arc::clone(&app.player.status);
    app.mpris = Some(crate::mpris::test_handle(
        Arc::clone(&local_status),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = Arc::clone(&remote.status);
    let sess = make_session("remote-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());

    app.restore_local_mode("test: ending direct remote session");

    let handle = app.mpris.as_ref().expect("mpris handle still present");
    let bound_status = crate::mpris::test_status(handle);
    assert!(
        Arc::ptr_eq(&bound_status, &local_status),
        "restore_local_mode must rebind MPRIS back to the restored local status"
    );
    assert!(!Arc::ptr_eq(&bound_status, &remote_status));
}

#[test]
fn restore_local_mode_clears_active_route() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx, &stub_endpoint());
    assert_eq!(app.active_route.as_deref(), Some("music"));

    app.restore_local_mode("Local playback restored");

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}
