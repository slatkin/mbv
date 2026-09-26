pub use mbv_core::config::{
    clear_queue_state, is_system_instance, load_library_position_state, load_queue_state,
    migrate_legacy_emby_token, prefs_path, save_queue_state, Config, LibraryPosition,
    LibraryPositionLevel, LibraryPositionState, QueueSource, QueueState,
};
#[cfg(test)]
pub use mbv_core::config::{
    load_last_remote_connection, save_last_remote_connection, save_library_position_state,
};
#[cfg(test)]
pub use mbv_core::config::{LastRemoteConnection, TestStateDirGuard};

use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

pub const DEFAULT_VISUALIZER_GLYPH: &str = "●";

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
    pub visualizer_glyph: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            image_protocol: None,
            image_cache_size: 50,
            use_nerd_fonts: false,
            indicator_style: "keyvalue".into(),
            visualizer_glyph: DEFAULT_VISUALIZER_GLYPH.into(),
        }
    }
}

pub fn load_config() -> Result<Config, String> {
    mbv_core::config::load_config()
}

pub fn load_ui_config() -> Result<UiConfig, String> {
    let path = mbv_core::config::config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(UiConfig::default());
    };
    parse_ui_config(&text).map_err(|e| format!("Config parse error in {}: {e}", path.display()))
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
        .and_then(toml::Value::as_integer)
        .map_or(50, |v| usize::try_from(v.max(1)).unwrap_or(usize::MAX));
    let use_nerd_fonts = display
        .and_then(|m| m.get("use_nerd_fonts"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let indicator_style = display
        .and_then(|m| m.get("indicator_style"))
        .and_then(|v| v.as_str())
        .unwrap_or("keyvalue")
        .to_string();
    let visualizer_glyph = validated_visualizer_glyph(
        display
            .and_then(|m| m.get("visualizer_glyph"))
            .and_then(|v| v.as_str()),
    );

    Ok(UiConfig {
        image_protocol,
        image_cache_size,
        use_nerd_fonts,
        indicator_style,
        visualizer_glyph,
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
        toml::Value::Integer(i64::try_from(ui.image_cache_size).unwrap_or(i64::MAX)),
    );
    display.insert(
        "use_nerd_fonts".to_string(),
        toml::Value::Boolean(ui.use_nerd_fonts),
    );
    display.insert(
        "indicator_style".to_string(),
        toml::Value::String(ui.indicator_style.clone()),
    );
    display.insert(
        "visualizer_glyph".to_string(),
        toml::Value::String(validated_visualizer_glyph(Some(&ui.visualizer_glyph))),
    );
    if let Some(protocol) = &ui.image_protocol {
        display.insert(
            "image_protocol".to_string(),
            toml::Value::String(protocol.clone()),
        );
    } else {
        display.remove("image_protocol");
        display.remove("card_image_protocol");
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

fn validated_visualizer_glyph(value: Option<&str>) -> String {
    let Some(value) = value else {
        return DEFAULT_VISUALIZER_GLYPH.into();
    };
    if value.is_empty() || value.chars().any(char::is_control) || UnicodeWidthStr::width(value) != 1
    {
        DEFAULT_VISUALIZER_GLYPH.into()
    } else {
        value.into()
    }
}

#[cfg(test)]
mod ui_config_tests {
    use super::{parse_ui_config, validated_visualizer_glyph, DEFAULT_VISUALIZER_GLYPH};

    #[test]
    fn visualizer_glyph_round_trips_and_invalid_values_fall_back() {
        let config = parse_ui_config("[display]\nvisualizer_glyph = \"x\"\n").unwrap();
        assert_eq!(config.visualizer_glyph, "x");

        let serialized = format!(
            "[display]\nvisualizer_glyph = \"{}\"\n",
            config.visualizer_glyph
        );
        assert_eq!(parse_ui_config(&serialized).unwrap().visualizer_glyph, "x");

        for value in ["", "界"] {
            let invalid = format!("[display]\nvisualizer_glyph = \"{value}\"\n");
            assert_eq!(
                parse_ui_config(&invalid).unwrap().visualizer_glyph,
                DEFAULT_VISUALIZER_GLYPH
            );
        }
        assert_eq!(
            validated_visualizer_glyph(Some("\n")),
            DEFAULT_VISUALIZER_GLYPH
        );
    }
}

pub fn image_disk_cache_dir() -> PathBuf {
    mbv_core::config::cache_dir().join("images")
}

pub fn read_image_disk_cache(key: &str) -> Option<Vec<u8>> {
    let path = image_disk_cache_dir().join(safe_cache_filename(key));
    let bytes = std::fs::read(&path).ok()?;
    touch_image_disk_cache(&path);
    Some(bytes)
}

/// Path to the on-disk cached image file for `key`, if one is already
/// present -- without reading its bytes. Used to build `mpris:artUrl`
/// `file://` URIs (see `src/mpris.rs::resolve_art_url`), which need the
/// path itself, not the decoded image data.
pub fn image_disk_cache_path(key: &str) -> Option<PathBuf> {
    let path = image_disk_cache_dir().join(safe_cache_filename(key));
    if !path.is_file() {
        return None;
    }
    touch_image_disk_cache(&path);
    Some(path)
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

pub fn clear_image_disk_cache_prefix(prefix: &str) {
    let Ok(entries) = std::fs::read_dir(image_disk_cache_dir()) else {
        return;
    };
    let prefix = safe_cache_filename(prefix);
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(not(test))]
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
                if meta.modified().is_ok_and(|m| m < cutoff) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    });
}

/// Best-effort mtime refresh marking a cache file as recently used, so the
/// 30-day mtime eviction (`evict_old_image_cache`) measures last use rather
/// than first write. Reads never update mtime on their own, so without this
/// every regularly-viewed image still ages out and the cache wipes itself.
/// Throttled to one write per file per day; all failures ignored.
fn touch_image_disk_cache(path: &std::path::Path) {
    let stale = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| {
            std::time::SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|age| age.as_secs() >= 24 * 3600)
        });
    if stale {
        // Read-only open: refreshing mtime needs no write access to the file.
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
    }
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
