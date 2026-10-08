#[cfg(test)]
use crate::tests::SYS_ENV_LOCK;
#[cfg(test)]
use crate::{
    Config, PanelAccentColor, PanelConfig, PanelSide, config_path, load_config, parse_config,
    save_config_settings,
};
#[cfg(test)]
use std::num::NonZeroU16;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

// The pure `[section]`-parsing tests own the crate's public contract and
// live in the `tests/config` integration binary; these stay because they
// need `SYS_ENV_LOCK` and the `cfg(test)`-only env helpers.

#[cfg(test)]
#[test]
fn parse_video_cache_settings_and_save_round_trip() {
    let cases = [
        ("", 50, 100),
        (
            "video_cache_forward_mb = 75\nvideo_cache_back_mb = 125",
            75,
            125,
        ),
        (
            "video_cache_forward_mb = \"75\"\nvideo_cache_back_mb = 0",
            50,
            100,
        ),
        (
            "video_cache_forward_mb = 0\nvideo_cache_back_mb = -1",
            50,
            100,
        ),
        (
            "video_cache_forward_mb = -1\nvideo_cache_back_mb = 9223372036854775807",
            50,
            100,
        ),
    ];
    for (body, forward, back) in cases {
        let cfg = parse_config(&format!("[mpv]\n{body}")).unwrap();
        assert_eq!(cfg.video_cache_forward_mb, forward, "toml: {body:?}");
        assert_eq!(cfg.video_cache_back_mb, back, "toml: {body:?}");
    }

    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");
    std::fs::write(
        config_path(),
        "[library]\nhidden_libraries = [\"Live TV\"]\nhidden_latest = [\"Movies\"]\n",
    )
    .unwrap();
    let legacy = parse_config(&std::fs::read_to_string(config_path()).unwrap()).unwrap();
    assert_eq!(legacy.hidden_libraries, vec!["live tv"]);
    let cfg = Config {
        video_cache_forward_mb: 75,
        video_cache_back_mb: 125,
        ..Default::default()
    };
    save_config_settings(&cfg).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(!saved.contains("hidden_latest"));
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.video_cache_forward_mb, 75);
    assert_eq!(reparsed.video_cache_back_mb, 125);
    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(test)]
#[test]
fn mouse_support_round_trips_and_defaults_on() {
    // Absent key parses to true (existing configs unchanged on upgrade).
    let cfg = parse_config("[display]\nsystem_notifications = true\n").unwrap();
    assert!(cfg.mouse_support);

    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");

    let cfg = Config {
        mouse_support: false,
        ..Default::default()
    };
    save_config_settings(&cfg).unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(saved.contains("mouse_support = false"));
    let reparsed = parse_config(&saved).unwrap();
    assert!(!reparsed.mouse_support);

    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(test)]
#[test]
fn save_config_settings_round_trips_consume_audio_flags() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");

    let mut cfg = Config {
        server_url: "http://localhost:8096".into(),
        ..Default::default()
    };
    cfg.consume_audio = true;
    cfg.save_playlist_on_consume_audio = true;
    cfg.quit_timeout_secs = 7;
    cfg.audio_device = "alsa/hw:Loopback,0,0".into();
    std::fs::write(config_path(), "[shared_data]\nenabled = true\n").unwrap();
    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(!saved.contains("[shared_data]"));
    let reparsed = parse_config(&saved).unwrap();
    assert!(reparsed.consume_audio);
    assert!(reparsed.save_playlist_on_consume_audio);
    assert_eq!(reparsed.quit_timeout_secs, 7);
    assert_eq!(reparsed.audio_device, "alsa/hw:Loopback,0,0");

    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_config_settings_round_trips_auto_reconnect_values() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");

    for auto_reconnect in [true, false] {
        let cfg = Config {
            server_url: "http://localhost:8096".into(),
            auto_reconnect,
            ..Default::default()
        };
        save_config_settings(&cfg).unwrap();

        let saved = std::fs::read_to_string(config_path()).unwrap();
        let reparsed = parse_config(&saved).unwrap();
        assert_eq!(reparsed.auto_reconnect, auto_reconnect);
    }

    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_config_settings_preserves_general_feed_view_when_auto_reconnect_exists() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");
    std::fs::write(
        config_path(),
        r#"
[server]
url = "http://localhost:8096"

[session]
auto_reconnect = true

[library]
feed_view_libraries = ["YouTube"]
"#,
    )
    .unwrap();

    let cfg = load_config().unwrap();
    assert_eq!(cfg.feed_view_libraries, vec!["youtube"]);

    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.feed_view_libraries, vec!["youtube"]);
    assert!(
        !saved.contains("feed_view_libraries = []"),
        "saved config should not overwrite the feed view selection with none:\n{saved}"
    );

    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Contract (task 4.2 of `pin-mbv-in-pinwin`): an out-of-range or malformed
/// `[panel]` value falls back to its own default without changing the other
/// keys, and a valid `[panel]` section round-trips through save.
#[test]
fn panel_values_fall_back_per_key_and_a_valid_section_round_trips() {
    let cfg = parse_config(
        r#"
[panel]
side = "up"
cols = 0
cols_expanded = "wide"
gutter_top = 12
gutter_bottom = 3000000000
gutter_left = -5
gutter_right = "wide"
accent = "yes"
accent_color = "orange"
accent_width = 0
"#,
    )
    .unwrap();
    assert_eq!(cfg.panel.side, PanelSide::Left);
    assert_eq!(cfg.panel.cols, crate::DEFAULT_PANEL_COLS);
    assert_eq!(cfg.panel.cols_expanded, crate::DEFAULT_PANEL_COLS_EXPANDED);
    assert_eq!(cfg.panel.gutter_top, 12);
    assert_eq!(cfg.panel.gutter_bottom, 0);
    assert_eq!(cfg.panel.gutter_left, -5);
    assert_eq!(cfg.panel.gutter_right, 0);
    assert!(cfg.panel.accent);
    assert_eq!(cfg.panel.accent_color, crate::DEFAULT_PANEL_ACCENT_COLOR);
    assert_eq!(cfg.panel.accent_width, crate::DEFAULT_PANEL_ACCENT_WIDTH);

    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    crate::set_test_env_var("XDG_CONFIG_HOME", &dir);
    crate::remove_test_env_var("MBV_SYSTEM");

    let expected = PanelConfig {
        side: PanelSide::Right,
        cols: 55,
        cols_expanded: 70,
        gutter_top: 1,
        gutter_bottom: -2,
        gutter_left: 3,
        gutter_right: -4,
        accent: false,
        accent_color: PanelAccentColor([0x12, 0x34, 0x56]),
        accent_width: NonZeroU16::new(2).unwrap(),
    };
    save_config_settings(&Config {
        panel: expected,
        ..Default::default()
    })
    .unwrap();
    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(
        saved.contains("[panel]"),
        "saved config writes the panel section:\n{saved}"
    );
    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.panel, expected);

    crate::remove_test_env_var("XDG_CONFIG_HOME");
    let _ = std::fs::remove_dir_all(&dir);
}
