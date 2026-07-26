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
    app.client.lock().unwrap().config.auto_reconnect = true;
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
    app.client.lock().unwrap().config.auto_reconnect = true;
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
    app.client.lock().unwrap().config.auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_direct_remote(&sess, remote, remote_rx);
    app.teardown(Duration::from_secs(1));

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
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
    app.client.lock().unwrap().config.auto_reconnect = true;

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
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
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

// ── wants_terminal_render (#156: detached stay-alive must not touch
// the terminal — Terminal::clear() blocks on a cursor-position DSR
// query nobody answers once the terminal-client has detached) ────────

#[test]
fn wants_terminal_render_true_when_attached_and_due() {
    let mut app = make_app_stub();
    app.attached = true;
    let stale = Instant::now() - Duration::from_secs(10);
    assert!(app.wants_terminal_render(false, stale, Duration::from_secs(1)));
}

#[test]
fn wants_terminal_render_false_when_detached_even_with_events_and_force_clear() {
    let mut app = make_app_stub();
    app.attached = false;
    app.force_clear = true;
    let stale = Instant::now() - Duration::from_secs(10);
    // had_events, force_clear, and an elapsed render_interval would all
    // independently demand a render while attached -- none of them may
    // override `attached == false`, or the run loop calls
    // Terminal::clear()/draw() with nobody left to answer the pty.
    assert!(!app.wants_terminal_render(true, stale, Duration::from_secs(1)));
}

#[test]
fn wants_terminal_render_false_when_detached_and_idle() {
    let app = make_app_stub();
    let mut app = app;
    app.attached = false;
    let recent = Instant::now();
    assert!(!app.wants_terminal_render(false, recent, Duration::from_secs(1)));
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
    let app = make_app_stub();
    assert_eq!(app.render_interval(), Duration::from_secs(1));
}

#[test]
fn try_quit_bare_mode_does_not_touch_attached() {
    let mut app = make_app_stub();
    app.attached = true;
    // No `stay_alive_ctrl` -> bare mode -> `attached` is irrelevant and
    // must stay untouched (it's never consulted outside stay-alive).
    let _ = app.try_quit();
    assert!(app.attached);
}

#[test]
fn try_quit_stay_alive_detach_clears_attached_and_notifies_relay() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.attached = true;
    app.client.lock().unwrap().config.stay_alive = true;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));

    let quit_loop_should_exit = app.try_quit();

    assert!(
        !quit_loop_should_exit,
        "stay-alive `q` must detach, never quit the run loop"
    );
    assert!(
        !app.attached,
        "detach must clear `attached` so the run loop skips terminal I/O \
             until the next reattach (#156)"
    );

    // And it must have actually told the relay to detach, not just
    // flipped local state.
    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(&buf[..n], b"DETACH\n");
}

#[test]
fn try_quit_stay_alive_session_exits_when_setting_disabled() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.attached = true;
    app.client.lock().unwrap().config.stay_alive = false;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));

    let quit_loop_should_exit = app.try_quit();

    assert!(
        quit_loop_should_exit,
        "disabling Stay alive on exit must make the next `q` quit this attached session"
    );
    assert!(
        app.attached,
        "real quit should not flip the detached-session guard"
    );

    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(
        n, 0,
        "runtime quit path must not tell the relay to detach once Stay alive on exit is disabled"
    );
}

#[test]
fn stay_alive_settings_toggle_changes_current_session_q_behavior_both_ways() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.stay_alive = true;
    app.attached = true;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));
    app.settings_cursor = (0..settings::settings_total_rows())
        .find(|&idx| settings::settings_cursor_to_key(idx) == SettingKey::StayAlive)
        .expect("StayAlive setting row must exist");

    app.handle_settings_activate();
    assert!(
        !app.client.lock().unwrap().config.stay_alive,
        "settings toggle should disable Stay alive on exit for the current session too"
    );
    assert!(
        app.try_quit(),
        "disabling Stay alive on exit should make q quit even while a relay control channel exists"
    );

    app.handle_settings_activate();
    assert!(
        app.client.lock().unwrap().config.stay_alive,
        "re-enabling Stay alive on exit should restore detach-on-q in the same stay-alive session"
    );
    assert!(
        !app.try_quit(),
        "re-enabling Stay alive on exit should restore detach-on-q in the same stay-alive session"
    );

    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(&buf[..n], b"DETACH\n");
}

#[test]
fn auto_reconnect_settings_row_displays_and_toggles_current_session() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = false;
    app.settings_cursor = (0..settings::settings_total_rows())
        .find(|&idx| settings::settings_cursor_to_key(idx) == SettingKey::AutoReconnect)
        .expect("AutoReconnect setting row must exist");

    let cfg = app.client.lock().unwrap().config.clone();
    assert_eq!(
        settings::setting_label(SettingKey::AutoReconnect),
        "Auto reconnect"
    );
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "off"
    );

    app.handle_settings_activate();
    let cfg = app.client.lock().unwrap().config.clone();
    assert!(cfg.auto_reconnect);
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "on"
    );
    assert!(
        app.settings_save_at.is_some(),
        "settings toggle must use the delayed save path"
    );

    app.handle_settings_activate();
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
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
fn transport_prev_next_both_unavailable_on_single_item_queue() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
    }
    assert_eq!(app.transport_prev_next_available(), (false, false));
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
