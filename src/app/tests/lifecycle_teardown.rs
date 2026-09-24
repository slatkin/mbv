use super::super::*;
use super::*;

#[test]
fn teardown_fast_when_player_thread_is_not_hung() {
    let mut app = make_app_stub();

    let started = std::time::Instant::now();
    app.teardown(Duration::from_secs(5), None);
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "teardown against a player with no thread to join should return \
             promptly, not wait anywhere near the quit_timeout budget, took {elapsed:?}"
    );
}

#[test]
fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.active_route = Some("music".to_string());

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_persists_connected_session_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    app.connected_session_id = Some(sess.id.clone());
    app.connected_session_state = Some(sess);

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_persists_direct_remote_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_issues_no_stop_for_an_attached_cast_target() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.attach_cast("device-1".to_string());
    let (job_tx, calls) = crate::app::state::types::cast::spawn_fake_cast_worker(
        crate::app::state::types::cast::FakeCastTransport::default(),
    );
    app.set_cast_client("device-1", job_tx);

    app.teardown(Duration::from_secs(1), None);

    assert!(!calls.lock().unwrap().contains(&"stop".to_string()));
}

#[test]
fn teardown_clears_persisted_connection_when_exiting_local() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(crate::config::load_last_remote_connection().unwrap(), None);
}

#[test]
fn teardown_never_touches_persisted_state_when_auto_reconnect_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.config.lock().unwrap().auto_reconnect);
    app.active_route = None;

    app.teardown(Duration::from_secs(1), None);

    // Feature is off: the file from before this test's own `app` even
    // existed must be left exactly as it was, not cleared just because
    // `active_route` is currently `None`.
    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_persists_a_reconnected_target_for_a_local_daemon_launch_that_moved_remote() {
    // Regression guard for design.md's Decision 2 walk-through: a client
    // launched attached to the local daemon (`home_is_local_daemon = true`)
    // that has since reconnected to a genuinely remote target
    // (`player_endpoint` set to a non-local endpoint) must still have
    // that connection persisted at teardown -- the old gate, keyed on the
    // mutable `is_local_daemon`, would have wrongly skipped this.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.launched_as_remote = true;
    app.home_is_local_daemon = true;
    app.config.lock().unwrap().stay_alive = true;
    app.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));
    app.active_route = Some("music".to_string());

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_skips_persistence_for_an_explicit_remote_daemon_launch() {
    // The existing skip case, unaffected by rebasing the gate onto
    // `home_is_local_daemon`: an explicit `--connect-daemon` launch has
    // `home_is_local_daemon = false` for its entire lifetime, so it must
    // still be skipped and must not overwrite an existing saved record.
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.launched_as_remote = true;
    app.home_is_local_daemon = false;
    app.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));
    app.active_route = None;

    app.teardown(Duration::from_secs(1), None);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn local_daemon_client_does_not_overwrite_authoritative_queue_on_teardown() {
    let _guard = crate::config::TestStateDirGuard::new();
    let old_items = make_items(1);
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Album,
        items: old_items
            .iter()
            .cloned()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    })
    .expect("save queue state");

    let mut app = make_local_daemon_app_stub(make_items(2));
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.teardown(Duration::from_secs(1), None);

    let state = crate::config::load_queue_state().expect("existing daemon snapshot");
    assert_eq!(
        state.items.iter().map(|item| item.id()).collect::<Vec<_>>(),
        old_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}
