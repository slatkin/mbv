use super::*;
use crate::app::tests::*;

// #202: the in-app quit-key path used to join the player thread
// unboundedly, so a hanging shutdown-time network call could hold the
// single-instance flock indefinitely. `teardown` is the extracted,
// testable sequence both the normal quit-key path and the
// SIGHUP/SIGTERM watchdog path now share; this proves it actually
// returns within its bounded window against a player thread that
// hangs well past the configured `quit_timeout`, without needing a
// real tty (`run()` itself stays untested end-to-end, unchanged status
// quo).
#[test]
fn teardown_bounds_previously_unbounded_normal_quit_join() {
    use mbv_core::player::PlayerProxy;

    let mut app = make_app_stub();
    let status = app.player.status.clone();
    // Thread hangs for 10s; quit_timeout below is 200ms, so a correctly
    // bounded teardown (outer_bound = quit_timeout + quit_timeout/2 +
    // 3s cushion ~= 3.3s here) must return well short of the full hang.
    app.player = PlayerProxy::stub_with_hung_thread(status, Duration::from_secs(10));

    let started = std::time::Instant::now();
    app.teardown(Duration::from_millis(200));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "teardown should return within its bounded window (outer_bound \
             ~= 3.3s here), took {elapsed:?} — previously unbounded on the \
             normal in-app quit path, which is the #202 bug"
    );
}

#[test]
fn teardown_fast_when_player_thread_is_not_hung() {
    let mut app = make_app_stub();

    let started = std::time::Instant::now();
    app.teardown(Duration::from_secs(5));
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

    app.teardown(Duration::from_secs(1));

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

    app.teardown(Duration::from_secs(1));

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
    app.teardown(Duration::from_secs(1));

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
    let (job_tx, calls) =
        super::types_cast::spawn_fake_cast_worker(super::types_cast::FakeCastTransport::default());
    app.set_cast_client("device-1", job_tx);

    app.teardown(Duration::from_secs(1));

    assert!(!calls.lock().unwrap().contains(&"stop".to_string()));
}

#[test]
fn teardown_persists_attached_cast_receiver_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = true;
    app.attach_cast("device-1".to_string());

    app.teardown(Duration::from_secs(1));

    assert_eq!(
        mbv_core::config::load_last_cast_receiver().unwrap(),
        Some("device-1".to_string())
    );
}

