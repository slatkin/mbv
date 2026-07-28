pub fn load_config() -> Result<Config, String> {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(Config::default()),
    };
    parse_config(&text).map_err(|e| format!("Config parse error in {:?}: {e}", path))
}

pub fn parse_config(text: &str) -> Result<Config, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;

    let server = match doc.get("server") {
        Some(s) => s,
        None => return Ok(Config::default()),
    };

    let get_str = |section: &toml::Value, key: &str| -> String {
        section
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let misc = doc.get("mpv");
    let mbvd = doc.get("mbvd");
    let session = doc.get("session");
    let library = doc.get("library");
    let display = doc.get("display");
    let playback = doc.get("playback");
    let queue = doc.get("queue");
    let music = doc.get("library").and_then(|l| l.get("music"));

    let hidden_libraries: Vec<String> = library
        .and_then(|m| m.get("hidden_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_else(|| vec!["live tv".into()]);

    let hidden_latest: Vec<String> = library
        .and_then(|m| m.get("hidden_latest"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let show_audio_window = misc
        .and_then(|m| m.get("show_audio_window"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let use_mpv_config = misc
        .and_then(|m| m.get("use_mpv_config"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let audio_pipe_enabled = misc
        .and_then(|m| m.get("audio_pipe_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let audio_pipe_path = misc
        .and_then(|m| m.get("audio_pipe_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("/tmp/mbv-pipe")
        .to_string();

    let audio_pipe_samplerate = misc
        .and_then(|m| m.get("audio_pipe_samplerate"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u32)
        .unwrap_or(192_000);
    let audio_pipe_bitdepth = misc
        .and_then(|m| m.get("audio_pipe_bitdepth"))
        .and_then(|v| v.as_integer())
        .map(|v| match v {
            16 | 24 | 32 => v as u8,
            _ => 32,
        })
        .unwrap_or(32);

    let spectrum_snapserver_host = mbvd
        .and_then(|m| m.get("spectrum_snapserver_host"))
        .and_then(|v| v.as_str())
        .unwrap_or("127.0.0.1")
        .to_string();
    let spectrum_snapserver_port = mbvd
        .and_then(|m| m.get("spectrum_snapserver_port"))
        .and_then(|v| v.as_integer())
        .map(|v| v.clamp(1, u16::MAX as i64) as u16)
        .unwrap_or(1704);
    let spectrum_snapclient_host_id = mbvd
        .and_then(|m| m.get("spectrum_snapclient_host_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("puffin-balls")
        .to_string();
    let spectrum_snapclient_instance = mbvd
        .and_then(|m| m.get("spectrum_snapclient_instance"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(0) as u32)
        .unwrap_or(2);
    let spectrum_fifo_path = mbvd
        .and_then(|m| m.get("spectrum_fifo_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("/tmp/mbv-spectrum.fifo")
        .to_string();

    let always_play_next = queue
        .and_then(|q| q.get("always_play_next"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let consume_videos = queue
        .and_then(|q| q.get("consume_videos"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let consume_audio = queue
        .and_then(|q| q.get("consume_audio"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_skip_intro = session
        .and_then(|m| m.get("always_skip_intro"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let show_systray_icon = playback
        .and_then(|d| d.get("show_systray_icon"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let stay_alive = session
        .and_then(|m| m.get("stay_alive"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let auto_reconnect = session
        .and_then(|m| m.get("auto_reconnect"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let save_playlist_on_quit = session
        .and_then(|m| m.get("save_playlist_on_quit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let no_scripts = misc
        .and_then(|m| m.get("no_scripts"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let autoload = misc
        .and_then(|m| m.get("autoload"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let music_levels: Vec<String> = music
        .and_then(|m| m.get("levels"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let system_notifications = display
        .and_then(|m| m.get("system_notifications"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let save_playlist_on_consume = queue
        .and_then(|q| q.get("save_playlist_on_consume"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let save_playlist_on_consume_audio = queue
        .and_then(|q| q.get("save_playlist_on_consume_audio"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let subtitle_mode = playback
        .and_then(|p| p.get("subtitle_mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let subtitle_lang = playback
        .and_then(|p| p.get("subtitle_lang"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let audio_lang = playback
        .and_then(|p| p.get("audio_lang"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let my_languages: Vec<String> = playback
        .and_then(|p| p.get("my_languages"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let progress_interval_secs = session
        .and_then(|m| m.get("progress_interval_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(10);

    let quit_timeout_secs = session
        .and_then(|m| m.get("quit_timeout_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(5);

    let daemon_broadcast_ms = mbvd
        .and_then(|d| d.get("broadcast_ms"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(100) as u64)
        .unwrap_or(500);

    let daemon_client_endpoint = mbvd
        .and_then(|d| d.get("client"))
        .and_then(|c| c.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let daemon_server_tcp_listen = mbvd
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("tcp_listen"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_daemon_server_tcp_listen);

    let feed_view_libraries: Vec<String> = library
        .and_then(|m| m.get("feed_view_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let library_routes: std::collections::HashMap<String, String> = doc
        .get("library_routes")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Ok(Config {
        server_url: get_str(server, "url").trim_end_matches('/').to_string(),
        username: String::new(),
        password: String::new(),
        api_key: String::new(),
        hidden_libraries,
        hidden_latest,
        show_audio_window,
        use_mpv_config,
        audio_pipe_enabled,
        audio_pipe_path,
        audio_pipe_samplerate,
        audio_pipe_bitdepth,
        spectrum_snapserver_host,
        spectrum_snapserver_port,
        spectrum_snapclient_host_id,
        spectrum_snapclient_instance,
        spectrum_fifo_path,
        always_play_next,
        consume_videos,
        consume_audio,
        always_skip_intro,
        show_systray_icon,
        no_scripts,
        stay_alive,
        save_playlist_on_quit,
        autoload,
        music_levels,
        system_notifications,
        save_playlist_on_consume,
        save_playlist_on_consume_audio,
        subtitle_mode,
        subtitle_lang,
        audio_lang,
        my_languages,
        feed_view_libraries,
        library_routes,
        progress_interval_secs,
        quit_timeout_secs,
        daemon_broadcast_ms,
        daemon_client_endpoint,
        daemon_server_tcp_listen,
        auto_reconnect,
    })
}
