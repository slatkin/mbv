use super::*;
use crate::app::tests::*;

#[test]
fn local_daemon_bootstrap_adopts_saved_local_queue_and_source() {
    let items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("pl1".into()),
                name: "Saved".into(),
            },
            items,
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.queue_cursor, 1);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Saved"
    ));
    assert!(matches!(
        bootstrap.adopt_queue,
        Some((_, 1, crate::config::QueueSource::Playlist { ref name, .. })) if name == "Saved"
    ));
}

#[test]
fn failed_local_daemon_adoption_routes_through_remote_disconnected() {
    // #119 task 5: a swallowed `adopt_queue()` send-failure must not
    // leave the app silently sitting on optimistic queue state the
    // daemon never received — it routes through the same handling a
    // live `PlayerEvent::RemoteDisconnected` uses.
    let mut app = make_local_daemon_app_stub(Vec::new());
    assert_eq!(app.queue_scope, QueueScope::Local);

    app.handle_failed_local_daemon_adoption();

    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert!(app.status.contains("daemon connection lost"));
}

#[test]
fn remote_app_starts_on_local_queue_when_remote_queue_is_empty() {
    let app = make_remote_app_stub(make_items(2), Vec::new());

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
    let app = make_remote_app_stub(make_items(2), make_items(1));

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
}

#[test]
fn local_daemon_bootstrap_carries_saved_positions_for_enrichment() {
    let items = make_items(2);
    let mut positions = std::collections::HashMap::new();
    positions.insert(items[0].id.clone(), 999);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items,
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: positions.clone(),
        }),
    );

    assert_eq!(bootstrap.positions, positions);
}

#[test]
fn local_daemon_bootstrap_has_no_positions_without_saved_state() {
    let bootstrap =
        bootstrap_local_daemon_queue(Vec::new(), 0, crate::config::QueueSource::Unknown, None);

    assert!(bootstrap.positions.is_empty());
}

#[test]
fn local_daemon_bootstrap_uses_restore_cursor_and_carries_last_played_state() {
    let items = make_items(3);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items.clone(),
            cursor: 0,
            last_played_item_id: Some(items[1].id.clone()),
            last_played_completed: true,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.queue_cursor, 2);
    assert_eq!(
        bootstrap.last_played_item_id.as_deref(),
        Some(items[1].id.as_str())
    );
    assert!(bootstrap.last_played_completed);
}

#[test]
fn local_daemon_bootstrap_prefers_existing_daemon_queue_state() {
    let remote_items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        remote_items.clone(),
        0,
        crate::config::QueueSource::Playlist {
            id: Some("daemon".into()),
            name: "Daemon Queue".into(),
        },
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("local".into()),
                name: "Local Saved".into(),
            },
            items: make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.items[0].id, remote_items[0].id);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Daemon Queue"
    ));
    assert!(bootstrap.adopt_queue.is_none());
}

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
        String,
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
        String,
    > {
        Err("incompatible daemon protocol version: peer=1 local=3".to_string())
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
        String,
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
    app.switch_to_library_route("music", remote, remote_rx);
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
        String,
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

#[test]
fn try_daemon_route_connect_returns_remote_player_on_successful_connect() {
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
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(result.is_ok());
}

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

#[test]
fn try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure() {
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

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    // `RemotePlayer` derives only `Clone` (no `PartialEq`/`Debug` --
    // confirmed against `crates/mbv-core/src/remote_player.rs`), so the
    // whole `Result` can't go through `assert_eq!` directly; match out
    // the `Err` payload instead.
    match result {
        Ok(_) => panic!("expected a connect failure to return Err, got Ok"),
        Err(message) => {
            assert_eq!(
                message,
                "\u{26a0} Music route unreachable, using local playback (mbv.log)"
            );
        }
    }
    // The primitive itself must never flash -- that is the caller's
    // job (see Architecture). `make_app_stub()` starts with an empty
    // status, so this pins down that `try_daemon_route_connect` left
    // it untouched.
    assert!(app.status.is_empty());
}

#[test]
fn app_construction_never_attempts_a_daemon_route_connect() {
    // #222 acceptance criterion: "No connection attempt happens before
    // the first play/enqueue action that needs one." There is no
    // production call site wiring `try_daemon_route_connect` into
    // startup yet (that wiring is #223's job) -- this test pins the
    // invariant down as a regression guard so a future startup-time
    // call is caught immediately instead of silently reintroducing the
    // eager-connect behavior #222 replaces.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    fn counting_connect(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(counting_connect);
    let _app = make_app_stub();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn apply_route_for_playback_swaps_to_routed_daemon_on_success() {
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

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

#[test]
fn apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
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
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}

#[test]
fn apply_route_for_playback_is_noop_when_item_already_matches_active_route() {
    // #256: resolution is now a pure config read -- no live session
    // lookup, no SESSIONS_LOAD_OVERRIDE seam needed to reach the no-op
    // branch (`name == current`), even though this test's whole point
    // is that no *connect* attempt happens.
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.active_route = Some("music".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    // No connect attempt was needed (no DAEMON_ROUTE_CONNECT_OVERRIDE
    // set, so a real connect attempt would panic/hang if this weren't
    // a no-op) -- active_route and local-ness are unchanged.
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_when_item_has_no_route() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: movies_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();

    app.apply_route_for_playback(&item);

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_via_restore_local_mode_when_swap_to_a_different_route_fails(
) {
    // Regression guard for the `Err` branch's `was_routed.is_some()`
    // arm: already on a different route ("music"), an item resolving
    // to a *new* route ("movies") whose connect attempt fails must be
    // torn down through `restore_local_mode` -- not just flashed a
    // warning while silently staying attached to the stale "music"
    // remote player.
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
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

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.library_routes
        .insert("movies".to_string(), "tcp://127.0.0.1:9001".to_string());
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());

    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();
    app.library_tab = 1;

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    app.apply_route_for_playback(&item);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}

#[test]
fn remote_position_extrapolation_does_not_round_up_partial_seconds() {
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_millis(1600)),
        11
    );
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_secs(2)),
        12
    );
}
