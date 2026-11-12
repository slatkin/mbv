//! The Settings rows' presentation text (`add-gui-settings-screen` design D3,
//! D4): each row's label, how it is edited, its value text, and its toggle
//! flag. Both Clients paint these, so they sit beside the declarations they
//! read rather than in a Client. Split out of `settings.rs` along that
//! responsibility seam when the file crossed its line ceiling.

use crate::UiConfig;
use mbv_config::Config;

use super::SettingKey;

#[must_use]
pub fn setting_label(key: SettingKey) -> &'static str {
    match setting_kind(key) {
        SettingValueKind::Key => setting_key_label(key),
        SettingValueKind::Boolean => setting_boolean_label(key),
        SettingValueKind::Text => setting_text_label(key),
        SettingValueKind::Stepper => setting_stepper_label(key),
        SettingValueKind::Collection => setting_collection_label(key),
        SettingValueKind::Info => setting_info_label(key),
    }
}

fn setting_key_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::Services, "Services"),
            (SettingKey::Keys, "Keys"),
            (SettingKey::Panel, "Panel"),
        ],
    )
}

fn setting_boolean_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::StayAlive, "Stay alive on exit"),
            (SettingKey::AutoReconnect, "Auto reconnect"),
            (SettingKey::SavePlaylistOnQuit, "Save playlist on quit"),
            (SettingKey::AlwaysPlayNext, "Always play next"),
            (SettingKey::ConsumeVideos, "Consume videos"),
            (SettingKey::ConsumeAudio, "Consume audio"),
            (
                SettingKey::SavePlaylistOnConsume,
                "Save playlist on consume",
            ),
            (
                SettingKey::SavePlaylistOnConsumeAudio,
                "Save playlist on consume (audio)",
            ),
            (SettingKey::AlwaysSkipIntro, "Always skip intro"),
            (SettingKey::ShowAudioWindow, "Show audio window"),
            (SettingKey::UseMpvConfig, "Use mpv config"),
            (SettingKey::NoScripts, "No scripts"),
            (SettingKey::Autoload, "autoload"),
            (SettingKey::ShowSysTrayIcon, "Show systray icon"),
            (SettingKey::SystemNotifications, "System notifications"),
            (SettingKey::MouseSupport, "Mouse support"),
            (SettingKey::PanelAccent, "Accent"),
            (SettingKey::PanelReserve, "Reserve space"),
        ],
    )
}

fn setting_text_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::ImageProtocol, "Image protocol"),
            (SettingKey::SubtitleMode, "Subtitle mode"),
            (SettingKey::SubtitleLanguage, "Subtitle language"),
            (SettingKey::AudioLanguage, "Audio language"),
            (SettingKey::PanelSide, "Side"),
            (SettingKey::PanelAccentColor, "Accent color"),
        ],
    )
}

fn setting_stepper_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::PanelWidth, "Width"),
            (SettingKey::PanelWidthExpanded, "Expanded width"),
            (SettingKey::PanelGutterTop, "Gutter top"),
            (SettingKey::PanelGutterBottom, "Gutter bottom"),
            (SettingKey::PanelGutterLeft, "Gutter left"),
            (SettingKey::PanelGutterRight, "Gutter right"),
            (SettingKey::PanelAccentWidth, "Accent width"),
        ],
    )
}

fn setting_collection_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::HiddenLibraries, "Hidden libraries"),
            (SettingKey::MyLanguages, "My languages"),
            (SettingKey::FeedViewLibraries, "Feed view"),
            (SettingKey::LibraryRoutes, "Library routes"),
            (SettingKey::ManageFeeds, "Manage feeds"),
            (SettingKey::ResetUiState, "Reset UI state"),
            (SettingKey::LogOut, "Log out"),
        ],
    )
}

fn setting_info_label(key: SettingKey) -> &'static str {
    label_for(
        key,
        &[
            (SettingKey::ClientVersion, "This client"),
            (SettingKey::OwnerVersion, "Owner"),
        ],
    )
}

