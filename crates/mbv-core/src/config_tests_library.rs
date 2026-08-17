#[test]
fn parse_music_levels_group_album() {
    let toml = "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"group\", \"album\"]";
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.music_levels, vec!["group", "album"]);
}

#[test]
fn parse_music_levels_album_only() {
    let toml = "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"album\"]";
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.music_levels, vec!["album"]);
}

#[test]
fn parse_music_levels_missing_defaults_empty() {
    let toml = "[server]\nurl = \"http://host\"";
    assert!(parse_config(toml).unwrap().music_levels.is_empty());
}

// always_play_next and start_on_queue live in [queue].
#[test]
fn parse_always_play_next_in_wrong_section_is_ignored() {
    let toml = "[server]\nurl = \"http://host\"\nalways_play_next = true";
    assert!(
        !parse_config(toml).unwrap().always_play_next,
        "always_play_next must be in [queue], not [server]"
    );
}

#[test]
fn load_queue_state_backfills_missing_cursor() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    let temp = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state_dir = temp.join("mbv");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("XDG_STATE_HOME", &temp);
    std::fs::write(
            state_dir.join("queue_state.json"),
            r#"{"source":{"type":"unknown"},"last_played_item_id":null,"last_played_completed":false,"positions":{}}"#,
        )
        .unwrap();

    // Pre-"full items" on-disk files have no `items` field at all; it must
    // default to empty rather than fail to load.
    let state = load_queue_state().expect("queue state missing newer fields should still load");
    assert!(state.items.is_empty());
    assert_eq!(state.cursor, 0);

    std::env::remove_var("XDG_STATE_HOME");
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn parse_feeds_basic() {
    let toml = r#"
[server]
url = "http://host"
[[feeds]]
name = "Nova"
url = "https://novaramedia.com/feed/"
kind = "video"
[[feeds]]
name = "Radio"
url = "https://example.com/podcast.xml"
kind = "audio"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.feeds.len(), 2);
    assert_eq!(cfg.feeds[0].name, "Nova");
    assert_eq!(cfg.feeds[0].url, "https://novaramedia.com/feed/");
    assert_eq!(cfg.feeds[0].kind, FeedKind::Video);
    assert_eq!(cfg.feeds[1].name, "Radio");
    assert_eq!(cfg.feeds[1].kind, FeedKind::Audio);
}

#[test]
fn parse_feeds_tolerates_partial_rows() {
    let toml = r#"
[server]
url = "http://host"
[[feeds]]
name = "No URL"
[[feeds]]
url = "https://host.example/feed.xml"
[[feeds]]
name = "Odd kind"
url = "https://odd.example/rss"
kind = "podcast"
"#;
    let cfg = parse_config(toml).unwrap();
    // URL-less row skipped; name-less row falls back to the host;
    // unknown kind defaults to Video.
    assert_eq!(cfg.feeds.len(), 2);
    assert_eq!(cfg.feeds[0].name, "host.example");
    assert_eq!(cfg.feeds[0].kind, FeedKind::Video);
    assert_eq!(cfg.feeds[1].name, "Odd kind");
    assert_eq!(cfg.feeds[1].kind, FeedKind::Video);
}

#[test]
fn parse_feeds_without_server_section() {
    // No [server] at all: feeds still parse, everything else defaults.
    let toml = r#"
[[feeds]]
name = "Only"
url = "https://only.example/feed"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.server_url, "");
    assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
    assert_eq!(cfg.feeds.len(), 1);
    assert_eq!(cfg.feeds[0].name, "Only");
}

#[test]
fn parse_no_feeds_defaults_empty() {
    let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
    assert!(cfg.feeds.is_empty());
}

