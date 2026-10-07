use super::*;

#[rstest]
#[case::render_interval_is_fast_while_a_card_image_fetch_is_in_flight(
    true,
    Duration::from_millis(150)
)]
fn render_interval(#[case] image_loading: bool, #[case] expected: Duration) {
    let mut app = make_app_stub();
    if image_loading {
        let req = mbv_images::ImageFetchReq {
            cache_key: "movie-1:cmp_primary".into(),
            item_id: "movie-1".into(),
            series_id: String::new(),
            types: vec!["Primary".into()],
            source: mbv_images::ImageSource::Emby,
        };
        let _ = app.images.reserve_fetch(req, 0);
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

#[rstest]
#[case::transport_prev_next_unavailable_when_player_inactive(false, 0, 0, false, (false, false))]
#[case::transport_prev_unavailable_on_first_item(true, 3, 0, false, (false, true))]
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
        app.player.update_status(|st| {
            st.active = active;
            st.queue_len = queue_len;
            st.current_idx = current_idx;
        });
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
