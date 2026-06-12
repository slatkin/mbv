use std::path::PathBuf;
use std::env;

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
}

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
        }
    }
}

fn config_dir() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("mbv")
}

pub fn osc_script_path() -> PathBuf {
    let user = data_dir().join("scripts").join("mbv.lua");
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
    config_dir().join("prefs.json")
}

pub fn load_subs_off() -> bool {
    let path = prefs_path();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["subs_off"].as_bool())
        .unwrap_or(true)
}

fn data_dir() -> PathBuf {
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("mbv")
}

pub fn osc_fonts_dir() -> PathBuf {
    let user = data_dir().join("fonts");
    if user.exists() {
        return user;
    }
    PathBuf::from("/usr/share/mbv/fonts")
}

pub fn mpv_ipc_path() -> String {
    let runtime = env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/mbv-mpv.sock", runtime)
}

pub fn control_socket_path() -> String {
    let runtime = env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/mbv-ctrl.sock", runtime)
}

pub fn playlist_cache_path() -> PathBuf {
    config_dir().join("playlist.json")
}

pub fn token_cache_path() -> PathBuf {
    config_dir().join("token.json")
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
        section.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let misc = doc.get("mpv");
    let daemon = doc.get("daemon");
    // [general] is the new name; fall back to legacy [mbv] for existing configs.
    let general = doc.get("general").or_else(|| doc.get("mbv"));
    // [queue] is the new name; fall back to legacy [mbv.queue] then [mbv] for existing configs.
    let queue = doc.get("queue")
        .or_else(|| general.and_then(|m| m.get("queue")));
    let music = doc.get("music");

    let hidden_libraries: Vec<String> = general
        .and_then(|m| m.get("hidden_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_lowercase()).collect())
        .unwrap_or_else(|| vec!["live tv".into(), "podcasts".into()]);

    let hidden_latest: Vec<String> = general
        .and_then(|m| m.get("hidden_latest"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_lowercase()).collect())
        .unwrap_or_default();

    let show_audio_window = misc
        .and_then(|m| m.get("show_audio_window"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let use_mpv_config = misc
        .and_then(|m| m.get("use_mpv_config"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_play_next = queue
        .and_then(|q| q.get("always_play_next"))
        .and_then(|v| v.as_bool())
        .or_else(|| general.and_then(|m| m.get("always_play_next")).and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let consume_videos = queue
        .and_then(|q| q.get("consume_videos"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let start_on_queue = queue
        .and_then(|q| q.get("start_on_queue"))
        .and_then(|v| v.as_bool())
        .or_else(|| general.and_then(|m| m.get("start_on_queue")).and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let always_skip_intro = general
        .and_then(|m| m.get("always_skip_intro"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let image_protocol = general
        .and_then(|m| m.get("image_protocol").or_else(|| m.get("card_image_protocol")))
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
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect())
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
    })
}

pub fn save_config_settings(cfg: &Config) {
    let path = config_path();
    let mut doc: toml::Value = std::fs::read_to_string(&path).ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return,
    };

    macro_rules! section {
        ($name:literal) => {
            table.entry($name.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
                .as_table_mut().unwrap()
        };
    }

    // Migrate legacy [mbv] keys to new section names.
    if let Some(old) = table.get_mut("mbv").and_then(|v| v.as_table_mut()) {
        for key in &["daemon_mode_on_exit", "always_skip_intro", "show_log_tab",
                     "hidden_libraries", "hidden_latest", "image_protocol",
                     "card_image_protocol", "always_play_next", "start_on_queue", "queue"] {
            old.remove(*key);
        }
    }

    let general = section!("general");
    general.insert("daemon_mode_on_exit".to_string(), toml::Value::Boolean(cfg.daemon_mode_on_exit));
    general.insert("always_skip_intro".to_string(),   toml::Value::Boolean(cfg.always_skip_intro));
    general.insert("show_log_tab".to_string(),        toml::Value::Boolean(cfg.show_log_tab));
    general.insert("hidden_libraries".to_string(), toml::Value::Array(
        cfg.hidden_libraries.iter().map(|s| toml::Value::String(s.clone())).collect()
    ));
    general.insert("hidden_latest".to_string(), toml::Value::Array(
        cfg.hidden_latest.iter().map(|s| toml::Value::String(s.clone())).collect()
    ));
    match &cfg.image_protocol {
        Some(p) => { general.insert("image_protocol".to_string(), toml::Value::String(p.clone())); }
        None    => { general.remove("image_protocol"); }
    }

    let queue = section!("queue");
    queue.insert("always_play_next".to_string(),      toml::Value::Boolean(cfg.always_play_next));
    queue.insert("consume_videos".to_string(), toml::Value::Boolean(cfg.consume_videos));
    queue.insert("start_on_queue".to_string(),         toml::Value::Boolean(cfg.start_on_queue));

    let mpv = section!("mpv");
    mpv.insert("show_audio_window".to_string(), toml::Value::Boolean(cfg.show_audio_window));
    mpv.insert("use_mpv_config".to_string(),    toml::Value::Boolean(cfg.use_mpv_config));
    mpv.insert("no_scripts".to_string(),        toml::Value::Boolean(cfg.no_scripts));
    mpv.insert("autoload".to_string(),          toml::Value::Boolean(cfg.autoload));

    let daemon = section!("daemon");
    daemon.insert("show_systray_icon".to_string(), toml::Value::Boolean(cfg.show_systray_icon));

    if let Ok(s) = toml::to_string(&doc) {
        let _ = std::fs::write(path, s);
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
    fn parse_use_mpv_config_true() {
        let toml = "[server]\nurl = \"http://host\"\n[mpv]\nuse_mpv_config = true";
        assert!(parse_config(toml).unwrap().use_mpv_config);
    }

    #[test]
    fn parse_use_mpv_config_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().use_mpv_config);
    }

    #[test]
    fn parse_show_audio_window_true() {
        let toml = "[server]\nurl = \"http://host\"\n[mpv]\nshow_audio_window = true";
        assert!(parse_config(toml).unwrap().show_audio_window);
    }

    #[test]
    fn parse_show_audio_window_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().show_audio_window);
    }

    #[test]
    fn token_cache_path_uses_xdg() {
        // XDG_CONFIG_HOME takes precedence over HOME
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test");
        let path = token_cache_path();
        std::env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(path.to_str().unwrap(), "/tmp/xdg-test/mbv/token.json");
    }

    #[test]
    fn parse_show_log_tab_true() {
        let toml = "[server]\nurl = \"http://host\"\n[general]\nshow_log_tab = true";
        assert!(parse_config(toml).unwrap().show_log_tab);
    }

    #[test]
    fn parse_show_log_tab_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().show_log_tab);
    }

    #[test]
    fn parse_no_scripts_true() {
        let toml = "[server]\nurl = \"http://host\"\n[mpv]\nno_scripts = true";
        assert!(parse_config(toml).unwrap().no_scripts);
    }

    #[test]
    fn parse_no_scripts_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().no_scripts);
    }

    #[test]
    fn parse_autoload_true() {
        let toml = "[server]\nurl = \"http://host\"\n[mpv]\nautoload = true";
        assert!(parse_config(toml).unwrap().autoload);
    }

    #[test]
    fn parse_autoload_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().autoload);
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
    fn parse_always_play_next_true_from_queue_section() {
        let toml = "[server]\nurl = \"http://host\"\n[queue]\nalways_play_next = true";
        assert!(parse_config(toml).unwrap().always_play_next);
    }

    #[test]
    fn parse_always_play_next_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().always_play_next);
    }

    #[test]
    fn parse_always_play_next_in_wrong_section_is_ignored() {
        let toml = "[server]\nurl = \"http://host\"\nalways_play_next = true";
        assert!(!parse_config(toml).unwrap().always_play_next, "always_play_next must be in [queue], not [server]");
    }

    #[test]
    fn parse_always_play_next_legacy_mbv_section_still_works() {
        let toml = "[server]\nurl = \"http://host\"\n[mbv]\nalways_play_next = true";
        assert!(parse_config(toml).unwrap().always_play_next, "backward compat: [mbv] fallback");
    }

    #[test]
    fn parse_start_on_queue_true_from_queue_section() {
        let toml = "[server]\nurl = \"http://host\"\n[queue]\nstart_on_queue = true";
        assert!(parse_config(toml).unwrap().start_on_queue);
    }

    #[test]
    fn parse_start_on_queue_legacy_mbv_section_still_works() {
        let toml = "[server]\nurl = \"http://host\"\n[mbv]\nstart_on_queue = true";
        assert!(parse_config(toml).unwrap().start_on_queue, "backward compat: [mbv] fallback");
    }

    #[test]
    fn parse_always_skip_intro_true_from_general_section() {
        let toml = "[server]\nurl = \"http://host\"\n[general]\nalways_skip_intro = true";
        assert!(parse_config(toml).unwrap().always_skip_intro);
    }

    #[test]
    fn parse_always_skip_intro_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().always_skip_intro);
    }

    #[test]
    fn parse_daemon_mode_on_exit_true() {
        let toml = "[server]\nurl = \"http://host\"\n[general]\ndaemon_mode_on_exit = true";
        assert!(parse_config(toml).unwrap().daemon_mode_on_exit);
    }

    #[test]
    fn parse_daemon_mode_on_exit_defaults_false() {
        let toml = "[emby]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().daemon_mode_on_exit);
    }

    #[test]
    fn parse_consume_videos_true_from_queue_section() {
        let toml = "[server]\nurl = \"http://host\"\n[queue]\nconsume_videos = true";
        assert!(parse_config(toml).unwrap().consume_videos);
    }

    #[test]
    fn parse_consume_videos_defaults_false() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().consume_videos);
    }
}
