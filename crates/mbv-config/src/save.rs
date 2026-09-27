use super::{config_path, Config};

pub(super) fn save_config_settings_at(cfg: &Config, path: &std::path::Path) -> Result<(), String> {
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let Some(table) = doc.as_table_mut() else {
        return Err(format!("update {}: root is not a table", path.display()));
    };

    write_server_section(table, cfg);
    write_audiobookshelf_section(table, cfg);
    write_session_section(table, cfg);
    write_library_section(table, cfg);
    write_display_section(table, cfg);
    write_library_routes_section(table, cfg);
    write_feeds_section(table, cfg);
    write_queue_section(table, cfg);
    write_mpv_section(table, cfg);
    write_idle_feed_section(table, cfg);
    // Remove the retired shared-data section when rewriting an existing config.
    table.remove("shared_data");
    write_mbvd_section(table, cfg);
    write_playback_section(table, cfg);
    write_keys_section(table, cfg);

    let s = toml::to_string(&doc).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    write_config_text_at(path, &s)
}

fn section<'a>(
    table: &'a mut toml::map::Map<String, toml::Value>,
    name: &str,
) -> &'a mut toml::map::Map<String, toml::Value> {
    table
        .entry(name.to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap()
}

fn write_server_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let setup = cfg.emby_setup.as_ref().filter(|s| !s.server_url.is_empty());
    let server_url = setup.map_or(cfg.server_url.as_str(), |s| s.server_url.as_str());
    if !server_url.is_empty() {
        let server = section(table, "server");
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
}

fn write_audiobookshelf_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let audiobookshelf = cfg
        .audiobookshelf_setup
        .as_ref()
        .filter(|setup| !setup.server_url.is_empty());
    if let Some(setup) = audiobookshelf {
        let section = section(table, "audiobookshelf");
        section.insert(
            "url".to_string(),
            toml::Value::String(setup.server_url.clone()),
        );
        section.insert(
            "revision".to_string(),
            toml::Value::Integer(i64::try_from(setup.revision).unwrap_or(i64::MAX)),
        );
    } else {
        table.remove("audiobookshelf");
    }
}

fn write_session_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let session = section(table, "session");
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
        toml::Value::Integer(i64::try_from(cfg.quit_timeout_secs).unwrap_or(i64::MAX)),
    );
    session.insert(
        "progress_interval_secs".to_string(),
        toml::Value::Integer(i64::try_from(cfg.progress_interval_secs).unwrap_or(i64::MAX)),
    );
}

fn write_library_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let library = section(table, "library");
    library.insert(
        "hidden_libraries".to_string(),
        toml::Value::Array(
            cfg.hidden_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    library.remove("hidden_latest");
    library.insert(
        "feed_view_libraries".to_string(),
        toml::Value::Array(
            cfg.feed_view_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );

    if !cfg.music_levels.is_empty() {
        let music = section(library, "music");
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
}

fn write_display_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let display = section(table, "display");
    display.insert(
        "system_notifications".to_string(),
        toml::Value::Boolean(cfg.system_notifications),
    );
    display.insert(
        "mouse_support".to_string(),
        toml::Value::Boolean(cfg.mouse_support),
    );
}

fn write_library_routes_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
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
}

fn write_feeds_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
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
}

fn write_queue_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let queue = section(table, "queue");
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
}

fn write_mpv_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let mpv = section(table, "mpv");
    mpv.insert(
        "show_audio_window".to_string(),
        toml::Value::Boolean(cfg.show_audio_window),
    );
    mpv.insert(
        "use_mpv_config".to_string(),
        toml::Value::Boolean(cfg.use_mpv_config),
    );
    mpv.insert(
        "video_cache_forward_mb".to_string(),
        toml::Value::Integer(i64::from(cfg.video_cache_forward_mb)),
    );
    mpv.insert(
        "video_cache_back_mb".to_string(),
        toml::Value::Integer(i64::from(cfg.video_cache_back_mb)),
    );
    mpv.insert(
        "no_scripts".to_string(),
        toml::Value::Boolean(cfg.no_scripts),
    );
    mpv.insert("autoload".to_string(), toml::Value::Boolean(cfg.autoload));
    mpv.insert(
        "audio_device".to_string(),
        toml::Value::String(cfg.audio_device.clone()),
    );
}

