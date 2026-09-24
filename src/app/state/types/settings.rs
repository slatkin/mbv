#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(in crate::app) enum PanelFocus {
    Queue, // queue column (queue list below the card)
    #[default]
    Library, // library side (library browser); driven by library_tab
}

impl PanelFocus {
    pub(in crate::app) fn from_pref(value: Option<&str>) -> Self {
        match value {
            Some("queue_side") => Self::Queue,
            Some("library_side") => Self::Library,
            _ => Self::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(in crate::app) enum PanelMode {
    #[default]
    Both, // both panels: card+queue left, library right
    LibraryOnly, // left (queue) column hidden; library spans full window
    QueueOnly,   // right (library) column hidden; queue spans full window
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingKey {
    Services,
    Keys,
    StayAlive,
    AutoReconnect,
    SavePlaylistOnQuit,
    AlwaysPlayNext,
    ConsumeVideos,
    ConsumeAudio,
    SavePlaylistOnConsume,
    SavePlaylistOnConsumeAudio,
    AlwaysSkipIntro,
    ImageProtocol,
    HiddenLibraries,
    ShowAudioWindow,
    UseMpvConfig,
    NoScripts,
    Autoload,
    ShowSysTrayIcon,
    SystemNotifications,
    MouseSupport,
    MyLanguages,
    SubtitleMode,
    FeedViewLibraries,
    LibraryRoutes,
    SubtitleLanguage,
    AudioLanguage,
    ManageFeeds,
    LogOut,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsDestination {
    #[default]
    Main,
    Services,
    Keys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ServiceEntry {
    Emby,
    Audiobookshelf,
    Feeds,
}

pub(in crate::app) const SERVICE_ENTRIES: [ServiceEntry; 3] = [
    ServiceEntry::Emby,
    ServiceEntry::Audiobookshelf,
    ServiceEntry::Feeds,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ServiceActionIntent {
    SetupEmby,
    RetryEmby,
    RepairEmby,
    SetupAudiobookshelf,
    TestAudiobookshelf,
    RemoveAudiobookshelf,
    ReplaceAudiobookshelf,
    ManageFeeds,
}

// Sections rendered as YELLOW blocks in a 2×2 grid.
// LogOut is rendered separately as a plain line below the grid.
pub(in crate::app) static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
    ("Services", &[SettingKey::Services]),
    // Navigation-only entry (opens the read-only Keys destination); not a
    // config section, so `KeySection` has no `Keys` variant.
    ("Keys", &[SettingKey::Keys]),
    (
        "Playback",
        &[
            SettingKey::SubtitleMode,
            SettingKey::SubtitleLanguage,
            SettingKey::AudioLanguage,
            SettingKey::MyLanguages,
        ],
    ),
    (
        "Display",
        &[
            SettingKey::ImageProtocol,
            SettingKey::SystemNotifications,
            SettingKey::MouseSupport,
        ],
    ),
    (
        "Session",
        &[
            SettingKey::StayAlive,
            SettingKey::AutoReconnect,
            SettingKey::AlwaysSkipIntro,
            SettingKey::SavePlaylistOnQuit,
            SettingKey::ShowSysTrayIcon,
        ],
    ),
    (
        "Library",
        &[
            SettingKey::HiddenLibraries,
            SettingKey::FeedViewLibraries,
            SettingKey::LibraryRoutes,
        ],
    ),
    (
        "Queue",
        &[
            SettingKey::AlwaysPlayNext,
            SettingKey::ConsumeVideos,
            SettingKey::ConsumeAudio,
            SettingKey::SavePlaylistOnConsume,
            SettingKey::SavePlaylistOnConsumeAudio,
        ],
    ),
    (
        "Mpv",
        &[
            SettingKey::ShowAudioWindow,
            SettingKey::UseMpvConfig,
            SettingKey::NoScripts,
            SettingKey::Autoload,
        ],
    ),
    ("Feeds", &[SettingKey::ManageFeeds]),
    ("Actions", &[SettingKey::LogOut]),
];

use crate::config::{Config, UiConfig};

pub fn setting_label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::Services => "Services",
        SettingKey::Keys => "Keys",
        SettingKey::StayAlive => "Stay alive on exit",
        SettingKey::AutoReconnect => "Auto reconnect",
        SettingKey::SavePlaylistOnQuit => "Save playlist on quit",
        SettingKey::AlwaysPlayNext => "Always play next",
        SettingKey::ConsumeVideos => "Consume videos",
        SettingKey::ConsumeAudio => "Consume audio",
        SettingKey::SavePlaylistOnConsume => "Save playlist on consume",
        SettingKey::SavePlaylistOnConsumeAudio => "Save playlist on consume (audio)",
        SettingKey::AlwaysSkipIntro => "Always skip intro",
        SettingKey::ImageProtocol => "Image protocol",
        SettingKey::HiddenLibraries => "Hidden libraries",
        SettingKey::ShowAudioWindow => "Show audio window",
        SettingKey::UseMpvConfig => "Use mpv config",
        SettingKey::NoScripts => "No scripts",
        SettingKey::Autoload => "autoload",
        SettingKey::ShowSysTrayIcon => "Show systray icon",
        SettingKey::SystemNotifications => "System notifications",
        SettingKey::MouseSupport => "Mouse support",
        SettingKey::MyLanguages => "My languages",
        SettingKey::SubtitleMode => "Subtitle mode",

        SettingKey::SubtitleLanguage => "Subtitle language",
        SettingKey::AudioLanguage => "Audio language",
        SettingKey::FeedViewLibraries => "Feed view",
        SettingKey::LibraryRoutes => "Library routes",
        SettingKey::ManageFeeds => "Manage feeds",
        SettingKey::LogOut => "Log out",
    }
}

pub fn setting_value(key: SettingKey, cfg: &Config, ui: &UiConfig) -> String {
    match key {
        SettingKey::Services => "Emby, Audiobookshelf, Feeds".into(),
        SettingKey::Keys => {
            // Live keybind summary (design D7): the configured prefix and
            // the number of actions whose router binding deviates from the
            // declared default, both from the loaded configuration.
            let keys = &cfg.keybinds;
            let prefix = keys.prefix.map(|chord| chord.to_string());
            let count = keys.override_count();
            match (prefix, count) {
                (None, 0) => "defaults".into(),
                (None, n) => format!("{n} overridden"),
                (Some(prefix), 0) => format!("{prefix} · defaults"),
                (Some(prefix), n) => format!("{prefix} · {n} overridden"),
            }
        }
        SettingKey::StayAlive => bool_val(cfg.stay_alive),
        SettingKey::AutoReconnect => bool_val(cfg.auto_reconnect),
        SettingKey::SavePlaylistOnQuit => bool_val(cfg.save_playlist_on_quit),
        SettingKey::AlwaysPlayNext => bool_val(cfg.always_play_next),
        SettingKey::ConsumeVideos => bool_val(cfg.consume_videos),
        SettingKey::ConsumeAudio => bool_val(cfg.consume_audio),
        SettingKey::SavePlaylistOnConsume => bool_val(cfg.save_playlist_on_consume),
        SettingKey::SavePlaylistOnConsumeAudio => bool_val(cfg.save_playlist_on_consume_audio),
        SettingKey::AlwaysSkipIntro => bool_val(cfg.always_skip_intro),
        SettingKey::ImageProtocol => ui.image_protocol.clone().unwrap_or_else(|| "none".into()),
        SettingKey::HiddenLibraries => fmt_hidden_list(&cfg.hidden_libraries),
        SettingKey::ShowAudioWindow => bool_val(cfg.show_audio_window),
        SettingKey::UseMpvConfig => bool_val(cfg.use_mpv_config),
        SettingKey::NoScripts => bool_val(cfg.no_scripts),
        SettingKey::Autoload => bool_val(cfg.autoload),
        SettingKey::ShowSysTrayIcon => bool_val(cfg.show_systray_icon),
        SettingKey::SystemNotifications => bool_val(cfg.system_notifications),
        SettingKey::MouseSupport => bool_val(cfg.mouse_support),
        SettingKey::MyLanguages => fmt_lang_list(&cfg.my_languages),
        SettingKey::SubtitleMode => {
            if cfg.subtitle_mode.is_empty() {
                "Default".into()
            } else {
                cfg.subtitle_mode.clone()
            }
        }

        SettingKey::SubtitleLanguage => {
            if cfg.subtitle_lang.is_empty() {
                "any".into()
            } else {
                cfg.subtitle_lang.clone()
            }
        }
        SettingKey::AudioLanguage => {
            if cfg.audio_lang.is_empty() {
                "any".into()
            } else {
                cfg.audio_lang.clone()
            }
        }
        SettingKey::FeedViewLibraries => fmt_feed_view_list(&cfg.feed_view_libraries),
        SettingKey::LibraryRoutes => fmt_library_routes(&cfg.library_routes),
        SettingKey::ManageFeeds => fmt_feeds_list(&cfg.feeds),
        SettingKey::LogOut => String::new(),
    }
}

pub fn fmt_hidden_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} hidden"),
    }
}

pub fn fmt_lang_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} languages"),
    }
}

pub fn fmt_feed_view_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} libraries"),
    }
}

pub fn fmt_feeds_list(list: &[mbv_core::config::FeedSubscription]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].name.clone(),
        n => format!("{n} feeds"),
    }
}

pub fn fmt_library_routes(routes: &std::collections::HashMap<String, String>) -> String {
    match routes.len() {
        0 => "none".into(),
        1 => {
            let (lib, dev) = routes.iter().next().unwrap();
            format!("{lib} -> {dev}")
        }
        n => format!("{n} routes"),
    }
}

pub fn bool_val(v: bool) -> String {
    if v {
        "on".into()
    } else {
        "off".into()
    }
}

pub fn settings_cursor_to_key(cursor: usize) -> SettingKey {
    let mut idx = 0;
    for &(_, keys) in SETTING_SECTIONS {
        for &key in keys {
            if idx == cursor {
                return key;
            }
            idx += 1;
        }
    }
    SettingKey::LogOut
}
