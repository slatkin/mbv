use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub api_key: String,
    pub hidden_libraries: Vec<String>,
    pub hidden_latest: Vec<String>,
    pub show_audio_window: bool,
    pub use_mpv_config: bool,
    pub audio_pipe_enabled: bool,
    pub audio_pipe_path: String,
    pub audio_pipe_samplerate: u32, // fixed output rate forced on the pipe (Hz); mpv resamples everything to this
    pub audio_pipe_bitdepth: u8,    // fixed PCM bit depth for the pipe (16|24|32)
    pub always_play_next: bool,
    pub consume_videos: bool,
    pub always_skip_intro: bool,
    pub image_protocol: Option<String>, // "auto" | "halfblocks" | "sixel" | "kitty" | "iterm2"
    pub show_systray_icon: bool,
    pub show_log_tab: bool,
    pub no_scripts: bool,
    pub start_on_queue: bool,
    pub daemon_mode_on_exit: bool,
    pub autoload: bool,
    pub music_levels: Vec<String>,
    pub system_notifications: bool,
    pub image_cache_size: usize,
    pub save_playlist_on_consume: bool,
    pub use_nerd_fonts: bool,
    pub indicator_style: String, // status-indicator treatment: chips|brackets|outlined|dots|pipes|keyvalue|powerline
    // [playback] — client-only subtitle/audio preferences (never pushed to Emby server)
    pub subtitle_mode: String, // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"; "" = inherit from Emby
    pub subtitle_lang: String, // full language name, e.g. "English"; "" = any
    pub audio_lang: String,    // full language name, e.g. "English"; "" = any
    pub my_languages: Vec<String>, // user's relevant languages; filters subtitle/audio lang cycling
    pub feed_view_libraries: Vec<String>, // libraries treated as feed view (unplayed, date-sorted)
    pub config_version: u32,   // schema version for future migrations (0 = unversioned)
    pub progress_interval_secs: u64, // how often to report playback progress to Emby (seconds)
    pub daemon_broadcast_ms: u64, // how often the daemon broadcasts status to connected TUIs (ms)
    pub daemon_client_endpoint: String, // [daemon.client] endpoint; empty = auto-detect local daemon
    pub daemon_server_tcp_listen: String, // [daemon.server] tcp_listen; empty = unix-only unless system instance default applies
}

pub const DEFAULT_SYSTEM_DAEMON_TCP_LISTEN: &str = "0.0.0.0:47788";

impl Default for Config {
    fn default() -> Self {
        Config {
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            api_key: String::new(),
            hidden_libraries: vec!["live tv".into(), "podcasts".into()],
            hidden_latest: vec![],
            show_audio_window: false,
            use_mpv_config: false,
            audio_pipe_enabled: false,
            audio_pipe_path: "/tmp/mbv-pipe".to_string(),
            audio_pipe_samplerate: 192_000,
            audio_pipe_bitdepth: 32,
            always_play_next: false,
            consume_videos: false,
            always_skip_intro: false,
            image_protocol: None,
            show_systray_icon: true,
            show_log_tab: false,
            no_scripts: false,
            start_on_queue: false,
            daemon_mode_on_exit: false,
            autoload: false,
            music_levels: vec![],
            system_notifications: false,
            image_cache_size: 50,
            save_playlist_on_consume: false,
            use_nerd_fonts: false,
            indicator_style: "keyvalue".into(),
            subtitle_mode: String::new(),
            subtitle_lang: String::new(),
            audio_lang: String::new(),
            my_languages: vec![],
            feed_view_libraries: vec![],
            config_version: 0,
            progress_interval_secs: 10,
            daemon_broadcast_ms: 500,
            daemon_client_endpoint: String::new(),
            daemon_server_tcp_listen: String::new(),
        }
    }
}

impl Config {
    /// The mpv audio-pipe FIFO path to write to, or `None` when the feature
    /// is disabled. Centralizes the enabled/path pair so callers never need
    /// to re-derive this themselves.
    pub fn audio_pipe_target(&self) -> Option<String> {
        if self.audio_pipe_enabled {
            Some(self.audio_pipe_path.clone())
        } else {
            None
        }
    }
}