fn write_idle_feed_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let idle_feed = section(table, "idle_feed");
    idle_feed.insert(
        "rss_url".to_string(),
        toml::Value::String(cfg.idle_feed_rss_url.clone()),
    );
    idle_feed.insert(
        "rotation_interval_secs".to_string(),
        toml::Value::Integer(i64::try_from(cfg.idle_feed_rotation_secs).unwrap_or(i64::MAX)),
    );
}

fn write_mbvd_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let mbvd = section(table, "mbvd");
    mbvd.insert(
        "broadcast_ms".to_string(),
        toml::Value::Integer(i64::try_from(cfg.daemon_broadcast_ms).unwrap_or(i64::MAX)),
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
        toml::Value::Integer(i64::from(cfg.audio_pipe_samplerate)),
    );
    mbvd.insert(
        "audio_pipe_bitdepth".to_string(),
        toml::Value::Integer(i64::from(cfg.audio_pipe_bitdepth)),
    );
    match cfg.audio_pipe_playout_delay_ms {
        Some(delay_ms) => {
            mbvd.insert(
                "audio_pipe_playout_delay_ms".to_string(),
                toml::Value::Integer(i64::try_from(delay_ms).unwrap_or(i64::MAX)),
            );
        }
        None => {
            mbvd.remove("audio_pipe_playout_delay_ms");
        }
    }
    let mbvd_client = section(mbvd, "client");
    if cfg.daemon_client_endpoint.trim().is_empty() {
        mbvd_client.remove("endpoint");
    } else {
        mbvd_client.insert(
            "endpoint".to_string(),
            toml::Value::String(cfg.daemon_client_endpoint.clone()),
        );
    }
    let mbvd_server = section(mbvd, "server");
    if cfg.daemon_server_tcp_listen.trim().is_empty() {
        mbvd_server.remove("tcp_listen");
    } else {
        mbvd_server.insert(
            "tcp_listen".to_string(),
            toml::Value::String(cfg.daemon_server_tcp_listen.clone()),
        );
    }
}

fn write_playback_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let playback = section(table, "playback");
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
}

// `[keys]` (change `add-configurable-keybinds`): patched like any other
// section through the read-patch-write path, in the section-outer file
// shape (design D3, lowercase section names) that `parse_raw_keybinds`
// reads back. An all-defaults configuration — or one whose sections all
// lost their entries — prunes the whole table; a section with no
// surviving entries contributes no table.
fn write_keys_section(table: &mut toml::map::Map<String, toml::Value>, cfg: &Config) {
    let mut keys_table = toml::map::Map::new();
    if let Some(prefix) = &cfg.keybinds.prefix {
        keys_table.insert(
            "prefix".to_string(),
            toml::Value::String(prefix.to_string()),
        );
    }
    for (section, bindings) in &cfg.keybinds.sections {
        if bindings.router.is_empty() && bindings.prefix.is_empty() {
            continue;
        }
        let mut section_table = toml::map::Map::new();
        for (action_id, chord) in &bindings.router {
            section_table.insert(
                action_id.to_string(),
                toml::Value::String(chord.to_string()),
            );
        }
        if !bindings.prefix.is_empty() {
            let mut prefix_table = toml::map::Map::new();
            for (action_id, chord) in &bindings.prefix {
                prefix_table.insert(
                    action_id.to_string(),
                    toml::Value::String(chord.to_string()),
                );
            }
            section_table.insert("prefix".to_string(), toml::Value::Table(prefix_table));
        }
        keys_table.insert(
            section.name().to_ascii_lowercase(),
            toml::Value::Table(section_table),
        );
    }
    if keys_table.is_empty() {
        table.remove("keys");
    } else {
        table.insert("keys".to_string(), toml::Value::Table(keys_table));
    }
}

pub(super) fn write_config_text_at(path: &std::path::Path, text: &str) -> Result<(), String> {
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
