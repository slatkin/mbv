#[cfg(test)]
#[test]
fn parse_full_config() {
    let toml = r#"
[server]
url = "http://localhost:8096/"
[library]
hidden_libraries = ["Live TV", "Podcasts", "Music"]
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.server_url, "http://localhost:8096"); // trailing slash stripped
    assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts", "music"]);
}

#[cfg(test)]
#[test]
fn parse_missing_server_section_returns_default() {
    let cfg = parse_config("[mpv]\nshow_audio_window = false").unwrap();
    assert_eq!(cfg.server_url, "");
    assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
}

#[cfg(test)]
#[test]
fn parse_empty_string_returns_default() {
    let cfg = parse_config("").unwrap();
    assert_eq!(cfg.server_url, "");
}

#[cfg(test)]
#[test]
fn parse_audio_pipe_settings() {
    let toml = r#"
[server]
url = "http://localhost:8096"
[mpv]
audio_pipe_enabled = true
        audio_pipe_path = "/tmp/custom-pipe"
audio_pipe_samplerate = 96000
audio_pipe_bitdepth = 16
audio_pipe_playout_delay_ms = 1250
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.audio_pipe_enabled);
    assert_eq!(cfg.audio_pipe_path, "/tmp/custom-pipe");
    assert_eq!(cfg.audio_pipe_samplerate, 96000);
    assert_eq!(cfg.audio_pipe_bitdepth, 16);
    assert_eq!(cfg.audio_pipe_playout_delay_ms, Some(1250));
}

#[cfg(test)]
#[test]
fn parse_audio_pipe_defaults() {
    let cfg = parse_config("").unwrap();
    assert!(!cfg.audio_pipe_enabled);
    assert_eq!(cfg.audio_pipe_path, "/tmp/mbv-pipe");
    assert_eq!(cfg.audio_pipe_samplerate, 192_000);
    assert_eq!(cfg.audio_pipe_bitdepth, 32);
    assert_eq!(cfg.audio_pipe_playout_delay_ms, None);
}

#[cfg(test)]
#[test]
fn negative_audio_pipe_playout_delay_is_rejected() {
    let error = parse_config(
        "[server]\nurl = \"http://localhost\"\n[mpv]\naudio_pipe_playout_delay_ms = -1",
    )
    .unwrap_err();
    assert!(error.contains("audio_pipe_playout_delay_ms"));
}

#[cfg(test)]
#[test]
fn audio_device_configuration_table() {
    // (mpv-section body, expected outcome: Ok(resolved value) or Err(substring))
    let cases: &[(&str, Result<&str, &str>)] = &[
        ("", Ok("alsa")),                          // absent -> default
        ("audio_device = \"alsa\"\n", Ok("alsa")), // explicit default
        (
            "audio_device = \"alsa/hw:Loopback,0,0\"\n",
            Ok("alsa/hw:Loopback,0,0"),
        ), // exact endpoint
        ("audio_device = \"\"\n", Err("audio_device")), // empty identifier
        ("audio_device = \"pipewire\"\n", Err("audio_device")), // non-alsa output
    ];
    for (mpv_body, expected) in cases {
        let toml = format!("[server]\nurl = \"http://localhost\"\n[mpv]\n{mpv_body}");
        match expected {
            Ok(resolved) => {
                let cfg = parse_config(&toml).unwrap();
                assert_eq!(cfg.audio_device, *resolved, "toml: {toml:?}");
            }
            Err(needle) => {
                let error = parse_config(&toml).unwrap_err();
                assert!(error.contains(needle), "toml: {toml:?} error: {error}");
            }
        }
    }
    // Bare mode and the Local daemon never apply this value; parsing is
    // owner-agnostic, so the packaged-daemon default above is also
    // Config's unconditional default (see player-runtime tests for the
    // owner-scoped output selection that actually differs).
    assert_eq!(Config::default().audio_device, "alsa");
}

#[cfg(test)]
#[test]
fn parse_daemon_client_endpoint() {
    let toml = r#"
[server]
url = "http://localhost:8096"
[mbvd.client]
endpoint = "unix:///tmp/mbv.sock"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.daemon_client_endpoint, "unix:///tmp/mbv.sock");
}

#[cfg(test)]
#[test]
fn parse_daemon_server_tcp_listen() {
    let toml = r#"
[server]
url = "http://localhost:8096"
[mbvd.server]
tcp_listen = "0.0.0.0:8890"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.daemon_server_tcp_listen, "0.0.0.0:8890");
}

#[cfg(test)]
#[test]
fn parse_quit_timeout_defaults_and_clamps() {
    let cfg = parse_config("[server]\nurl = \"http://localhost:8096\"").unwrap();
    assert_eq!(cfg.quit_timeout_secs, 5);

    let cfg = parse_config(
        r#"
[server]
url = "http://localhost:8096"
[session]
quit_timeout_secs = 0
"#,
    )
    .unwrap();
    assert_eq!(cfg.quit_timeout_secs, 1);

    let cfg = parse_config(
        r#"
[server]
url = "http://localhost:8096"
[session]
quit_timeout_secs = -10
"#,
    )
    .unwrap();
    assert_eq!(cfg.quit_timeout_secs, 1);
}

