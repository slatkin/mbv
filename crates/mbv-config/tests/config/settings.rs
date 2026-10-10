use mbv_config::{Config, parse_config};

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

#[test]
fn parse_missing_server_section_returns_default() {
    let cfg = parse_config("[mpv]\nshow_audio_window = false").unwrap();
    assert_eq!(cfg.server_url, "");
    assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
}

#[test]
fn parse_empty_string_returns_default() {
    let cfg = parse_config("").unwrap();
    assert_eq!(cfg.server_url, "");
}

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

#[test]
fn parse_audio_pipe_defaults() {
    let cfg = parse_config("").unwrap();
    assert!(!cfg.audio_pipe_enabled);
    // Default now names a per-user runtime directory (issue #918), so it is
    // asserted against the helper, not a fixed /tmp path.
    assert_eq!(cfg.audio_pipe_path, mbv_config::default_audio_pipe_path());
    assert_eq!(cfg.audio_pipe_samplerate, 192_000);
    assert_eq!(cfg.audio_pipe_bitdepth, 32);
    assert_eq!(cfg.audio_pipe_playout_delay_ms, None);
}

#[test]
fn negative_audio_pipe_playout_delay_is_rejected() {
    let error = parse_config(
        "[server]\nurl = \"http://localhost\"\n[mpv]\naudio_pipe_playout_delay_ms = -1",
    )
    .unwrap_err();
    assert!(error.to_string().contains("audio_pipe_playout_delay_ms"));
}

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
        ("audio_device = true\n", Err("audio_device")), // non-string value
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
                assert!(
                    error.to_string().contains(needle),
                    "toml: {toml:?} error: {error}"
                );
            }
        }
    }
    // Bare mode and the Local daemon never apply this value; parsing is
    // owner-agnostic, so the packaged-daemon default above is also
    // Config's unconditional default (see player-runtime tests for the
    // owner-scoped output selection that actually differs).
    assert_eq!(Config::default().audio_device, "alsa");
}

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

#[test]
fn parse_daemon_broadcast_ms_clamps_to_ceiling() {
    // Regression for issue #922: a mistyped broadcast integer fills the
    // i64 range and previously became an effectively-never sleep on the
    // daemon broadcast thread; parse clamps it to 60_000 ms.
    let toml = r#"
[server]
url = "http://localhost:8096"
[mbvd]
broadcast_ms = 9223372036854775807
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.daemon_broadcast_ms, 60_000);
}

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

#[test]
fn parse_consume_audio_and_autosave_default_to_false() {
    let cfg = parse_config("").unwrap();
    assert!(!cfg.consume_audio);
    assert!(!cfg.save_playlist_on_consume_audio);
}

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
fn parse_default_library_routes_when_absent() {
    let toml = r#"
[server]
url = "http://host"
"#;
    let cfg = parse_config(toml).unwrap();
    assert!(cfg.library_routes.is_empty());
}

#[test]
fn parse_invalid_toml_errors() {
    let error = parse_config("not [ valid toml !!!").unwrap_err();
    assert!(error.is_parse());
}
