#[cfg(any(test, feature = "test"))]
pub use mbv_config::LastRemoteConnection;
#[cfg(any(test, feature = "test"))]
pub use mbv_config::TestStateDirGuard;
pub use mbv_config::{
    Config, is_system_instance, load_library_position_state, migrate_legacy_emby_token, prefs_path,
};
#[cfg(any(test, feature = "test"))]
pub use mbv_config::{load_last_remote_connection, save_last_remote_connection};

use unicode_width::UnicodeWidthStr;

pub const DEFAULT_VISUALIZER_GLYPH: &str = "●";

#[cfg(any(test, feature = "test"))]
pub mod tests {
    pub use mbv_config::tests::SYS_ENV_LOCK;
}

pub use mbv_ui_model::UiConfig;

pub fn load_config() -> Result<Config, mbv_config::ConfigError> {
    mbv_config::load_config()
}

pub fn load_ui_config() -> Result<UiConfig, mbv_config::ConfigError> {
    let path = mbv_config::config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(UiConfig::default());
    };
    parse_ui_config(&text)
}

fn parse_ui_config(text: &str) -> Result<UiConfig, mbv_config::ConfigError> {
    let doc: toml::Value = toml::from_str(text)?;
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

pub fn save_config_section(
    cfg: &Config,
    section: mbv_config::ConfigSection,
) -> Result<(), mbv_config::ConfigError> {
    mbv_config::save_config_section(cfg, section)
}

pub fn save_config_with_ui(cfg: &Config, ui: &UiConfig) {
    if let Err(e) = mbv_config::save_config_settings(cfg) {
        tracing::warn!(
            name: "config.save.failed",
            target: "config",
            error = %e,
            "config save failed"
        );
    }
    save_ui_config(ui);
}

/// Writes the `display` section of the config file, keeping every other key.
///
/// # Panics
///
/// Panics when the config file's `display` key holds something other than a
/// table.
pub fn save_ui_config(ui: &UiConfig) {
    let path = mbv_config::config_path();
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
    use super::{DEFAULT_VISUALIZER_GLYPH, parse_ui_config, validated_visualizer_glyph};

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
