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

    // All non-Emby sections are parsed unconditionally, even when
    // [server] is absent (feed-only / service-independent startup).
    let feeds = parse_feeds(doc.get("feeds"));
    let server = doc.get("server");
    let audiobookshelf = doc.get("audiobookshelf");

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
    let audio_pipe_playout_delay_ms = match misc
        .and_then(|m| m.get("audio_pipe_playout_delay_ms"))
        .and_then(|v| v.as_integer())
    {
        Some(value) if value < 0 => {
            return Err("mpv.audio_pipe_playout_delay_ms must be nonnegative".to_string())
        }
        Some(value) => Some(value as u64),
        None => None,
    };

    let audio_device = match misc.and_then(|m| m.get("audio_device")) {
        None => "alsa".to_string(),
        Some(value) => match value.as_str() {
            Some(value) if is_valid_audio_device(value) => value.to_string(),
            _ => {
                return Err(format!(
                    "mpv.audio_device must be \"alsa\" or start with \"alsa/\", got {value:?}"
                ))
            }
        },
    };

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

    let idle_feed = doc.get("idle_feed");
    let idle_feed_rss_url = idle_feed
        .and_then(|s| s.get("rss_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://novaramedia.com/feed/")
        .to_string();
    let idle_feed_rotation_secs = idle_feed
        .and_then(|s| s.get("rotation_interval_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(10);

    // ── Shared-data configuration ────────────────────────────────────
    let shared = doc.get("shared_data");
    let shared_data_enabled = shared
        .and_then(|s| s.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let shared_data_listen = shared
        .and_then(|s| s.get("listen"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let shared_data_tls_cert_path = shared
        .and_then(|s| s.get("tls_cert_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let shared_data_tls_key_path = shared
        .and_then(|s| s.get("tls_key_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let shared_data_endpoint = shared
        .and_then(|s| s.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let server_url = server
        .map(|s| get_str(s, "url"))
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    let user_id = server.map(|s| get_str(s, "user_id")).unwrap_or_default();
    let emby_setup = (!server_url.is_empty() && !user_id.is_empty()).then(|| {
        let mut setup = EmbySetup::new(&server_url, user_id);
        setup.revision = server
            .and_then(|s| s.get("revision"))
            .and_then(|v| v.as_integer())
            .and_then(|v| u64::try_from(v).ok())
            .filter(|revision| *revision > 0)
            .unwrap_or(setup.revision);
        setup
    });
    let audiobookshelf_url = audiobookshelf
        .map(|section| get_str(section, "url"))
        .unwrap_or_default();
    let audiobookshelf_setup = (!audiobookshelf_url.trim().is_empty()).then(|| {
        let mut setup = AudiobookshelfSetup::new(audiobookshelf_url);
        setup.revision = audiobookshelf
            .and_then(|s| s.get("revision"))
            .and_then(|v| v.as_integer())
            .and_then(|v| u64::try_from(v).ok())
            .filter(|revision| *revision > 0)
            .unwrap_or(setup.revision);
        setup
    });

    Ok(Config {
        emby_setup,
        audiobookshelf_setup,
        server_url,
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
        audio_pipe_playout_delay_ms,
        audio_device,
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
        idle_feed_rss_url,
        idle_feed_rotation_secs,
        shared_data_enabled,
        shared_data_listen,
        shared_data_tls_cert_path,
        shared_data_tls_key_path,
        shared_data_endpoint,
        feeds,
    })
}

/// Parse the `[[feeds]]` array-of-tables tolerantly: a row with an
/// empty/absent `url` is skipped, an empty `name` falls back to the
/// URL's host, and missing or unknown `kind` values default to Video.
/// One malformed row never fails the whole config load.
pub fn parse_feeds(value: Option<&toml::Value>) -> Vec<FeedSubscription> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let t = item.as_table()?;
            let url = t
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if url.is_empty() {
                return None;
            }
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let kind = t
                .get("kind")
                .and_then(|v| v.as_str())
                .and_then(FeedKind::parse)
                .unwrap_or_default();
            Some(FeedSubscription {
                name: if name.is_empty() {
                    default_feed_name(&url)
                } else {
                    name
                },
                url,
                kind,
            })
        })
        .collect()
}

/// Derive a display name from a feed URL's host when the row has none.
fn default_feed_name(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(url)
        .to_string()
}
