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

    let setup = cfg.emby_setup.as_ref().filter(|s| !s.server_url.is_empty());
    let server_url = setup
        .map(|s| s.server_url.as_str())
        .unwrap_or(&cfg.server_url);
    if !server_url.is_empty() {
        let server = section!("server");
        server.insert(
            "url".to_string(),
            toml::Value::String(server_url.to_string()),
        );
        if let Some(setup) = setup {
            server.insert(
                "user_id".to_string(),
                toml::Value::String(setup.user_id.clone()),
            );
        } else {
            server.remove("user_id");
        }
    }

    let audiobookshelf = cfg
        .audiobookshelf_setup
        .as_ref()
        .filter(|setup| !setup.server_url.is_empty());
    if let Some(setup) = audiobookshelf {
        let section = section!("audiobookshelf");
        section.insert(
            "url".to_string(),
            toml::Value::String(setup.server_url.clone()),
        );
        section.insert(
            "revision".to_string(),
            toml::Value::Integer(setup.revision as i64),
        );
    } else {
        table.remove("audiobookshelf");
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

    if cfg.feeds.is_empty() {
        table.remove("feeds");
    } else {
        // Saved as a feeds array of tables (read back by `parse_feeds`).
        // Merge-in-place preserves the rest of the file untouched.
        let feeds_arr = cfg
            .feeds
            .iter()
            .map(|f| {
                let mut row = toml::map::Map::new();
                row.insert("name".to_string(), toml::Value::String(f.name.clone()));
                row.insert("url".to_string(), toml::Value::String(f.url.clone()));
                row.insert(
                    "kind".to_string(),
                    toml::Value::String(f.kind.as_str().to_string()),
                );
                toml::Value::Table(row)
            })
            .collect();
        table.insert("feeds".to_string(), toml::Value::Array(feeds_arr));
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

    let idle_feed = section!("idle_feed");
    idle_feed.insert(
        "rss_url".to_string(),
        toml::Value::String(cfg.idle_feed_rss_url.clone()),
    );
    idle_feed.insert(
        "rotation_interval_secs".to_string(),
        toml::Value::Integer(cfg.idle_feed_rotation_secs as i64),
    );

    let shared_data = section!("shared_data");
    shared_data.insert(
        "enabled".to_string(),
        toml::Value::Boolean(cfg.shared_data_enabled),
    );
    if cfg.shared_data_listen.trim().is_empty() {
        shared_data.remove("listen");
    } else {
        shared_data.insert(
            "listen".to_string(),
            toml::Value::String(cfg.shared_data_listen.clone()),
        );
    }
    if cfg.shared_data_tls_cert_path.trim().is_empty() {
        shared_data.remove("tls_cert_path");
    } else {
        shared_data.insert(
            "tls_cert_path".to_string(),
            toml::Value::String(cfg.shared_data_tls_cert_path.clone()),
        );
    }
    if cfg.shared_data_tls_key_path.trim().is_empty() {
        shared_data.remove("tls_key_path");
    } else {
        shared_data.insert(
            "tls_key_path".to_string(),
            toml::Value::String(cfg.shared_data_tls_key_path.clone()),
        );
    }
    if cfg.shared_data_endpoint.trim().is_empty() {
        shared_data.remove("endpoint");
    } else {
        shared_data.insert(
            "endpoint".to_string(),
            toml::Value::String(cfg.shared_data_endpoint.clone()),
        );
    }

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
    match cfg.audio_pipe_playout_delay_ms {
        Some(delay_ms) => {
            mbvd.insert(
                "audio_pipe_playout_delay_ms".to_string(),
                toml::Value::Integer(delay_ms as i64),
            );
        }
        None => {
            mbvd.remove("audio_pipe_playout_delay_ms");
        }
    }
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
