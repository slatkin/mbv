pub use mbv_core::config::{
    clear_queue_state, is_system_instance, load_library_position_state, load_queue_state,
    prefs_path, save_library_position_state, save_queue_state, Config, LibraryPosition,
    LibraryPositionLevel, LibraryPositionState, QueueSource, QueueState,
};
#[cfg(test)]
pub use mbv_core::config::{load_last_remote_connection, save_last_remote_connection};
#[cfg(test)]
pub use mbv_core::config::{LastRemoteConnection, TestStateDirGuard};

use std::path::PathBuf;

#[cfg(test)]
pub mod tests {
    pub use mbv_core::config::tests::SYS_ENV_LOCK;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiConfig {
    pub image_protocol: Option<String>, // "auto" | "halfblocks" | "sixel" | "kitty" | "iterm2"
    pub image_cache_size: usize,
    pub use_nerd_fonts: bool,
    pub indicator_style: String, // chips|brackets|outlined|dots|pipes|keyvalue|powerline
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            image_protocol: None,
            image_cache_size: 50,
            use_nerd_fonts: false,
            indicator_style: "keyvalue".into(),
        }
    }
}

pub fn load_config() -> Result<Config, String> {
    mbv_core::config::load_config()
}

pub fn load_ui_config() -> Result<UiConfig, String> {
    let path = mbv_core::config::config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(UiConfig::default()),
    };
    parse_ui_config(&text).map_err(|e| format!("Config parse error in {:?}: {e}", path))
}

fn parse_ui_config(text: &str) -> Result<UiConfig, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let display = doc.get("display");

    let image_protocol = display
        .and_then(|m| {
            m.get("image_protocol")
                .or_else(|| m.get("card_image_protocol"))
        })
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let image_cache_size = display
        .and_then(|m| m.get("image_cache_size"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as usize)
        .unwrap_or(50);
    let use_nerd_fonts = display
        .and_then(|m| m.get("use_nerd_fonts"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let indicator_style = display
        .and_then(|m| m.get("indicator_style"))
        .and_then(|v| v.as_str())
        .unwrap_or("keyvalue")
        .to_string();

    Ok(UiConfig {
        image_protocol,
        image_cache_size,
        use_nerd_fonts,
        indicator_style,
    })
}

pub fn save_config_settings(cfg: &Config) -> Result<(), String> {
    mbv_core::config::save_config_settings(cfg)
}

pub fn save_config_with_ui(cfg: &Config, ui: &UiConfig) {
    if let Err(e) = mbv_core::config::save_config_settings(cfg) {
        log::warn!(target: "config", "config save failed: {e}");
    }
    save_ui_config(ui);
}

pub fn save_ui_config(ui: &UiConfig) {
    let path = mbv_core::config::config_path();
    let mut doc: toml::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let Some(table) = doc.as_table_mut() else {
        return;
    };

    let display = table
        .entry("display".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();

    display.insert(
        "image_cache_size".to_string(),
        toml::Value::Integer(ui.image_cache_size as i64),
    );
    display.insert(
        "use_nerd_fonts".to_string(),
        toml::Value::Boolean(ui.use_nerd_fonts),
    );
    display.insert(
        "indicator_style".to_string(),
        toml::Value::String(ui.indicator_style.clone()),
    );
    match &ui.image_protocol {
        Some(protocol) => {
            display.insert(
                "image_protocol".to_string(),
                toml::Value::String(protocol.clone()),
            );
        }
        None => {
            display.remove("image_protocol");
            display.remove("card_image_protocol");
        }
    }

    if let Ok(text) = toml::to_string(&doc) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("toml.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

pub fn image_disk_cache_dir() -> PathBuf {
    mbv_core::config::cache_dir().join("images")
}

pub fn read_image_disk_cache(key: &str) -> Option<Vec<u8>> {
    let path = image_disk_cache_dir().join(safe_cache_filename(key));
    std::fs::read(path).ok()
}

/// Path to the on-disk cached image file for `key`, if one is already
/// present -- without reading its bytes. Used to build `mpris:artUrl`
/// `file://` URIs (see `src/mpris.rs::resolve_art_url`), which need the
/// path itself, not the decoded image data.
pub fn image_disk_cache_path(key: &str) -> Option<PathBuf> {
    let path = image_disk_cache_dir().join(safe_cache_filename(key));
    path.is_file().then_some(path)
}

/// Cache-key suffix for a card's primary image (see `src/app/render/card.rs`).
pub const IMAGE_CACHE_SUFFIX_CARD_PRIMARY: &str = "card";

/// Cache-key suffix for an album-level card
/// (see `src/app/render/card.rs`).
pub const IMAGE_CACHE_SUFFIX_ALBUM_CARD: &str = "album_card";

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
