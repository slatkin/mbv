use super::*;

#[test]
fn teardown_flushes_pending_settings_save() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config_for_test(|config| config.consume_audio = true);
    app.seed_deferred_settings_save_for_test();

    app.teardown(Duration::from_secs(1), false);

    assert!(crate::app::config::load_config().unwrap().consume_audio);
}

#[test]
fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config_for_test(|config| config.auto_reconnect = true);
    app.seed_active_route_for_test(Some("music".to_string()));

    app.teardown(Duration::from_secs(1), false);

    assert_eq!(
        crate::app::config::load_last_remote_connection().unwrap(),
        Some(crate::app::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_never_touches_persisted_state_when_auto_reconnect_disabled() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let _ = crate::app::config::save_last_remote_connection(Some(
        &crate::app::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.config_for_test(|config| config.auto_reconnect));
    app.seed_active_route_for_test(None);

    app.teardown(Duration::from_secs(1), false);

    // Feature is off: the file from before this test's own `app` even
    // existed must be left exactly as it was, not cleared just because
    // `active_route` is currently `None`.
    assert_eq!(
        crate::app::config::load_last_remote_connection().unwrap(),
        Some(crate::app::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

// Owns the auto-reconnect teardown rule (the local-Client exit must not
// erase the saved target): an exit that merely had no remote at the moment --
// the App never attached a remote this run and never explicitly left one --
// keeps the record a previous run saved, exactly as a Client that never
// changed the connection this run must not clear it.
#[test]
fn teardown_without_a_remote_keeps_the_saved_target() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let _ = crate::app::config::save_last_remote_connection(Some(
        &crate::app::config::LastRemoteConnection::DirectSession {
            device_name: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config_for_test(|config| config.auto_reconnect = true);

    app.teardown(Duration::from_secs(1), false);

    assert_eq!(
        crate::app::config::load_last_remote_connection().unwrap(),
        Some(crate::app::config::LastRemoteConnection::DirectSession {
            device_name: "music".to_string(),
        })
    );
}

// The other side of the same rule: the user explicitly left the remote this
// run (`disconnect_remote` under Direct remote control), so teardown clears
// the record the previous run saved and the next launch stays local.
#[test]
fn teardown_after_an_explicit_disconnect_clears_the_saved_target() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let _ = crate::app::config::save_last_remote_connection(Some(
        &crate::app::config::LastRemoteConnection::DirectSession {
            device_name: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.config_for_test(|config| config.auto_reconnect = true);
    app.seed_direct_remote_connected_for_test();
    // The Local endpoint puts `restore_local_mode` on its own local-baseline
    // early return, so the detach needs no owner connection.
    app.set_player_endpoint_for_test(mbv_remote_player::DaemonEndpoint::Local);

    app.disconnect_remote_for_test();

    app.teardown(Duration::from_secs(1), false);

    assert_eq!(
        crate::app::config::load_last_remote_connection().unwrap(),
        None
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
    let _guard = crate::app::config::TestStateDirGuard::new();
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = make_app_stub();
    app.config_for_test(|config| config.stay_alive = false);

    let (home_remote, _home_rx, home_peer) = mbv_remote_player::connect_stub_daemon_pair().unwrap();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    app.suspend_home_link_for_test(
        mbv_player::PlayerProxy::from_remote(home_remote, false),
        event_rx,
    );
    let mut model = crate::tui::Model::new(app);

    // The run loop's home-link drain delivers the Owner's `SwapQuit` to the
    // shell handler; drive that same entry point.
    event_tx
        .send(mbv_ctrl::player::PlayerEvent::SwapQuit)
        .unwrap();
    model.drain_suspended_home_events();
    assert!(model.swapped_out_exit);
    assert!(crate::app::QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed));

    model.teardown(Duration::ZERO);

    assert!(model.app.pending_exit_message_for_test().is_none());
    assert_eq!(mbv_config::load_tui_launch_state(), None);

    model.app.suspended_home_remote_disconnect_for_test();
    home_peer.join().unwrap();
}
