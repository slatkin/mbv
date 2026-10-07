use super::{
    AudiobookshelfSetup, Config, ConfigError, DEFAULT_VIDEO_CACHE_BACK_MB,
    DEFAULT_VIDEO_CACHE_FORWARD_MB, EmbySetup, FeedKind, FeedSubscription, PANEL_COLS_MIN,
    PanelAccentColor, PanelConfig, PanelSide, config_path, default_daemon_server_tcp_listen,
    is_valid_audio_device,
};
use std::num::NonZeroU16;

pub fn load_config() -> Result<Config, ConfigError> {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Config::default());
    };
    parse_config(&text)
        .map_err(|e| ConfigError::parse(format!("Config parse error in {}: {e}", path.display())))
}

pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let doc: toml::Value = toml::from_str(text)?;

    // All non-Emby sections are parsed unconditionally, even when
    // [server] is absent (feed-only / service-independent startup).
    let feeds = parse_feeds(doc.get("feeds"));
    let mpv = parse_mpv_section(doc.get("mpv"))?;
    let queue = parse_queue_section(doc.get("queue"));
    let session = parse_session_section(doc.get("session"));
    let playback = parse_playback_section(doc.get("playback"));
    let display = parse_display_section(doc.get("display"));
    let mbvd = parse_mbvd_section(doc.get("mbvd"));
    let library = parse_library_section(doc.get("library"));
    let idle_feed = parse_idle_feed_section(doc.get("idle_feed"));
    let panel = parse_panel_section(doc.get("panel"));
    let (server_url, emby_setup) = parse_server_section(doc.get("server"));
    let audiobookshelf_setup = parse_audiobookshelf_section(doc.get("audiobookshelf"));
    let library_routes = parse_library_routes_section(doc.get("library_routes"));

    // `[keys]` (change `add-configurable-keybinds`): parsed into the raw
    // section-outer shape, then compiled through the keybind registry so
    // every entry — unknown sections and actions, section mismatches,
    // reserved chords, and both collision classes — is rejected here, at
    // the existing config error path, before the process starts.
    let keybinds = match doc.get("keys") {
        None => mbv_keybinds::Keybinds::default(),
        Some(keys) => {
            let raw = parse_raw_keybinds(keys)?;
            mbv_keybinds::load(&raw)?
        }
    };

    Ok(Config {
        emby_setup,
        audiobookshelf_setup,
        server_url,
        username: String::new(),
        password: String::new(),
        api_key: String::new(),
        hidden_libraries: library.hidden_libraries,
        show_audio_window: mpv.show_audio_window,
        use_mpv_config: mpv.use_mpv_config,
        video_cache_forward_mb: mpv.video_cache_forward_mb,
        video_cache_back_mb: mpv.video_cache_back_mb,
        audio_pipe_enabled: mpv.audio_pipe_enabled,
        audio_pipe_path: mpv.audio_pipe_path,
        audio_pipe_samplerate: mpv.audio_pipe_samplerate,
        audio_pipe_bitdepth: mpv.audio_pipe_bitdepth,
        audio_pipe_playout_delay_ms: mpv.audio_pipe_playout_delay_ms,
        audio_device: mpv.audio_device,
        always_play_next: queue.always_play_next,
        consume_videos: queue.consume_videos,
        consume_audio: queue.consume_audio,
        always_skip_intro: session.always_skip_intro,
        show_systray_icon: playback.show_systray_icon,
        no_scripts: mpv.no_scripts,
        stay_alive: session.stay_alive,
        save_playlist_on_quit: session.save_playlist_on_quit,
        autoload: mpv.autoload,
        music_levels: library.music_levels,
        system_notifications: display.system_notifications,
        mouse_support: display.mouse_support,
        save_playlist_on_consume: queue.save_playlist_on_consume,
        save_playlist_on_consume_audio: queue.save_playlist_on_consume_audio,
        subtitle_mode: playback.subtitle_mode,
        subtitle_lang: playback.subtitle_lang,
        audio_lang: playback.audio_lang,
        my_languages: playback.my_languages,
        feed_view_libraries: library.feed_view_libraries,
        library_routes,
        progress_interval_secs: session.progress_interval_secs,
        quit_timeout_secs: session.quit_timeout_secs,
        daemon_broadcast_ms: mbvd.broadcast_ms,
        daemon_client_endpoint: mbvd.client_endpoint,
        daemon_server_tcp_listen: mbvd.server_tcp_listen,
        auto_reconnect: session.auto_reconnect,
        idle_feed_rss_url: idle_feed.rss_url,
        idle_feed_rotation_secs: idle_feed.rotation_secs,
        feeds,
        keybinds,
        panel,
    })
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "mpv settings are independent configuration options (design analysis, issue #804)"
)]
struct MpvSettings {
    show_audio_window: bool,
    use_mpv_config: bool,
    video_cache_forward_mb: u32,
    video_cache_back_mb: u32,
    audio_pipe_enabled: bool,
    audio_pipe_path: String,
    audio_pipe_samplerate: u32,
    audio_pipe_bitdepth: u8,
    audio_pipe_playout_delay_ms: Option<u64>,
    audio_device: String,
    no_scripts: bool,
    autoload: bool,
}

