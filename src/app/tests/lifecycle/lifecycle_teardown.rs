use super::*;

#[test]
fn teardown_flushes_pending_settings_save() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().consume_audio = true;
    app.settings_save_at = Some(Instant::now());

    app.teardown(
        Duration::from_secs(1),
        crate::app::dispatch::run_loop::teardown::test_quit_exit_kind(),
    );

    assert!(crate::config::load_config().unwrap().consume_audio);
}

#[test]
fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.active_route = Some("music".to_string());

    app.teardown(
        Duration::from_secs(1),
        crate::app::dispatch::run_loop::teardown::test_quit_exit_kind(),
    );

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
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

    app.teardown(
        Duration::from_secs(1),
        crate::app::dispatch::run_loop::teardown::test_quit_exit_kind(),
    );

    assert!(!calls.lock().unwrap().contains(&"stop".to_string()));
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

    app.teardown(
        Duration::from_secs(1),
        crate::app::dispatch::run_loop::teardown::test_quit_exit_kind(),
    );

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
    app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Tcp(
        "127.0.0.1:0".parse().unwrap(),
    ));
    app.active_route = None;

    app.teardown(
        Duration::from_secs(1),
        crate::app::dispatch::run_loop::teardown::test_quit_exit_kind(),
    );

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

// Owns daemon-lifecycle "Swapped-out TUI with Stay Alive off" (tray-pin-swap
// 4.4, shell layer per docs/architecture/tui-frontend.md): a `SwapQuit` marks
// the shell exit swapped-out, and teardown then sends no coordinated shutdown
// request and saves no launch snapshot, so the daemon and playback keep
// running with the replacement Client. The Quit path under the same setup is
// owned by `quitting_with_stay_alive_off_stops_this_machines_local_daemon`
// (dispatch/run_loop/teardown.rs), which observes the attempted request.
#[test]
fn swapped_out_teardown_sends_no_shutdown_request() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = make_app_stub();
    app.home_is_local_daemon = true;
    app.config.lock().unwrap().stay_alive = false;

    let (home_remote, _home_rx, home_peer) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    app.suspended_local = Some(crate::app::SuspendedLocalSession {
        player: mbv_player::PlayerProxy::from_remote(home_remote, false),
        player_rx: event_rx,
    });
    let mut model = crate::app::Model::new(app);

    // The run loop's home-link drain delivers the Owner's `SwapQuit` to the
    // shell handler; drive that same entry point.
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::SwapQuit)
        .unwrap();
    model.drain_suspended_home_events();
    assert!(model.swapped_out_exit);
    assert!(crate::app::QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));

    model.teardown(Duration::ZERO);

    assert!(model.app.pending_exit_message.is_none());
    assert_eq!(mbv_config::load_tui_launch_state(), None);

    model
        .app
        .suspended_local
        .as_ref()
        .unwrap()
        .player
        .remote()
        .disconnect();
    home_peer.join().unwrap();
}