#[test]
fn save_config_settings_round_trips_feeds_and_preserves_rest() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");
    std::fs::write(
        config_path(),
        r#"
[server]
url = "http://localhost:8096"

[session]
auto_reconnect = true

[[feeds]]
name = "Nova"
url = "https://novaramedia.com/feed/"
kind = "video"
"#,
    )
    .unwrap();

    let cfg = load_config().unwrap();
    assert!(cfg.auto_reconnect);
    assert_eq!(cfg.feeds.len(), 1);
    assert_eq!(cfg.feeds[0].name, "Nova");
    assert_eq!(cfg.feeds[0].kind, FeedKind::Video);

    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    let reparsed = parse_config(&saved).unwrap();
    assert!(reparsed.auto_reconnect, "unrelated keys preserved");
    assert_eq!(reparsed.feeds.len(), 1);
    assert_eq!(reparsed.feeds[0].name, "Nova");
    assert_eq!(reparsed.feeds[0].kind, FeedKind::Video);

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_config_settings_removes_feeds_key_when_empty() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");
    std::fs::write(
        config_path(),
        r#"
[server]
url = "http://localhost:8096"
[[feeds]]
name = "Nova"
url = "https://example.com/feed/"
"#,
    )
    .unwrap();

    let mut cfg = load_config().unwrap();
    assert_eq!(cfg.feeds.len(), 1);
    cfg.feeds.clear();
    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(
        !saved.contains("feeds"),
        "empty feeds must remove the key:\n{saved}"
    );
    let reparsed = parse_config(&saved).unwrap();
    assert!(reparsed.feeds.is_empty());
    assert_eq!(reparsed.server_url, "http://localhost:8096");

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

// ── Service-independent startup tests (tasks 1.1–1.4) ────────────────────

#[test]
fn non_emby_config_sections_parse_and_save_without_server() {
    // Non-[server] sections (idle_feed, mbvd, library, playback, feeds,
    // library_routes) must survive both parse and save+reparse when
    // no [server] section is present.
    let toml = r#"
[idle_feed]
rss_url = "https://custom.example/feed"
rotation_interval_secs = 30

[library]
feed_view_libraries = ["YouTube"]

[mbvd]
broadcast_ms = 250

[playback]
show_systray_icon = false

[[feeds]]
name = "Feed A"
url = "https://a.example/feed"
kind = "audio"
"#;

    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.server_url, "", "no [server] means empty server_url");
    assert_eq!(cfg.idle_feed_rss_url, "https://custom.example/feed");
    assert_eq!(cfg.idle_feed_rotation_secs, 30);
    assert_eq!(cfg.feed_view_libraries, vec!["youtube"]);
    assert_eq!(cfg.daemon_broadcast_ms, 250);
    assert!(!cfg.show_systray_icon);
    assert_eq!(cfg.feeds.len(), 1);
    assert_eq!(cfg.feeds[0].name, "Feed A");

    // Save and reparse: all non-Emby fields must survive.
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-no-server-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");

    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    // The saved file must NOT contain a [server] section.
    assert!(
        !saved.contains("[server]"),
        "config without server_url must not write [server]:\n{saved}"
    );

    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.server_url, "");
    assert_eq!(reparsed.idle_feed_rss_url, "https://custom.example/feed");
    assert_eq!(reparsed.idle_feed_rotation_secs, 30);
    assert_eq!(reparsed.feed_view_libraries, vec!["youtube"]);
    assert_eq!(reparsed.daemon_broadcast_ms, 250);
    assert!(!reparsed.show_systray_icon);
    assert_eq!(reparsed.feeds.len(), 1);
    assert_eq!(reparsed.feeds[0].name, "Feed A");

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn non_emby_config_sections_including_library_routes_round_trip_without_server() {
    let toml = r#"
[library_routes]
music = "tcp://192.168.0.104:47788"

[queue]
consume_audio = true
"#;

    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.server_url, "");
    assert!(cfg.consume_audio);
    assert_eq!(
        cfg.library_routes.get("music").map(String::as_str),
        Some("tcp://192.168.0.104:47788")
    );

    let _g = SYS_ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "mbv-no-server-routes-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("mbv")).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");

    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(
        !saved.contains("[server]"),
        "config with library_routes but no server must not write [server]:\n{saved}"
    );
    assert!(
        saved.contains("library_routes"),
        "library_routes must survive"
    );
    assert!(
        saved.contains("consume_audio"),
        "queue section must survive"
    );

    let reparsed = parse_config(&saved).unwrap();
    assert_eq!(reparsed.server_url, "");
    assert!(reparsed.consume_audio);
    assert_eq!(
        reparsed.library_routes.get("music").map(String::as_str),
        Some("tcp://192.168.0.104:47788")
    );

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn emby_setup_round_trips_url_and_user_id_without_secret_in_config() {
    let _guard = TestStateDirGuard::new();
    let setup = EmbySetup::new("http://emby.example/", "user-42");
    save_emby_setup(&setup).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    assert!(saved.contains("user_id = \"user-42\""));
    assert!(!saved.contains("token"));
    let config = load_config().unwrap();
    assert_eq!(
        config.emby_setup,
        Some(EmbySetup::new("http://emby.example", "user-42"))
    );
}