fn parse_mpv_section(misc: Option<&toml::Value>) -> Result<MpvSettings, ConfigError> {
    Ok(MpvSettings {
        show_audio_window: mpv_bool(misc, "show_audio_window"),
        use_mpv_config: mpv_bool(misc, "use_mpv_config"),
        video_cache_forward_mb: mpv_cache_size(
            misc,
            "video_cache_forward_mb",
            DEFAULT_VIDEO_CACHE_FORWARD_MB,
        ),
        video_cache_back_mb: mpv_cache_size(
            misc,
            "video_cache_back_mb",
            DEFAULT_VIDEO_CACHE_BACK_MB,
        ),
        audio_pipe_enabled: mpv_bool(misc, "audio_pipe_enabled"),
        audio_pipe_path: mpv_audio_pipe_path(misc),
        audio_pipe_samplerate: mpv_audio_pipe_samplerate(misc),
        audio_pipe_bitdepth: mpv_audio_pipe_bitdepth(misc),
        audio_pipe_playout_delay_ms: mpv_audio_pipe_playout_delay(misc)?,
        audio_device: mpv_audio_device(misc)?,
        no_scripts: mpv_bool(misc, "no_scripts"),
        autoload: mpv_bool(misc, "autoload"),
    })
}

fn mpv_bool(misc: Option<&toml::Value>, key: &str) -> bool {
    misc.and_then(|m| m.get(key))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

fn mpv_cache_size(misc: Option<&toml::Value>, key: &str, default: u32) -> u32 {
    misc.and_then(|m| m.get(key))
        .and_then(toml::Value::as_integer)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn mpv_audio_pipe_path(misc: Option<&toml::Value>) -> String {
    misc.and_then(|m| m.get("audio_pipe_path"))
        .and_then(toml::Value::as_str)
        .unwrap_or("/tmp/mbv-pipe")
        .to_string()
}

fn mpv_audio_pipe_samplerate(misc: Option<&toml::Value>) -> u32 {
    misc.and_then(|m| m.get("audio_pipe_samplerate"))
        .and_then(toml::Value::as_integer)
        .map_or(192_000, |v| u32::try_from(v.max(1)).unwrap_or(u32::MAX))
}

fn mpv_audio_pipe_bitdepth(misc: Option<&toml::Value>) -> u8 {
    misc.and_then(|m| m.get("audio_pipe_bitdepth"))
        .and_then(toml::Value::as_integer)
        .map_or(32, |v| match v {
            16 | 24 | 32 => u8::try_from(v).expect("bit depth is bounded to 16, 24, or 32"),
            _ => 32,
        })
}

fn mpv_audio_pipe_playout_delay(misc: Option<&toml::Value>) -> Result<Option<u64>, ConfigError> {
    match misc
        .and_then(|m| m.get("audio_pipe_playout_delay_ms"))
        .and_then(toml::Value::as_integer)
    {
        Some(value) if value < 0 => Err(ConfigError::parse(
            "mpv.audio_pipe_playout_delay_ms must be nonnegative",
        )),
        Some(value) => Ok(Some(
            u64::try_from(value).expect("negative values were rejected"),
        )),
        None => Ok(None),
    }
}

fn mpv_audio_device(misc: Option<&toml::Value>) -> Result<String, ConfigError> {
    match misc.and_then(|m| m.get("audio_device")) {
        None => Ok("alsa".to_string()),
        Some(value) => match value.as_str() {
            Some(value) if is_valid_audio_device(value) => Ok(value.to_string()),
            _ => Err(ConfigError::parse(format!(
                "mpv.audio_device must be \"alsa\" or start with \"alsa/\", got {value:?}"
            ))),
        },
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "queue settings control independent behaviors and may be enabled together (design analysis, issue #804)"
)]
struct QueueSettings {
    always_play_next: bool,
    consume_videos: bool,
    consume_audio: bool,
    save_playlist_on_consume: bool,
    save_playlist_on_consume_audio: bool,
}

fn parse_queue_section(queue: Option<&toml::Value>) -> QueueSettings {
    let get_bool = |key: &str| {
        queue
            .and_then(|q| q.get(key))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
    };
    QueueSettings {
        always_play_next: get_bool("always_play_next"),
        consume_videos: get_bool("consume_videos"),
        consume_audio: get_bool("consume_audio"),
        save_playlist_on_consume: get_bool("save_playlist_on_consume"),
        save_playlist_on_consume_audio: get_bool("save_playlist_on_consume_audio"),
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "session settings control independent behaviors and may be enabled together (design analysis, issue #804)"
)]
struct SessionSettings {
    always_skip_intro: bool,
    stay_alive: bool,
    auto_reconnect: bool,
    save_playlist_on_quit: bool,
    progress_interval_secs: u64,
    quit_timeout_secs: u64,
}

fn parse_session_section(session: Option<&toml::Value>) -> SessionSettings {
    let get_bool = |key: &str| {
        session
            .and_then(|m| m.get(key))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
    };
    SessionSettings {
        always_skip_intro: get_bool("always_skip_intro"),
        stay_alive: get_bool("stay_alive"),
        auto_reconnect: get_bool("auto_reconnect"),
        save_playlist_on_quit: get_bool("save_playlist_on_quit"),
        progress_interval_secs: session
            .and_then(|m| m.get("progress_interval_secs"))
            .and_then(toml::Value::as_integer)
            .map_or(10, |v| u64::try_from(v.max(1)).unwrap_or(u64::MAX)),
        quit_timeout_secs: session
            .and_then(|m| m.get("quit_timeout_secs"))
            .and_then(toml::Value::as_integer)
            .map_or(5, |v| u64::try_from(v.max(1)).unwrap_or(u64::MAX)),
    }
}

struct PlaybackSettings {
    show_systray_icon: bool,
    subtitle_mode: String,
    subtitle_lang: String,
    audio_lang: String,
    my_languages: Vec<String>,
}

fn parse_playback_section(playback: Option<&toml::Value>) -> PlaybackSettings {
    let get_str = |key: &str| {
        playback
            .and_then(|p| p.get(key))
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    PlaybackSettings {
        show_systray_icon: playback
            .and_then(|d| d.get("show_systray_icon"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        subtitle_mode: get_str("subtitle_mode"),
        subtitle_lang: get_str("subtitle_lang"),
        audio_lang: get_str("audio_lang"),
        my_languages: playback
            .and_then(|p| p.get("my_languages"))
            .and_then(toml::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

struct DisplaySettings {
    system_notifications: bool,
    mouse_support: bool,
}

fn parse_display_section(display: Option<&toml::Value>) -> DisplaySettings {
    DisplaySettings {
        system_notifications: display
            .and_then(|m| m.get("system_notifications"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        mouse_support: display
            .and_then(|m| m.get("mouse_support"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true),
    }
}

struct MbvdSettings {
    broadcast_ms: u64,
    client_endpoint: String,
    server_tcp_listen: String,
}

fn parse_mbvd_section(mbvd: Option<&toml::Value>) -> MbvdSettings {
    MbvdSettings {
        broadcast_ms: mbvd
            .and_then(|d| d.get("broadcast_ms"))
            .and_then(toml::Value::as_integer)
            .map_or(500, |v| u64::try_from(v.max(100)).unwrap_or(u64::MAX)),
        client_endpoint: mbvd
            .and_then(|d| d.get("client"))
            .and_then(|c| c.get("endpoint"))
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string(),
        server_tcp_listen: mbvd
            .and_then(|d| d.get("server"))
            .and_then(|s| s.get("tcp_listen"))
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map_or_else(default_daemon_server_tcp_listen, str::to_string),
    }
}

struct LibrarySettings {
    hidden_libraries: Vec<String>,
    music_levels: Vec<String>,
    feed_view_libraries: Vec<String>,
}

/// Parse `[panel]` into validated values (design D6): every key is optional,
/// and an out-of-range or malformed value falls back to its own default with
/// a logged warning while the other keys keep their values. No geometry
/// validation happens here — that needs live monitor metrics (pinwin's job).
fn parse_panel_section(panel: Option<&toml::Value>) -> PanelConfig {
    let defaults = PanelConfig::default();
    let Some(panel) = panel else {
        return defaults;
    };
    PanelConfig {
        side: panel_side(panel, defaults.side),
        cols: panel_cols(panel, "cols", defaults.cols),
        cols_expanded: panel_cols(panel, "cols_expanded", defaults.cols_expanded),
        gutter_top: panel_gutter(panel, "gutter_top", defaults.gutter_top),
        gutter_bottom: panel_gutter(panel, "gutter_bottom", defaults.gutter_bottom),
        gutter_left: panel_gutter(panel, "gutter_left", defaults.gutter_left),
        gutter_right: panel_gutter(panel, "gutter_right", defaults.gutter_right),
        accent: panel_bool(panel, "accent", defaults.accent),
        accent_color: panel_accent_color(panel, "accent_color", defaults.accent_color),
        accent_width: panel_accent_width(panel, "accent_width", defaults.accent_width),
    }
}

fn panel_side(panel: &toml::Value, default: PanelSide) -> PanelSide {
    match panel.get("side") {
        None => default,
        Some(value) => value
            .as_str()
            .and_then(PanelSide::parse)
            .unwrap_or_else(|| panel_fallback("side", value, default)),
    }
}

fn panel_cols(panel: &toml::Value, key: &str, default: u16) -> u16 {
    match panel.get(key) {
        None => default,
        Some(value) => value
            .as_integer()
            .and_then(|v| u16::try_from(v).ok())
            .filter(|v| *v >= PANEL_COLS_MIN)
            .unwrap_or_else(|| panel_fallback(key, value, default)),
    }
}

fn panel_gutter(panel: &toml::Value, key: &str, default: i32) -> i32 {
    match panel.get(key) {
        None => default,
        Some(value) => value
            .as_integer()
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or_else(|| panel_fallback(key, value, default)),
    }
}

fn panel_bool(panel: &toml::Value, key: &str, default: bool) -> bool {
    match panel.get(key) {
        None => default,
        Some(value) => value
            .as_bool()
            .unwrap_or_else(|| panel_fallback(key, value, default)),
    }
}

fn panel_accent_color(
    panel: &toml::Value,
    key: &str,
    default: PanelAccentColor,
) -> PanelAccentColor {
    match panel.get(key) {
        None => default,
        Some(value) => value
            .as_str()
            .and_then(PanelAccentColor::parse)
            .unwrap_or_else(|| panel_fallback(key, value, default)),
    }
}

fn panel_accent_width(panel: &toml::Value, key: &str, default: NonZeroU16) -> NonZeroU16 {
    match panel.get(key) {
        None => default,
        Some(value) => value
            .as_integer()
            .and_then(|v| u16::try_from(v).ok())
            .and_then(NonZeroU16::new)
            .unwrap_or_else(|| panel_fallback(key, value, default)),
    }
}

fn panel_fallback<T>(key: &str, value: &toml::Value, default: T) -> T {
    tracing::warn!(
        name: "config.panel.invalid",
        target: "config",
        key = key,
        value = %value,
        "invalid [panel] value; using default"
    );
    default
}

fn parse_library_section(library: Option<&toml::Value>) -> LibrarySettings {
    let music = library.and_then(|l| l.get("music"));
    LibrarySettings {
        hidden_libraries: library
            .and_then(|m| m.get("hidden_libraries"))
            .and_then(toml::Value::as_array)
            .map_or_else(
                || vec!["live tv".into()],
                |arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(str::to_lowercase)
                        .collect()
                },
            ),
        music_levels: music
            .and_then(|m| m.get("levels"))
            .and_then(toml::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        feed_view_libraries: library
            .and_then(|m| m.get("feed_view_libraries"))
            .and_then(toml::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

struct IdleFeedSettings {
    rss_url: String,
    rotation_secs: u64,
}

fn parse_idle_feed_section(idle_feed: Option<&toml::Value>) -> IdleFeedSettings {
    IdleFeedSettings {
        rss_url: idle_feed
            .and_then(|s| s.get("rss_url"))
            .and_then(toml::Value::as_str)
            .unwrap_or("https://novaramedia.com/feed/")
            .to_string(),
        rotation_secs: idle_feed
            .and_then(|s| s.get("rotation_interval_secs"))
            .and_then(toml::Value::as_integer)
            .map_or(10, |v| u64::try_from(v.max(1)).unwrap_or(u64::MAX)),
    }
}

fn parse_server_section(server: Option<&toml::Value>) -> (String, Option<EmbySetup>) {
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
            .and_then(toml::Value::as_integer)
            .and_then(|v| u64::try_from(v).ok())
            .filter(|revision| *revision > 0)
            .unwrap_or(setup.revision);
        setup
    });
    (server_url, emby_setup)
}

fn parse_audiobookshelf_section(
    audiobookshelf: Option<&toml::Value>,
) -> Option<AudiobookshelfSetup> {
    let audiobookshelf_url = audiobookshelf
        .map(|section| get_str(section, "url"))
        .unwrap_or_default();
    (!audiobookshelf_url.trim().is_empty()).then(|| {
        let mut setup = AudiobookshelfSetup::new(audiobookshelf_url);
        setup.revision = audiobookshelf
            .and_then(|s| s.get("revision"))
            .and_then(toml::Value::as_integer)
            .and_then(|v| u64::try_from(v).ok())
            .filter(|revision| *revision > 0)
            .unwrap_or(setup.revision);
        setup
    })
}

fn parse_library_routes_section(
    value: Option<&toml::Value>,
) -> std::collections::BTreeMap<String, String> {
    value
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn get_str(section: &toml::Value, key: &str) -> String {
    section
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn parse_prefix_chord<'a>(
    chord: &'a toml::Value,
    name: &str,
    action_id: &str,
) -> Result<&'a str, ConfigError> {
    chord.as_str().ok_or_else(|| {
        ConfigError::parse(format!(
            "keys.{name}.prefix.{action_id} must be a string chord"
        ))
    })
}

/// Parse the `[keys]` table into the raw section-outer shape (design D3):
/// `prefix` is a string chord, each other entry a per-section table whose
/// string keys are router-scope overrides and whose `prefix` sub-table
/// holds prefix-namespace assignments. Shape errors are reported here;
/// semantic validation is the registry's (`keybinds::load`).
fn parse_raw_keybinds(keys: &toml::Value) -> Result<mbv_keybinds::RawKeybinds, ConfigError> {
    use mbv_keybinds::{RawKeybinds, RawSection};

    let table = keys
        .as_table()
        .ok_or_else(|| ConfigError::parse("keys must be a table"))?;
    let mut raw = RawKeybinds::default();
    for (name, entry) in table {
        if name == "prefix" {
            let chord = entry
                .as_str()
                .ok_or_else(|| ConfigError::parse("keys.prefix must be a string chord"))?;
            raw.prefix = Some(chord.to_string());
            continue;
        }
        let section_table = entry
            .as_table()
            .ok_or_else(|| ConfigError::parse(format!("keys.{name} must be a table")))?;
        let mut section = RawSection::default();
        for (key, value) in section_table {
            if key == "prefix" {
                let prefix_table = value.as_table().ok_or_else(|| {
                    ConfigError::parse(format!("keys.{name}.prefix must be a table"))
                })?;
                for (action_id, chord) in prefix_table {
                    let chord = parse_prefix_chord(chord, name, action_id)?;
                    section.prefix.push((action_id.clone(), chord.to_string()));
                }
                continue;
            }
            let chord = value.as_str().ok_or_else(|| {
                ConfigError::parse(format!("keys.{name}.{key} must be a string chord"))
            })?;
            section.router.push((key.clone(), chord.to_string()));
        }
        raw.sections.push((name.clone(), section));
    }
    Ok(raw)
}

/// Parse the `[[feeds]]` array-of-tables tolerantly: a row with an
/// empty/absent `url` is skipped, an empty `name` falls back to the
/// URL's host, and missing or unknown `kind` values default to Video.
/// One malformed row never fails the whole config load.
#[must_use]
pub fn parse_feeds(value: Option<&toml::Value>) -> Vec<FeedSubscription> {
    let Some(arr) = value.and_then(toml::Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let t = item.as_table()?;
            let url = t
                .get("url")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if url.is_empty() {
                return None;
            }
            let name = t
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            let kind = t
                .get("kind")
                .and_then(toml::Value::as_str)
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
