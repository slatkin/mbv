use super::*;
use crate::app::tests::*;

#[test]
fn remote_slot_state_is_off_for_local_only_app() {
    let app = make_app_stub();
    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
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
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [d]disc [r]refresh [Esc]close"
    );
}

#[test]
fn remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect() {
    let app = make_remote_app_stub(make_items(2), make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}

#[test]
fn direct_remote_connect_keeps_local_scope_when_remote_queue_is_empty() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn direct_remote_connect_switches_to_remote_scope_when_remote_queue_has_items() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items.clone(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().items[0].id,
        remote_items[0].id
    );
    assert_eq!(app.player_tab.items.len(), 2);
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
    let local_status = app.player.status.clone();
    app.mpris = Some(crate::mpris::test_handle(
        local_status.clone(),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = remote.status.clone();
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    let handle = app.mpris.as_ref().expect("mpris handle still present");
    let bound_status = crate::mpris::test_status(handle);
    assert!(
        Arc::ptr_eq(&bound_status, &remote_status),
        "switch_to_direct_remote must rebind MPRIS to the new remote's status"
    );
    assert!(!Arc::ptr_eq(&bound_status, &local_status));
}

#[test]
fn switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap() {
    // Same #233 regression, but for the Sessions-panel direct-remote
    // path's already-remote branch (a second "Direct Remote" upgrade
    // while already on one).
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    let daemon_a = std::thread::spawn(move || {
        let (stream, _) = listener_a.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_b = listener_b.local_addr().unwrap();
    let daemon_b = std::thread::spawn(move || {
        let (stream, _) = listener_b.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let sess_a = make_session("daemon-a", "mbv");
    let (remote_a, remote_a_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
    app.switch_to_direct_remote(
        &sess_a,
        remote_a,
        remote_a_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    let sess_b = make_session("daemon-b", "mbv");
    let (remote_b, remote_b_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
    app.switch_to_direct_remote(
        &sess_b,
        remote_b,
        remote_b_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    let mut daemon_a_stream = daemon_a.join().unwrap();
    daemon_a_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    // Drain any in-flight control bytes before the socket shutdown
    // and require the peer to reach EOF within the timeout.
    let mut pending_control_bytes = Vec::new();
    daemon_a_stream
        .read_to_end(&mut pending_control_bytes)
        .expect("old direct-remote client socket must be shut down after the swap");

    drop(daemon_b);
    let _ = addr_b;
}

#[test]
fn switch_to_library_route_sets_active_route_and_suspends_local() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    // Must stay independent of the Sessions-panel direct-remote fields.
    assert!(app.connected_session_id.is_none());
    assert!(app.direct_remote_label.is_none());
}

#[test]
fn route_owned_transport_is_not_sessions_panel_disconnectable() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.status.clear();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );

    app.disconnect_remote();

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    assert_eq!(app.status, "No session connected");
}

#[test]
fn switch_to_library_route_sets_remote_queue_scope_when_daemon_has_items() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(2), 0);

    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    assert!(app.has_direct_remote_queue());
}

#[test]
fn switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap() {
    // #233 regression guard: swapping from one active library route
    // straight to another (the already-remote branch) must tear down
    // the OLD RemotePlayer's connection before replacing it, not just
    // let it leak via Drop. Uses two real TCP loopback "daemons" (not
    // RemotePlayer::stub, which has no real socket to observe) so the
    // first daemon's accepted connection can observe its client side
    // actually closing.
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    let daemon_a = std::thread::spawn(move || {
        let (stream, _) = listener_a.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_b = listener_b.local_addr().unwrap();
    let daemon_b = std::thread::spawn(move || {
        let (stream, _) = listener_b.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let (remote_a, remote_a_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
    app.switch_to_library_route(
        "music",
        remote_a,
        remote_a_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(!app.player.is_remote_disconnected());

    let (remote_b, remote_b_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
    app.switch_to_library_route(
        "movies",
        remote_b,
        remote_b_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    // The OLD (music) connection's daemon-side accept handle should
    // see its client hang up shortly after the swap -- proof the
    // reader thread actually exited instead of leaking.
    let mut daemon_a_stream = daemon_a.join().unwrap();
    daemon_a_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    // Drain any in-flight control bytes before the socket shutdown
    // and require the peer to reach EOF within the timeout.
    let mut pending_control_bytes = Vec::new();
    daemon_a_stream
        .read_to_end(&mut pending_control_bytes)
        .expect("old library route's client socket must be shut down after the swap");

    drop(daemon_b);
    let _ = addr_b; // silence unused warning if daemon_b's thread hasn't been joined
}

#[test]
fn restore_local_mode_rebinds_mpris_back_to_the_suspended_local_status() {
    // #175 follow-through: after a Direct Remote takeover ends (however
    // it ends -- disconnect, user action, etc.), MPRIS must follow
    // playback back to the restored local `Player`, not stay wired to
    // the now-defunct remote session.
    let mut app = make_app_stub();
    let local_status = app.player.status.clone();
    app.mpris = Some(crate::mpris::test_handle(
        local_status.clone(),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = remote.status.clone();
    let sess = make_session("remote-host", "mbv");
    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

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
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert_eq!(app.active_route.as_deref(), Some("music"));

    app.restore_local_mode("Local playback restored");

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn restore_local_mode_disconnects_the_remote_before_restoring_local() {
    // #233 follow-up regression guard: `restore_local_mode` is the
    // shared "go back to local" tail. `self.player.join()` is a
    // documented no-op for a remote player, so the subsequent
    // `self.player = suspended.player` reassignment used to drop the
    // old RemotePlayer without ever disconnecting it, leaking its
    // reader thread exactly like the two already-fixed remote-to-remote
    // swap branches. Uses a real TCP loopback "daemon" (not
    // RemotePlayer::stub, which has no real socket to observe) so the
    // daemon's accepted connection can observe its client side actually
    // closing.
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let (remote, remote_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(!app.player.is_remote_disconnected());

    app.restore_local_mode("test: ending library route session");

    // The OLD (music) connection's daemon-side accept handle should see
    // its client hang up shortly after `restore_local_mode` runs --
    // proof the reader thread actually exited instead of leaking.
    let mut daemon_stream = daemon.join().unwrap();
    daemon_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buf = [0u8; 8];
    let n = daemon_stream.read(&mut buf).unwrap_or(usize::MAX);
    assert_eq!(
        n, 0,
        "old remote's client socket must be shut down after restore_local_mode"
    );
}

#[test]
fn remote_slot_state_is_local_daemon_for_thin_client_mode() {
    let app = make_local_daemon_app_stub(make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}

#[test]
fn player_owner_is_on_this_machine_for_in_process_local_tcp_and_unix_targets() {
    // Bare-mode in-process player: owner is on this machine.
    let mut app = make_app_stub();
    assert!(app.player_owner_is_on_this_machine());

    // Managed local daemon: owner is on this machine.
    let local_daemon_app = make_local_daemon_app_stub(make_items(3));
    assert!(local_daemon_app.player_owner_is_on_this_machine());

    // TCP endpoint (genuinely remote): owner is elsewhere.
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_direct_remote(
        &make_session("remote-a", "mbv"),
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(!app.player_owner_is_on_this_machine());

    // Unix endpoint: treated as not-this-machine (matching existing
    // `DaemonEndpoint::is_local()` semantics).
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_direct_remote(
        &make_session("remote-b", "mbv"),
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(!app.player_owner_is_on_this_machine());
}

#[test]
fn restoring_a_suspended_in_process_player_clears_locality_and_shows_in_process_ownership() {
    // Regression guard: a bare-mode app that switches to a
    // managed-local-daemon-classified target, then restores its suspended
    // in-process player must not retain local-daemon status — that stale
    // boolean used to survive restoration and enable heartbeat icons and
    // local-daemon queue paths for a now-bare-mode process.
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_direct_remote(
        &make_session("local-daemon", "mbv"),
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Local,
    );
    assert!(app.is_local_daemon());
    assert!(app.player_owner_is_on_this_machine());

    app.restore_local_mode("Disconnected from direct remote session");

    assert!(!app.is_local_daemon());
    assert!(app.player_owner_is_on_this_machine());
    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
}

#[test]
fn announced_shutdown_of_current_remote_target_does_not_quit_local_daemon_home() {
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn reconnect_local(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            std::sync::mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(reconnect_local);
    QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let mut app = App::new_remote(
        mbv_core::api::EmbyClient::new(crate::config::Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );
    // The app was launched against the local daemon, but playback has since
    // moved to a genuinely remote route. Disconnect handling must use this
    // live target flag, not the launch-time home flag.
    app.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));

    app.handle_player_event(PlayerEvent::DaemonShutdownAnnounced);

    assert!(!QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.daemon_lost_modal.is_none());
    assert!(
        app.is_local_daemon(),
        "restore should reconnect the home daemon"
    );
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn resetting_local_daemon_queue_view_drops_stale_remote_queue_and_scope() {
    let mut app = make_local_daemon_app_stub(make_items(1));
    app.remote_player_tab = Some(PlayerTab::new(make_items(2), 1));
    app.queue_scope = QueueScope::Remote;

    app.reset_local_daemon_queue_view();

    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn attached_session_state_wins_over_local_daemon_indicator() {
    let mut app = make_local_daemon_app_stub(make_items(3));
    app.connected_session_id = Some("session-1".into());

    assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
    assert!(app.can_disconnect_remote());
}

#[test]
fn disconnect_remote_does_not_exit_local_daemon_mode() {
    let mut app = make_local_daemon_app_stub(make_items(3));

    app.disconnect_remote();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
    assert!(app.player.is_remote());
    assert!(!app.can_disconnect_remote());
    assert_eq!(app.status, "No session connected");
}

#[test]
fn disconnect_remote_clears_attached_remote_session() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.session_miss_count = 2;
    app.remote_pos_s = 120;

    app.disconnect_remote();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
    assert!(app.connected_session_id.is_none());
    assert!(app.connected_session_state.is_none());
    assert_eq!(app.session_miss_count, 0);
    assert_eq!(app.remote_pos_s, 0);
    assert_eq!(app.status, "Disconnected from remote session");
}

#[test]
fn disconnect_remote_restores_local_for_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let sess = make_session("music", "mbv");

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    assert_eq!(app.direct_remote_label.as_deref(), Some("music"));
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(app.direct_remote_label.is_none());
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert_eq!(app.status, "Disconnected from direct remote session");
}

#[test]
fn disconnecting_attached_session_preserves_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let direct_session = make_session("music", "mbv");
    let attached_session = make_session("living-room", "Emby");

    app.switch_to_direct_remote(
        &direct_session,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.connect_to_session(&attached_session);

    assert!(app.direct_remote_connected);
    assert!(app.connected_session_id.is_some());

    app.disconnect_remote();

    assert!(app.player.is_remote());
    assert!(app.direct_remote_connected);
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(!app.player.is_remote());
    assert!(!app.direct_remote_connected);
}

#[test]
fn displayed_queue_playback_state_stays_active_for_local_daemon_queue() {
    let mut app = make_local_daemon_app_stub(make_items(3));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 2;
        status.position_ticks = 42;
        status.runtime_ticks = 84;
        status.paused = true;
    }

    assert_eq!(
        app.displayed_queue_playback_state(),
        PlaybackState {
            active: true,
            active_idx: 2,
            position_ticks: 42,
            runtime_ticks: 84,
            paused: true,
        }
    );
}

#[test]
fn local_daemon_consume_adjusts_active_idx_after_removal_shift() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_local_daemon_app_stub(make_items(4));
    app.client.lock().unwrap().config.consume_videos = true;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 1,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    {
        let mut status = app.player.status.lock().unwrap();
        // Thin-client path: the remote player updates status.current_idx
        // from the daemon's TrackChanged event before App handles the
        // pending consume removal, so App must correct the shifted index.
        status.current_idx = 2;
    }
    app.handle_player_event(PlayerEvent::TrackChanged(2));

    assert_eq!(app.player_tab.queue_cursor, 1);
    assert_eq!(
        app.displayed_queue_playback_state().active_idx,
        1,
        "after removing the completed item, the active index must shift to \
             the now-playing item's new slot instead of following the stale \
             pre-removal numeric index"
    );
}

#[test]
fn direct_remote_consume_adjusts_active_idx_after_removal_shift() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(4);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.client.lock().unwrap().config.consume_videos = true;
    app.set_queue_scope(QueueScope::Remote);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 1,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    {
        let mut status = app.player.status.lock().unwrap();
        // Network direct-remote path receives the same raw pre-removal
        // TrackChanged index from the daemon as the same thin-client
        // control path covered above.
        status.current_idx = 2;
    }
    app.handle_player_event(PlayerEvent::TrackChanged(2));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(&app.player_tab.items).unwrap(),
        serde_json::to_value(&local_items).unwrap()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
        vec![
            remote_items[0].id.clone(),
            remote_items[2].id.clone(),
            remote_items[3].id.clone(),
        ]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert_eq!(
        app.displayed_queue_playback_state().active_idx,
        1,
        "after removing the completed remote item, the active index must \
             shift to the now-playing item's new remote-queue slot"
    );
}

#[test]
fn restore_local_mode_reconnects_local_daemon_when_no_suspended_local_player_exists() {
    // Regression guard: an `App::new_remote(..., is_local_daemon = true)`
    // instance (local-daemon home, e.g. `mbv -d` auto-detected at startup)
    // has no genuinely local in-process `Player` to suspend when it routes
    // away via `switch_to_library_route`'s already-remote branch --
    // `suspended_local` stays `None` for its whole life. Before this fix,
    // `restore_local_mode` did nothing in that case, leaving the player
    // disconnected instead of reconnected to the local daemon.
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

    let mut app = make_local_daemon_app_stub(make_items(2));
    assert!(app.home_is_local_daemon);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(app.suspended_local.is_none());

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);
    app.restore_local_mode("test: route no longer resolves");
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(app.player.is_remote());
    assert!(app.is_local_daemon());
    assert!(app.active_route.is_none());
}

#[test]
fn restore_local_mode_flashes_combined_status_when_local_daemon_reconnect_fails() {
    // Same starting scenario as above, but the local-daemon reconnect
    // attempt itself fails -- confirms `restore_local_mode` folds that
    // failure into the flashed status instead of panicking or silently
    // dropping it.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    let mut app = make_local_daemon_app_stub(make_items(2));
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route(
        "music",
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    app.restore_local_mode("test: route no longer resolves");
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    // `disconnect_remote` tears down the old remote's socket but doesn't
    // change `PlayerProxy`'s inner variant, so the stale remote player is
    // still what `is_remote()` reports here -- there was nothing to swap it
    // for since the reconnect failed. This mirrors `restore_local_mode`'s
    // pre-existing behavior for any other failure path: no invented
    // recovery UX, just the same left-disconnected player plus a status
    // flash.
    assert!(app.player.is_remote());
    assert!(app.status.contains("test: route no longer resolves"));
    assert!(app.status.contains("unreachable"));
}

#[test]
fn displayed_queue_playback_state_is_inactive_for_non_playback_scope() {
    let mut app = make_remote_app_stub(make_items(2), make_items(3));
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.connected_session_state
        .as_mut()
        .unwrap()
        .now_playing_item_id = Some("id1".into());
    app.set_queue_scope(QueueScope::Local);

    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert_eq!(
        app.displayed_queue_playback_state(),
        PlaybackState::default()
    );
}
