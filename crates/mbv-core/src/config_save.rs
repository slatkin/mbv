fn save_config_settings_at(cfg: &Config, path: &std::path::Path) -> Result<(), String> {
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return Err(format!("update {}: root is not a table", path.display())),
    };

    macro_rules! section {
        ($name:literal) => {
            table
                .entry($name.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
                .as_table_mut()
                .unwrap()
        };
    }

    if !cfg.server_url.is_empty() {
        let server = section!("server");
        server.insert(
            "url".to_string(),
            toml::Value::String(cfg.server_url.clone()),
        );
    }

    let session = section!("session");
    session.insert(
        "stay_alive".to_string(),
        toml::Value::Boolean(cfg.stay_alive),
    );
    session.insert(
        "auto_reconnect".to_string(),
        toml::Value::Boolean(cfg.auto_reconnect),
    );
    session.insert(
        "save_playlist_on_quit".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_quit),
    );
    session.insert(
        "always_skip_intro".to_string(),
        toml::Value::Boolean(cfg.always_skip_intro),
    );
    session.insert(
        "quit_timeout_secs".to_string(),
        toml::Value::Integer(cfg.quit_timeout_secs as i64),
    );
    session.insert(
        "progress_interval_secs".to_string(),
        toml::Value::Integer(cfg.progress_interval_secs as i64),
    );

    let library = section!("library");
    library.insert(
        "hidden_libraries".to_string(),
        toml::Value::Array(
            cfg.hidden_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    library.insert(
        "hidden_latest".to_string(),
        toml::Value::Array(
            cfg.hidden_latest
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    library.insert(
        "feed_view_libraries".to_string(),
        toml::Value::Array(
            cfg.feed_view_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );

    let display = section!("display");
    display.insert(
        "system_notifications".to_string(),
        toml::Value::Boolean(cfg.system_notifications),
    );

    if !cfg.music_levels.is_empty() {
        let library = section!("library");
        let music = library
            .entry("music".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .unwrap();
        music.insert(
            "levels".to_string(),
            toml::Value::Array(
                cfg.music_levels
                    .iter()
                    .map(|s| toml::Value::String(s.clone()))
                    .collect(),
            ),
        );
    }

    if cfg.library_routes.is_empty() {
        table.remove("library_routes");
    } else {
        let mut routes_table = toml::map::Map::new();
        for (library, device) in &cfg.library_routes {
            routes_table.insert(library.clone(), toml::Value::String(device.clone()));
        }
        table.insert(
            "library_routes".to_string(),
            toml::Value::Table(routes_table),
        );
    }

    let queue = section!("queue");
    queue.insert(
        "always_play_next".to_string(),
        toml::Value::Boolean(cfg.always_play_next),
    );
    queue.insert(
        "consume_videos".to_string(),
        toml::Value::Boolean(cfg.consume_videos),
    );
    queue.insert(
        "consume_audio".to_string(),
        toml::Value::Boolean(cfg.consume_audio),
    );
    queue.insert(
        "save_playlist_on_consume".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_consume),
    );
    queue.insert(
        "save_playlist_on_consume_audio".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_consume_audio),
    );

    let mpv = section!("mpv");
    mpv.insert(
        "show_audio_window".to_string(),
        toml::Value::Boolean(cfg.show_audio_window),
    );
    mpv.insert(
        "use_mpv_config".to_string(),
        toml::Value::Boolean(cfg.use_mpv_config),
    );
    mpv.insert(
        "no_scripts".to_string(),
        toml::Value::Boolean(cfg.no_scripts),
    );
    mpv.insert("autoload".to_string(), toml::Value::Boolean(cfg.autoload));

    let mbvd = section!("mbvd");
    mbvd.insert(
        "broadcast_ms".to_string(),
        toml::Value::Integer(cfg.daemon_broadcast_ms as i64),
    );
    mbvd.insert(
        "audio_pipe_enabled".to_string(),
        toml::Value::Boolean(cfg.audio_pipe_enabled),
    );
    mbvd.insert(
        "audio_pipe_path".to_string(),
        toml::Value::String(cfg.audio_pipe_path.clone()),
    );
    mbvd.insert(
        "audio_pipe_samplerate".to_string(),
        toml::Value::Integer(cfg.audio_pipe_samplerate as i64),
    );
    mbvd.insert(
        "audio_pipe_bitdepth".to_string(),
        toml::Value::Integer(cfg.audio_pipe_bitdepth as i64),
    );
    let mbvd_client = mbvd
        .entry("client".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_client_endpoint.trim().is_empty() {
        mbvd_client.remove("endpoint");
    } else {
        mbvd_client.insert(
            "endpoint".to_string(),
            toml::Value::String(cfg.daemon_client_endpoint.clone()),
        );
    }
    let mbvd_server = mbvd
        .entry("server".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_server_tcp_listen.trim().is_empty() {
        mbvd_server.remove("tcp_listen");
    } else {
        mbvd_server.insert(
            "tcp_listen".to_string(),
            toml::Value::String(cfg.daemon_server_tcp_listen.clone()),
        );
    }

    let playback = section!("playback");
    playback.insert(
        "show_systray_icon".to_string(),
        toml::Value::Boolean(cfg.show_systray_icon),
    );
    if cfg.subtitle_mode.is_empty() {
        playback.remove("subtitle_mode");
    } else {
        playback.insert(
            "subtitle_mode".to_string(),
            toml::Value::String(cfg.subtitle_mode.clone()),
        );
    }
    if cfg.subtitle_lang.is_empty() {
        playback.remove("subtitle_lang");
    } else {
        playback.insert(
            "subtitle_lang".to_string(),
            toml::Value::String(cfg.subtitle_lang.clone()),
        );
    }
    if cfg.audio_lang.is_empty() {
        playback.remove("audio_lang");
    } else {
        playback.insert(
            "audio_lang".to_string(),
            toml::Value::String(cfg.audio_lang.clone()),
        );
    }
    if cfg.my_languages.is_empty() {
        playback.remove("my_languages");
    } else {
        playback.insert(
            "my_languages".to_string(),
            toml::Value::Array(
                cfg.my_languages
                    .iter()
                    .map(|s| toml::Value::String(s.clone()))
                    .collect(),
            ),
        );
    }
    let s = toml::to_string(&doc).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    write_config_text_at(path, &s)
}

fn write_config_text_at(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create directory {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn save_config_settings(cfg: &Config) -> Result<(), String> {
    save_config_settings_at(cfg, &config_path())
}
