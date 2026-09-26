use super::*;

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
#[rstest]
#[case::render_interval_is_fast_while_a_card_image_fetch_is_in_flight(
    true,
    Duration::from_millis(150)
)]
#[case::render_interval_is_slow_when_idle_with_no_fetches_in_flight(false, Duration::from_secs(1))]
fn render_interval(#[case] image_loading: bool, #[case] expected: Duration) {
    let mut app = make_app_stub();
    if image_loading {
        app.card_image_loading.insert("movie-1:cmp_primary".into());
    }
    assert_eq!(app.render_interval(), expected);
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

#[rstest]
#[case::transport_prev_next_unavailable_when_player_inactive(false, 0, 0, false, (false, false))]
#[case::transport_prev_next_both_available_mid_queue(true, 3, 1, false, (true, true))]
#[case::transport_prev_unavailable_on_first_item(true, 3, 0, false, (false, true))]
#[case::transport_next_unavailable_on_last_item(true, 3, 2, false, (true, false))]
#[case::transport_prev_next_both_available_for_connected_remote_session_regardless_of_local_status(true, 3, 2, true, (true, true))]
fn transport_prev_next(
    #[case] active: bool,
    #[case] queue_len: usize,
    #[case] current_idx: usize,
    #[case] connected: bool,
    #[case] expected: (bool, bool),
) {
    let mut app = make_app_stub();
    if connected {
        app.connected_session_id = Some("session-1".into());
    }
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = active;
        st.queue_len = queue_len;
        st.current_idx = current_idx;
    };
    assert_eq!(app.transport_prev_next_available(), expected);
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
