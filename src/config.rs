use std::path::PathBuf;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub api_key: String,
    pub hidden_libraries: Vec<String>,
    pub show_audio_window: bool,
    pub use_mpv_config: bool,
    pub always_play_next: bool,
    pub card_image_protocol: Option<String>, // "halfblocks" | "sixel" | "kitty" | "iterm2"
    pub show_systray_icon: bool,
    pub show_log_tab: bool,
    pub no_scripts: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            api_key: String::new(),
            hidden_libraries: vec!["live tv".into(), "podcasts".into()],
            show_audio_window: false,
            use_mpv_config: false,
            always_play_next: false,
            card_image_protocol: None,
            show_systray_icon: true,
            show_log_tab: false,
            no_scripts: false,
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
    base.join("mby")
}

pub fn osc_script_path() -> PathBuf {
    let user = data_dir().join("scripts").join("mby.lua");
    if user.exists() {
        return user;
    }
    PathBuf::from("/usr/share/mby/scripts/mby.lua")
}

pub fn prefs_path() -> PathBuf {
    config_dir().join("prefs.json")
}

fn data_dir() -> PathBuf {
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("mby")
}

pub fn osc_fonts_dir() -> PathBuf {
    data_dir().join("fonts")
}

pub fn mpv_ipc_path() -> String {
    let runtime = env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp"));
    format!("{}/mby-mpv.sock", runtime)
}

pub fn playlist_cache_path() -> PathBuf {
    config_dir().join("playlist.json")
}

pub fn token_cache_path() -> PathBuf {
    config_dir().join("token.json")
}

pub fn config_path() -> PathBuf {
    let dir = config_dir();
    // Try the new "mby" path first, fall back to the legacy "emby-browser" path.
    let new_path = dir.join("config.toml");
    if new_path.exists() {
        return new_path;
    }
    let legacy = dir.parent().unwrap_or(&dir).join("emby-browser").join("config.toml");
    if legacy.exists() {
        return legacy;
    }
    new_path
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

    let emby = match doc.get("emby") {
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
    let mby = doc.get("mby");

    let hidden_libraries: Vec<String> = emby
        .get("hidden_libraries")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_lowercase()).collect())
        .unwrap_or_else(|| vec!["live tv".into(), "podcasts".into()]);

    let show_audio_window = misc
        .and_then(|m| m.get("show_audio_window"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let use_mpv_config = misc
        .and_then(|m| m.get("use_mpv_config"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_play_next = misc
        .and_then(|m| m.get("always_play_next"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let card_image_protocol = misc
        .and_then(|m| m.get("card_image_protocol"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let show_systray_icon = daemon
        .and_then(|d| d.get("show_systray_icon"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let show_log_tab = mby
        .and_then(|m| m.get("show_log_tab"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let no_scripts = misc
        .and_then(|m| m.get("no_scripts"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(Config {
        server_url: get_str(emby, "url").trim_end_matches('/').to_string(),
        username: String::new(),
        password: String::new(),
        api_key: String::new(),
        hidden_libraries,
        show_audio_window,
        use_mpv_config,
        always_play_next,
        card_image_protocol,
        show_systray_icon,
        show_log_tab,
        no_scripts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
[emby]
url = "http://localhost:8096/"
hidden_libraries = ["Live TV", "Podcasts", "Music"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.server_url, "http://localhost:8096"); // trailing slash stripped
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts", "music"]);
    }

    #[test]
    fn parse_missing_emby_section_returns_default() {
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
[emby]
url = "http://host"
hidden_libraries = ["Live TV", "MOVIES"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "movies"]);
    }

    #[test]
    fn parse_default_hidden_libraries_when_absent() {
        let toml = "[emby]\nurl = \"http://host\"";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts"]);
    }

    #[test]
    fn parse_invalid_toml_errors() {
        assert!(parse_config("not [ valid toml !!!").is_err());
    }

    #[test]
    fn parse_use_mpv_config_true() {
        let toml = "[emby]\nurl = \"http://host\"\n[mpv]\nuse_mpv_config = true";
        assert!(parse_config(toml).unwrap().use_mpv_config);
    }

    #[test]
    fn parse_use_mpv_config_defaults_false() {
        let toml = "[emby]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().use_mpv_config);
    }

    #[test]
    fn parse_show_audio_window_true() {
        let toml = "[emby]\nurl = \"http://host\"\n[mpv]\nshow_audio_window = true";
        assert!(parse_config(toml).unwrap().show_audio_window);
    }

    #[test]
    fn parse_show_audio_window_defaults_false() {
        let toml = "[emby]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().show_audio_window);
    }

    #[test]
    fn token_cache_path_uses_xdg() {
        // XDG_CONFIG_HOME takes precedence over HOME
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test");
        let path = token_cache_path();
        std::env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(path.to_str().unwrap(), "/tmp/xdg-test/mby/token.json");
    }

    #[test]
    fn parse_show_log_tab_true() {
        let toml = "[emby]\nurl = \"http://host\"\n[mby]\nshow_log_tab = true";
        assert!(parse_config(toml).unwrap().show_log_tab);
    }

    #[test]
    fn parse_show_log_tab_defaults_false() {
        let toml = "[emby]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().show_log_tab);
    }

    #[test]
    fn parse_no_scripts_true() {
        let toml = "[emby]\nurl = \"http://host\"\n[mpv]\nno_scripts = true";
        assert!(parse_config(toml).unwrap().no_scripts);
    }

    #[test]
    fn parse_no_scripts_defaults_false() {
        let toml = "[emby]\nurl = \"http://host\"";
        assert!(!parse_config(toml).unwrap().no_scripts);
    }
}