pub fn is_system_instance() -> bool {
    env::var("MBV_SYSTEM").ok().as_deref() == Some("1")
}

pub fn default_daemon_server_tcp_listen() -> String {
    if is_system_instance() {
        DEFAULT_SYSTEM_DAEMON_TCP_LISTEN.to_string()
    } else {
        String::new()
    }
}

fn config_dir() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/etc/mbv");
    }
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("mbv")
}

pub fn cache_dir() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/cache/mbv");
    }
    let base = env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".cache")
        });
    base.join("mbv")
}

fn state_dir() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("state")
        });
    base.join("mbv")
}

pub fn data_dir_system_or_local() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("mbv")
}

fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueueSource {
    Playlist {
        id: Option<String>,
        name: String,
    },
    Album,
    Series,
    Shuffle,
    Remote,
    Collection {
        collection_type: String,
    },
    #[default]
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct QueueState {
    #[serde(default)]
    pub source: QueueSource,
    pub item_ids: Vec<String>,
    pub cursor: usize,
    pub last_played_item_id: Option<String>,
    pub last_played_completed: bool,
    // Per-item resume positions saved at quit time. Used on restore to override stale Emby
    // UserData when a fresh launch races with Emby's async position write.
    #[serde(default)]
    pub positions: std::collections::HashMap<String, i64>,
}

pub fn save_queue_state(state: &QueueState) {
    let path = queue_state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(state) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_queue_state() -> Option<QueueState> {
    let text = std::fs::read_to_string(queue_state_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn clear_queue_state() {
    let _ = std::fs::remove_file(queue_state_path());
}

/// Visibility/size of the now-playing panel, cycled with `h` and remembered across restarts.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PanelMode {
    #[default]
    #[serde(alias = "expanded")] // old variant name; map to OneRow on load
    OneRow,
    Hidden,
}

impl PanelMode {
    /// `h` toggles between the one-row panel and hidden.
    pub fn next(self) -> Self {
        match self {
            PanelMode::OneRow => PanelMode::Hidden,
            PanelMode::Hidden => PanelMode::OneRow,
        }
    }
}

pub fn image_disk_cache_dir() -> PathBuf {
    cache_dir().join("images")
}

pub fn read_image_disk_cache(key: &str) -> Option<Vec<u8>> {
    let path = image_disk_cache_dir().join(safe_cache_filename(key));
    std::fs::read(path).ok()
}

pub fn write_image_disk_cache(key: &str, bytes: &[u8]) {
    let dir = image_disk_cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(safe_cache_filename(key)), bytes);
}

pub fn evict_old_image_cache() {
    std::thread::spawn(|| {
        let dir = image_disk_cache_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    });
}

fn safe_cache_filename(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn migrate_to_state(filename: &str) -> PathBuf {
    let dest = state_dir().join(filename);
    if dest.exists() {
        return dest;
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = cache_dir().join(filename);
    if cache.exists() {
        let _ = std::fs::rename(&cache, &dest);
        return dest;
    }
    let old = config_dir().join(filename);
    if old.exists() {
        let _ = std::fs::rename(&old, &dest);
    }
    dest
}

pub fn osc_script_path() -> PathBuf {
    let user = data_dir_system_or_local().join("scripts").join("mbv.lua");
    if user.exists() {
        return user;
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/mbv.lua"));
    if dev.exists() {
        return dev;
    }
    PathBuf::from("/usr/share/mbv/scripts/mbv.lua")
}

pub fn prefs_path() -> PathBuf {
    migrate_to_state("prefs.json")
}

pub fn osc_fonts_dir() -> PathBuf {
    let user = data_dir_system_or_local().join("fonts");
    if user.exists() {
        return user;
    }
    PathBuf::from("/usr/share/mbv/fonts")
}

fn runtime_dir() -> String {
    if is_system_instance() {
        return "/run/mbv".to_string();
    }
    env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
}

pub fn mpv_ipc_path() -> String {
    format!("{}/mbv-mpv.sock", runtime_dir())
}

pub fn control_socket_path() -> String {
    format!("{}/mbv-ctrl.sock", runtime_dir())
}

pub fn token_cache_path() -> PathBuf {
    migrate_to_state("token.json")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

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
    let daemon = doc.get("daemon");
    // [general] is the new name; fall back to legacy [mbv] for existing configs.
    let general = doc.get("general").or_else(|| doc.get("mbv"));
    // [queue] is the new name; fall back to legacy [mbv.queue] then [mbv] for existing configs.
    let queue = doc
        .get("queue")
        .or_else(|| general.and_then(|m| m.get("queue")));
    let music = doc.get("music");

    let hidden_libraries: Vec<String> = general
        .and_then(|m| m.get("hidden_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_else(|| vec!["live tv".into(), "podcasts".into()]);

    let hidden_latest: Vec<String> = general
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

    let always_play_next = queue
        .and_then(|q| q.get("always_play_next"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            general
                .and_then(|m| m.get("always_play_next"))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false);

    let consume_videos = queue
        .and_then(|q| q.get("consume_videos"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let start_on_queue = queue
        .and_then(|q| q.get("start_on_queue"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            general
                .and_then(|m| m.get("start_on_queue"))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false);

    let always_skip_intro = general
        .and_then(|m| m.get("always_skip_intro"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let image_protocol = general
        .and_then(|m| {
            m.get("image_protocol")
                .or_else(|| m.get("card_image_protocol"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let show_systray_icon = daemon
        .and_then(|d| d.get("show_systray_icon"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let show_log_tab = general
        .and_then(|m| m.get("show_log_tab"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let daemon_mode_on_exit = general
        .and_then(|m| m.get("daemon_mode_on_exit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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

    let system_notifications = general
        .and_then(|m| m.get("system_notifications"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let image_cache_size = general
        .and_then(|m| m.get("image_cache_size"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as usize)
        .unwrap_or(50);

    let save_playlist_on_consume = queue
        .and_then(|q| q.get("save_playlist_on_consume"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let use_nerd_fonts = general
        .and_then(|m| m.get("use_nerd_fonts"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let indicator_style = general
        .and_then(|m| m.get("indicator_style"))
        .and_then(|v| v.as_str())
        .unwrap_or("keyvalue")
        .to_string();

    let playback = doc.get("playback");
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
    let config_version = doc.get("version").and_then(|v| v.as_integer()).unwrap_or(0) as u32;

    let progress_interval_secs = general
        .and_then(|m| m.get("progress_interval_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(10);

    let daemon_broadcast_ms = daemon
        .and_then(|d| d.get("broadcast_ms"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(100) as u64)
        .unwrap_or(500);

    let daemon_client_endpoint = daemon
        .and_then(|d| d.get("client"))
        .and_then(|c| c.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let daemon_server_tcp_listen = daemon
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("tcp_listen"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_daemon_server_tcp_listen);

    let feed_view_libraries: Vec<String> = general
        .and_then(|m| m.get("feed_view_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
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
        always_play_next,
        consume_videos,
        always_skip_intro,
        image_protocol,
        show_systray_icon,
        show_log_tab,
        no_scripts,
        start_on_queue,
        daemon_mode_on_exit,
        autoload,
        music_levels,
        system_notifications,
        image_cache_size,
        save_playlist_on_consume,
        use_nerd_fonts,
        indicator_style,
        subtitle_mode,
        subtitle_lang,
        audio_lang,
        my_languages,
        feed_view_libraries,
        config_version,
        progress_interval_secs,
        daemon_broadcast_ms,
        daemon_client_endpoint,
        daemon_server_tcp_listen,
    })
}

pub fn save_config_settings(cfg: &Config) {
    let path = config_path();
    let mut doc: toml::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return,
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

    // Migrate legacy [mbv] keys to new section names.
    if let Some(old) = table.get_mut("mbv").and_then(|v| v.as_table_mut()) {
        for key in &[
            "daemon_mode_on_exit",
            "always_skip_intro",
            "show_log_tab",
            "hidden_libraries",
            "hidden_latest",
            "image_protocol",
            "card_image_protocol",
            "always_play_next",
            "start_on_queue",
            "queue",
        ] {
            old.remove(*key);
        }
    }

    if !cfg.server_url.is_empty() {
        let server = section!("server");
        server.insert(
            "url".to_string(),
            toml::Value::String(cfg.server_url.clone()),
        );
    }

    let general = section!("general");
    general.insert(
        "daemon_mode_on_exit".to_string(),
        toml::Value::Boolean(cfg.daemon_mode_on_exit),
    );
    general.insert(
        "always_skip_intro".to_string(),
        toml::Value::Boolean(cfg.always_skip_intro),
    );
    general.insert(
        "show_log_tab".to_string(),
        toml::Value::Boolean(cfg.show_log_tab),
    );
    general.insert(
        "system_notifications".to_string(),
        toml::Value::Boolean(cfg.system_notifications),
    );
    general.insert(
        "hidden_libraries".to_string(),
        toml::Value::Array(
            cfg.hidden_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    general.insert(
        "hidden_latest".to_string(),
        toml::Value::Array(
            cfg.hidden_latest
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    general.insert(
        "feed_view_libraries".to_string(),
        toml::Value::Array(
            cfg.feed_view_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    match &cfg.image_protocol {
        Some(p) => {
            general.insert("image_protocol".to_string(), toml::Value::String(p.clone()));
        }
        None => {
            general.remove("image_protocol");
        }
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
        "start_on_queue".to_string(),
        toml::Value::Boolean(cfg.start_on_queue),
    );
    queue.insert(
        "save_playlist_on_consume".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_consume),
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
    mpv.insert(
        "audio_pipe_enabled".to_string(),
        toml::Value::Boolean(cfg.audio_pipe_enabled),
    );
    mpv.insert(
        "audio_pipe_path".to_string(),
        toml::Value::String(cfg.audio_pipe_path.clone()),
    );
    mpv.insert(
        "audio_pipe_samplerate".to_string(),
        toml::Value::Integer(cfg.audio_pipe_samplerate as i64),
    );
    mpv.insert(
        "audio_pipe_bitdepth".to_string(),
        toml::Value::Integer(cfg.audio_pipe_bitdepth as i64),
    );

    let daemon = section!("daemon");
    daemon.insert(
        "show_systray_icon".to_string(),
        toml::Value::Boolean(cfg.show_systray_icon),
    );
    daemon.insert(
        "broadcast_ms".to_string(),
        toml::Value::Integer(cfg.daemon_broadcast_ms as i64),
    );
    let daemon_client = daemon
        .entry("client".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_client_endpoint.trim().is_empty() {
        daemon_client.remove("endpoint");
    } else {
        daemon_client.insert(
            "endpoint".to_string(),
            toml::Value::String(cfg.daemon_client_endpoint.clone()),
        );
    }
    let daemon_server = daemon
        .entry("server".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_server_tcp_listen.trim().is_empty() {
        daemon_server.remove("tcp_listen");
    } else {
        daemon_server.insert(
            "tcp_listen".to_string(),
            toml::Value::String(cfg.daemon_server_tcp_listen.clone()),
        );
    }

    let playback = section!("playback");
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
    if let Ok(s) = toml::to_string(&doc) {
        let tmp = path.with_extension("toml.tmp");
        if std::fs::write(&tmp, &s).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
[server]
url = "http://localhost:8096/"
[general]
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
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts"]);
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
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.audio_pipe_enabled);
        assert_eq!(cfg.audio_pipe_path, "/tmp/custom-pipe");
        assert_eq!(cfg.audio_pipe_samplerate, 96000);
        assert_eq!(cfg.audio_pipe_bitdepth, 16);
    }

    #[test]
    fn parse_audio_pipe_defaults() {
        let cfg = parse_config("").unwrap();
        assert!(!cfg.audio_pipe_enabled);
        assert_eq!(cfg.audio_pipe_path, "/tmp/mbv-pipe");
        assert_eq!(cfg.audio_pipe_samplerate, 192_000);
        assert_eq!(cfg.audio_pipe_bitdepth, 32);
    }

    #[test]
    fn parse_daemon_client_endpoint() {
        let toml = r#"
[server]
url = "http://localhost:8096"
[daemon.client]
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
[daemon.server]
tcp_listen = "0.0.0.0:8890"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.daemon_server_tcp_listen, "0.0.0.0:8890");
    }

    #[test]
    fn parse_hidden_libraries_lowercased() {
        let toml = r#"
[server]
url = "http://host"
[general]
hidden_libraries = ["Live TV", "MOVIES"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "movies"]);
    }

    #[test]
    fn parse_default_hidden_libraries_when_absent() {
        let toml = "[server]\nurl = \"http://host\"";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts"]);
    }

    #[test]
    fn parse_hidden_latest_lowercased() {
        let toml = r#"
[server]
url = "http://host"
[general]
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
    fn parse_invalid_toml_errors() {
        assert!(parse_config("not [ valid toml !!!").is_err());
    }

    #[test]
    fn parse_music_levels_group_album() {
        let toml = "[server]\nurl = \"http://host\"\n[music]\nlevels = [\"group\", \"album\"]";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.music_levels, vec!["group", "album"]);
    }

    #[test]
    fn parse_music_levels_album_only() {
        let toml = "[server]\nurl = \"http://host\"\n[music]\nlevels = [\"album\"]";
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
    fn parse_always_play_next_legacy_mbv_section_still_works() {
        let toml = "[server]\nurl = \"http://host\"\n[mbv]\nalways_play_next = true";
        assert!(
            parse_config(toml).unwrap().always_play_next,
            "backward compat: [mbv] fallback"
        );
    }

    #[test]
    fn parse_start_on_queue_legacy_mbv_section_still_works() {
        let toml = "[server]\nurl = \"http://host\"\n[mbv]\nstart_on_queue = true";
        assert!(
            parse_config(toml).unwrap().start_on_queue,
            "backward compat: [mbv] fallback"
        );
    }

    // ── System-instance path routing ─────────────────────────────────────────
    //
    // MBV_SYSTEM is a process-global env var, so these tests must not run in
    // parallel with each other or with the XDG tests above. Use a Mutex.

    use std::sync::Mutex;
    static SYS_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn is_system_instance_false_without_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!is_system_instance());
    }

    #[test]
    fn is_system_instance_true_with_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(result);
    }

    #[test]
    fn is_system_instance_false_with_env_set_to_zero() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "0");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!result);
    }

    #[test]
    fn is_system_instance_false_with_empty_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!result);
    }

    #[test]
    fn cache_dir_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = cache_dir();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/var/cache/mbv"));
    }

    #[test]
    fn cache_dir_uses_xdg_when_not_system() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        std::env::set_var("XDG_CACHE_HOME", "/tmp/xdg-test-cache");
        let path = cache_dir();
        std::env::remove_var("XDG_CACHE_HOME");
        assert_eq!(path, std::path::PathBuf::from("/tmp/xdg-test-cache/mbv"));
    }

    #[test]
    fn data_dir_system_or_local_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = data_dir_system_or_local();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/var/lib/mbv"));
    }

    #[test]
    fn config_path_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = config_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/etc/mbv/config.toml"));
    }

    #[test]
    fn mpv_ipc_path_uses_run_dir_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = mpv_ipc_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, "/run/mbv/mbv-mpv.sock");
    }

    #[test]
    fn control_socket_path_uses_run_dir_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = control_socket_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, "/run/mbv/mbv-ctrl.sock");
    }

    #[test]
    fn daemon_server_tcp_listen_defaults_for_system_instance() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(
            cfg.daemon_server_tcp_listen,
            DEFAULT_SYSTEM_DAEMON_TCP_LISTEN
        );
    }

    #[test]
    fn mpv_ipc_path_uses_xdg_runtime_dir_when_not_system() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let path = mpv_ipc_path();
        std::env::remove_var("XDG_RUNTIME_DIR");
        assert_eq!(path, "/run/user/1000/mbv-mpv.sock");
    }
}