#[test]
fn teardown_never_touches_persisted_cast_receiver_when_auto_reconnect_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = mbv_core::config::save_last_cast_receiver(Some("device-old"));
    let mut app = make_app_stub();
    assert!(!app.config.lock().unwrap().auto_reconnect);
    app.attach_cast("device-1".to_string());

    app.teardown(Duration::from_secs(1));

    // Feature is off: the previously saved record is left exactly as it
    // was, not cleared or overwritten just because a cast target happens
    // to be attached right now.
    assert_eq!(
        mbv_core::config::load_last_cast_receiver().unwrap(),
        Some("device-old".to_string())
    );
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

    app.teardown(Duration::from_secs(1));

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

    app.teardown(Duration::from_secs(1));

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

    app.teardown(Duration::from_secs(1));

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

    app.teardown(Duration::from_secs(1));

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_retires_remote_tracking_owned_state() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.connected_session_id = Some("session".into());
    app.remote_tracker = Some(
        mbv_core::remote_reconciliation::ReconciliationTracker::new(
            "session",
            vec![
                mbv_core::remote_reconciliation::SubmittedOccurrence::new(1, "a"),
                mbv_core::remote_reconciliation::SubmittedOccurrence::new(2, "b"),
            ],
            0,
            0,
        )
        .unwrap(),
    );
    app.pending_overlay = Some(super::types_overlay::OverlayRequest::RemoteReanchor(
        super::types_playback::RemoteReanchorPopup {
            targets: vec![(0, "a".into())],
            cursor: 0,
        },
    ));

    app.teardown(Duration::from_secs(1));

    assert!(app.remote_tracker.is_none());
    assert!(app.remote_queue_projection.is_none());
    assert!(matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::DismissRemoteReanchor)
    ));
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
    app.teardown(Duration::from_secs(1));

    let state = crate::config::load_queue_state().expect("existing daemon snapshot");
    assert_eq!(
        state.items.iter().map(|item| item.id()).collect::<Vec<_>>(),
        old_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn wants_terminal_render_true_when_due() {
    let app = make_app_stub();
    let stale = Instant::now() - Duration::from_secs(10);
    assert!(app.wants_terminal_render(false, stale, Duration::from_secs(1)));
}

// A compact-banner poster fetch (or any list-image prefetch) can easily
// outlast the idle render cadence: with nothing playing and no remote
// session, the run loop only repaints once a second unless something
// sets `had_events` (a key/mouse event, or the fetch itself completing).
// That meant a loading placeholder was computed correctly by
// `compact_banner_layout` but never actually painted -- the only two
// frames drawn were "just navigated, fetch not even started yet" and
// "fetch just completed", with nothing in between showing the reserved
// placeholder box. Treating an in-flight image fetch the same as active
// playback (fast 150ms cadence instead of the 1s idle one) gives the
// loop a reason to repaint while the placeholder should be visible.
#[test]
fn render_interval_is_fast_while_a_card_image_fetch_is_in_flight() {
    let mut app = make_app_stub();
    app.card_image_loading.insert("movie-1:cmp_primary".into());
    assert_eq!(app.render_interval(), Duration::from_millis(150));
}

#[test]
fn render_interval_is_slow_when_idle_with_no_fetches_in_flight() {
    let mut app = make_app_stub();
    assert_eq!(app.render_interval(), Duration::from_secs(1));
}



#[test]
fn auto_reconnect_settings_row_displays_and_toggles_current_session() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = false;

    let cfg = app.config.lock().unwrap().clone();
    assert_eq!(
        settings::setting_label(SettingKey::AutoReconnect),
        "Auto reconnect"
    );
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "off"
    );

    app.handle_settings_activate(SettingKey::AutoReconnect);
    let cfg = app.config.lock().unwrap().clone();
    assert!(cfg.auto_reconnect);
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "on"
    );
    assert!(
        app.settings_save_at.is_some(),
        "settings toggle must use the delayed save path"
    );

    app.handle_settings_activate(SettingKey::AutoReconnect);
    assert!(!app.config.lock().unwrap().auto_reconnect);
}

#[test]
fn enabling_auto_reconnect_persists_the_active_remote_target() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().auto_reconnect = false;
    app.active_route = Some("music".to_string());

    app.handle_settings_activate(SettingKey::AutoReconnect);

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

// ── transport_prev_next_available (issue #112) ─────────────────────────
// Drives whether playback transport is currently available at the queue
// boundaries. The header uses the `next` half directly, while the `P`/`N`
// keys still reuse both halves.

#[test]
fn transport_prev_next_unavailable_when_player_inactive() {
    let app = make_app_stub();
    assert!(!app.player.status.lock().unwrap().active);
    assert_eq!(app.transport_prev_next_available(), (false, false));
}

#[test]
fn transport_prev_next_both_available_mid_queue() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 1;
    }
    assert_eq!(app.transport_prev_next_available(), (true, true));
}

#[test]
fn transport_prev_unavailable_on_first_item() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 0;
    }
    assert_eq!(app.transport_prev_next_available(), (false, true));
}

#[test]
fn transport_next_unavailable_on_last_item() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 2;
    }
    assert_eq!(app.transport_prev_next_available(), (true, false));
}

#[test]
fn transport_prev_next_both_available_for_connected_remote_session_regardless_of_local_status() {
    // SessionInfo (see mbv_core::api::SessionInfo) exposes no
    // queue-position/length fields, so there's no boundary to check for a
    // connected remote session. Local status here is deliberately set to
    // "last item" to prove it's ignored while a session is connected.
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 2;
    }
    assert_eq!(app.transport_prev_next_available(), (true, true));
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