#[cfg(test)]
#[test]
fn parse_consume_audio_and_autosave_default_to_false() {
    let cfg = parse_config("").unwrap();
    assert!(!cfg.consume_audio);
    assert!(!cfg.save_playlist_on_consume_audio);
}

#[cfg(test)]
#[test]
fn parse_consume_audio_and_autosave_flags() {
    let toml = r#"
[server]
url = "http://localhost:8096"
[queue]
consume_audio = true
save_playlist_on_consume_audio = true
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.consume_audio);
    assert!(cfg.save_playlist_on_consume_audio);
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
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");

    let mut cfg = Config {
        server_url: "http://localhost:8096".into(),
        ..Default::default()
    };
    cfg.consume_audio = true;
    cfg.save_playlist_on_consume_audio = true;
    cfg.quit_timeout_secs = 7;
    save_config_settings(&cfg).unwrap();

    let saved = std::fs::read_to_string(config_path()).unwrap();
    let reparsed = parse_config(&saved).unwrap();
    assert!(reparsed.consume_audio);
    assert!(reparsed.save_playlist_on_consume_audio);
    assert_eq!(reparsed.quit_timeout_secs, 7);

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_hidden_libraries_lowercased() {
    let toml = r#"
[server]
url = "http://host"
[library]
hidden_libraries = ["Live TV", "MOVIES"]
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.hidden_libraries, vec!["live tv", "movies"]);
}

#[test]
fn parse_default_hidden_libraries_when_absent() {
    let toml = "[server]\nurl = \"http://host\"";
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
}

#[test]
fn parse_hidden_latest_lowercased() {
    let toml = r#"
[server]
url = "http://host"
[library]
hidden_latest = ["Movies", "TV SHOWS"]
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.hidden_latest, vec!["movies", "tv shows"]);
}

#[test]
fn parse_default_hidden_latest_when_absent() {
    let toml = "[server]\nurl = \"http://host\"";
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.hidden_latest.is_empty());
}

#[test]
fn parse_library_routes_lowercased_keys() {
    let toml = r#"
[server]
url = "http://host"
[library_routes]
Music = "tcp://192.168.0.104:47788"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(
        cfg.library_routes.get("music").map(String::as_str),
        Some("tcp://192.168.0.104:47788")
    );
}

#[test]
fn parse_library_routes_ignores_legacy_wildcard_key() {
    // "*" is no longer a wildcard -- it's just an (unusable) library
    // name like any other, since #239 dropped the catch-all.
    let toml = r#"
[server]
url = "http://host"
[library_routes]
"*" = "tcp://192.168.0.104:47788"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(
        cfg.library_routes.get("*").map(String::as_str),
        Some("tcp://192.168.0.104:47788")
    );
    assert_eq!(resolve_library_route(&cfg.library_routes, "movies"), None);
}

#[test]
fn parse_auto_reconnect_true() {
    let toml = r#"
[server]
url = "http://x"

[session]
auto_reconnect = true
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.auto_reconnect);
}

#[test]
fn parse_auto_reconnect_defaults_false_when_absent() {
    let toml = r#"
[server]
url = "http://x"
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(!cfg.auto_reconnect);
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
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");

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

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
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
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::remove_var("MBV_SYSTEM");
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

    std::env::remove_var("XDG_CONFIG_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_default_library_routes_when_absent() {
    let toml = r#"
[server]
url = "http://host"
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.library_routes.is_empty());
}

#[test]
fn resolve_library_route_has_no_wildcard_fallback() {
    let mut routes = std::collections::HashMap::new();
    routes.insert("music".to_string(), "tcp://192.168.0.104:47788".to_string());
    assert_eq!(
        resolve_library_route(&routes, "Music"),
        Some(crate::remote_player::DaemonEndpoint::Tcp(
            "192.168.0.104:47788".parse().unwrap()
        ))
    );
    assert_eq!(resolve_library_route(&routes, "movies"), None);
}

#[test]
fn resolve_library_route_rejects_a_bare_device_name_as_malformed() {
    // A stale pre-#256 config entry (device name, no scheme) must
    // NOT silently resolve -- DaemonEndpoint::parse would otherwise
    // accept it as a bogus Unix(PathBuf) socket path. Library routing
    // is tcp://-only (#239 addendum), so anything that doesn't parse
    // to Tcp(_) is treated as malformed: logged and skipped.
    let mut routes = std::collections::HashMap::new();
    routes.insert("music".to_string(), "living-room-pc".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
}

#[test]
fn resolve_library_route_rejects_unix_and_local_endpoints() {
    // Library routing is remote-only -- a unix:// or bare "local"
    // value is well-formed as a DaemonEndpoint but not a valid
    // library route, so it must still resolve to None.
    let mut routes = std::collections::HashMap::new();
    routes.insert("music".to_string(), "unix:///run/mbvd.sock".to_string());
    routes.insert("movies".to_string(), "local".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
    assert_eq!(resolve_library_route(&routes, "movies"), None);
}

#[test]
fn parse_invalid_toml_errors() {
    assert!(parse_config("not [ valid toml !!!").is_err());
}