fn label_for(key: SettingKey, labels: &[(SettingKey, &'static str)]) -> &'static str {
    labels
        .iter()
        .find_map(|(candidate, label)| (*candidate == key).then_some(*label))
        // A key missing from its group table is a table gap, not a reason to
        // kill the process that is drawing the settings screen: the row shows
        // a neutral label instead.
        .unwrap_or("Unknown setting")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingValueKind {
    Key,
    Boolean,
    Text,
    Stepper,
    Collection,
    /// A read-only row whose value the caller supplies (the version rows).
    /// It takes no toggle, stepper, or activation effect.
    Info,
}

#[must_use]
pub fn setting_kind(key: SettingKey) -> SettingValueKind {
    use SettingKey as K;
    use SettingValueKind as V;

    match key {
        K::Services | K::Keys | K::Panel => V::Key,
        K::StayAlive
        | K::AutoReconnect
        | K::SavePlaylistOnQuit
        | K::AlwaysPlayNext
        | K::ConsumeVideos
        | K::ConsumeAudio
        | K::SavePlaylistOnConsume
        | K::SavePlaylistOnConsumeAudio
        | K::AlwaysSkipIntro
        | K::ShowAudioWindow
        | K::UseMpvConfig
        | K::NoScripts
        | K::Autoload
        | K::ShowSysTrayIcon
        | K::SystemNotifications
        | K::MouseSupport
        | K::PanelAccent
        | K::PanelReserve => V::Boolean,
        K::ImageProtocol
        | K::SubtitleMode
        | K::SubtitleLanguage
        | K::AudioLanguage
        | K::PanelSide
        | K::PanelAccentColor => V::Text,
        K::PanelWidth
        | K::PanelWidthExpanded
        | K::PanelGutterTop
        | K::PanelGutterBottom
        | K::PanelGutterLeft
        | K::PanelGutterRight
        | K::PanelAccentWidth => V::Stepper,
        K::HiddenLibraries
        | K::MyLanguages
        | K::FeedViewLibraries
        | K::LibraryRoutes
        | K::ManageFeeds
        | K::ResetUiState
        | K::LogOut => V::Collection,
        K::ClientVersion | K::OwnerVersion => V::Info,
    }
}

#[must_use]
pub fn setting_value(key: SettingKey, cfg: &Config, ui: &UiConfig) -> String {
    match setting_kind(key) {
        SettingValueKind::Key => setting_key_value(key, cfg),
        SettingValueKind::Boolean => setting_boolean_value(key, cfg),
        SettingValueKind::Text => setting_text_value(key, cfg, ui),
        SettingValueKind::Stepper => setting_stepper_value(key, cfg),
        SettingValueKind::Collection => setting_collection_value(key, cfg),
        // The version rows' values come from `App`, not from configuration;
        // the caller fills them.
        SettingValueKind::Info => None,
    }
    .unwrap_or_default()
}

fn setting_key_value(key: SettingKey, cfg: &Config) -> Option<String> {
    match key {
        SettingKey::Services => Some("Emby, Audiobookshelf, Feeds".into()),
        // Navigation-only entry: the destination row summarizes the saved
        // layout.
        SettingKey::Panel => Some(format!(
            "{} · {} px",
            cfg.panel.side.as_str(),
            cfg.panel.width
        )),
        SettingKey::Keys => {
            // Live keybind summary: the configured prefix and
            // the number of actions whose router binding deviates from the
            // declared default, both from the loaded configuration.
            let keys = &cfg.keybinds;
            let prefix = keys.prefix.map(|chord| chord.to_string());
            let count = keys.override_count();
            Some(match (prefix, count) {
                (None, 0) => "defaults".into(),
                (None, n) => format!("{n} overridden"),
                (Some(prefix), 0) => format!("{prefix} · defaults"),
                (Some(prefix), n) => format!("{prefix} · {n} overridden"),
            })
        }
        _ => None,
    }
}

fn setting_boolean_value(key: SettingKey, cfg: &Config) -> Option<String> {
    setting_flag(key, cfg).map(bool_val)
}

/// The boolean setting's value, `None` for a key that is no boolean. The
/// Settings frame's toggle rows carry the flag itself (`add-gui-settings-screen`
/// design D9), and the TUI's text value is this flag through [`bool_val`].
#[must_use]
pub fn setting_flag(key: SettingKey, cfg: &Config) -> Option<bool> {
    let value = match key {
        SettingKey::StayAlive => cfg.stay_alive,
        SettingKey::AutoReconnect => cfg.auto_reconnect,
        SettingKey::SavePlaylistOnQuit => cfg.save_playlist_on_quit,
        SettingKey::AlwaysPlayNext => cfg.always_play_next,
        SettingKey::ConsumeVideos => cfg.consume_videos,
        SettingKey::ConsumeAudio => cfg.consume_audio,
        SettingKey::SavePlaylistOnConsume => cfg.save_playlist_on_consume,
        SettingKey::SavePlaylistOnConsumeAudio => cfg.save_playlist_on_consume_audio,
        SettingKey::AlwaysSkipIntro => cfg.always_skip_intro,
        SettingKey::ShowAudioWindow => cfg.show_audio_window,
        SettingKey::UseMpvConfig => cfg.use_mpv_config,
        SettingKey::NoScripts => cfg.no_scripts,
        SettingKey::Autoload => cfg.autoload,
        // The Tray is enabled while stay-alive is on, whatever the stored
        // preference.
        SettingKey::ShowSysTrayIcon => cfg.tray_enabled(),
        SettingKey::SystemNotifications => cfg.system_notifications,
        SettingKey::MouseSupport => cfg.mouse_support,
        SettingKey::PanelAccent => cfg.panel.accent,
        SettingKey::PanelReserve => cfg.panel.reserve,
        _ => return None,
    };
    Some(value)
}

fn setting_text_value(key: SettingKey, cfg: &Config, ui: &UiConfig) -> Option<String> {
    match key {
        SettingKey::ImageProtocol => {
            Some(ui.image_protocol.clone().unwrap_or_else(|| "none".into()))
        }
        SettingKey::SubtitleMode => Some(if cfg.subtitle_mode.is_empty() {
            "Default".into()
        } else {
            cfg.subtitle_mode.clone()
        }),
        SettingKey::SubtitleLanguage => Some(if cfg.subtitle_lang.is_empty() {
            "any".into()
        } else {
            cfg.subtitle_lang.clone()
        }),
        SettingKey::AudioLanguage => Some(if cfg.audio_lang.is_empty() {
            "any".into()
        } else {
            cfg.audio_lang.clone()
        }),
        SettingKey::PanelSide => Some(cfg.panel.side.as_str().to_string()),
        SettingKey::PanelAccentColor => Some(cfg.panel.accent_color.to_string()),
        _ => None,
    }
}

fn setting_stepper_value(key: SettingKey, cfg: &Config) -> Option<String> {
    match key {
        SettingKey::PanelWidth => Some(cfg.panel.width.to_string()),
        SettingKey::PanelWidthExpanded => Some(cfg.panel.width_expanded.to_string()),
        SettingKey::PanelGutterTop => Some(cfg.panel.gutter_top.to_string()),
        SettingKey::PanelGutterBottom => Some(cfg.panel.gutter_bottom.to_string()),
        SettingKey::PanelGutterLeft => Some(cfg.panel.gutter_left.to_string()),
        SettingKey::PanelGutterRight => Some(cfg.panel.gutter_right.to_string()),
        SettingKey::PanelAccentWidth => Some(cfg.panel.accent_width.get().to_string()),
        _ => None,
    }
}

fn setting_collection_value(key: SettingKey, cfg: &Config) -> Option<String> {
    match key {
        SettingKey::HiddenLibraries => Some(fmt_hidden_list(&cfg.hidden_libraries)),
        SettingKey::MyLanguages => Some(fmt_lang_list(&cfg.my_languages)),
        SettingKey::FeedViewLibraries => Some(fmt_feed_view_list(&cfg.feed_view_libraries)),
        SettingKey::LibraryRoutes => Some(fmt_library_routes(&cfg.library_routes)),
        SettingKey::ManageFeeds => Some(fmt_feeds_list(&cfg.feeds)),
        SettingKey::ResetUiState | SettingKey::LogOut => Some(String::new()),
        _ => None,
    }
}

#[must_use]
pub fn fmt_hidden_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} hidden"),
    }
}

#[must_use]
pub fn fmt_lang_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} languages"),
    }
}

#[must_use]
pub fn fmt_feed_view_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} libraries"),
    }
}

#[must_use]
pub fn fmt_feeds_list(list: &[mbv_config::FeedSubscription]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].name.clone(),
        n => format!("{n} feeds"),
    }
}

#[must_use]
pub fn fmt_library_routes(routes: &std::collections::BTreeMap<String, String>) -> String {
    match routes.len() {
        0 => "none".into(),
        1 => routes
            .first_key_value()
            .map_or_else(|| "none".into(), |(lib, dev)| format!("{lib} -> {dev}")),
        n => format!("{n} routes"),
    }
}

#[must_use]
pub fn bool_val(v: bool) -> String {
    if v { "on".into() } else { "off".into() }
}
