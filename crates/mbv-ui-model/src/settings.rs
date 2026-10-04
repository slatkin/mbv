#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelFocus {
    Queue, // queue column (queue list below the card)
    #[default]
    Library, // library side (library browser); driven by library_tab
}

impl PanelFocus {
    #[must_use]
    pub fn from_pref(value: Option<&str>) -> Self {
        match value {
            Some("queue_side") => Self::Queue,
            Some("library_side") => Self::Library,
            _ => Self::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelMode {
    #[default]
    Both, // both panels: card+queue left, library right
    LibraryOnly, // left (queue) column hidden; library spans full window
    QueueOnly,   // right (library) column hidden; queue spans full window
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKey {
    Services,
    Keys,
    Panel,
    PanelSide,
    PanelCols,
    PanelColsExpanded,
    PanelGutterTop,
    PanelGutterBottom,
    PanelGutterLeft,
    PanelGutterRight,
    PanelAccent,
    PanelAccentColor,
    PanelAccentWidth,
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
    ResetUiState,
    LogOut,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsDestination {
    #[default]
    Main,
    Services,
    Keys,
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceEntry {
    Emby,
    Audiobookshelf,
    Feeds,
}

pub const SERVICE_ENTRIES: [ServiceEntry; 3] = [
    ServiceEntry::Emby,
    ServiceEntry::Audiobookshelf,
    ServiceEntry::Feeds,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceActionIntent {
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
pub static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
    ("Services", &[SettingKey::Services]),
    // Navigation-only entry (opens the read-only Keys destination); not a
    // config section, so `KeySection` has no `Keys` variant.
    ("Keys", &[SettingKey::Keys]),
    // Navigation-only entry for the pinned-panel page (design D6).
    ("Panel", &[SettingKey::Panel]),
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
    ("Actions", &[SettingKey::ResetUiState, SettingKey::LogOut]),
];

use crate::UiConfig;
use mbv_config::Config;

#[must_use]
pub fn setting_label(key: SettingKey) -> &'static str {
    match setting_kind(key) {
        SettingValueKind::Key => setting_key_label(key),
        SettingValueKind::Boolean => setting_boolean_label(key),
        SettingValueKind::Text => setting_text_label(key),
        SettingValueKind::Stepper => setting_stepper_label(key),
        SettingValueKind::Collection => setting_collection_label(key),
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
            (SettingKey::PanelCols, "Cols"),
            (SettingKey::PanelColsExpanded, "Expanded cols"),
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

fn label_for(key: SettingKey, labels: &[(SettingKey, &'static str)]) -> &'static str {
    labels
        .iter()
        .find_map(|(candidate, label)| (*candidate == key).then_some(*label))
        .unwrap_or_else(|| unreachable!("setting label group must contain its key"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingValueKind {
    Key,
    Boolean,
    Text,
    Stepper,
    Collection,
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
        | K::PanelAccent => V::Boolean,
        K::ImageProtocol
        | K::SubtitleMode
        | K::SubtitleLanguage
        | K::AudioLanguage
        | K::PanelSide
        | K::PanelAccentColor => V::Text,
        K::PanelCols
        | K::PanelColsExpanded
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
    }
    .unwrap_or_default()
}

fn setting_key_value(key: SettingKey, cfg: &Config) -> Option<String> {
    match key {
        SettingKey::Services => Some("Emby, Audiobookshelf, Feeds".into()),
        // Navigation-only entry: the destination row summarizes the saved
        // layout (design D6, row 4.3).
        SettingKey::Panel => Some(format!(
            "{} · {} cols",
            cfg.panel.side.as_str(),
            cfg.panel.cols
        )),
        SettingKey::Keys => {
            // Live keybind summary (design D7): the configured prefix and
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
        SettingKey::ShowSysTrayIcon => cfg.show_systray_icon,
        SettingKey::SystemNotifications => cfg.system_notifications,
        SettingKey::MouseSupport => cfg.mouse_support,
        SettingKey::PanelAccent => cfg.panel.accent,
        _ => return None,
    };
    Some(bool_val(value))
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
        SettingKey::PanelCols => Some(cfg.panel.cols.to_string()),
        SettingKey::PanelColsExpanded => Some(cfg.panel.cols_expanded.to_string()),
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

/// The Panel destination's rows, in paint and cursor order (design D6, row
/// 4.3): `Side` first, then the six stepper rows, then the three accent rows
/// (change `pinned-panel-focus-accent`).
pub const PANEL_SETTING_KEYS: [SettingKey; 10] = [
    SettingKey::PanelSide,
    SettingKey::PanelCols,
    SettingKey::PanelColsExpanded,
    SettingKey::PanelGutterTop,
    SettingKey::PanelGutterBottom,
    SettingKey::PanelGutterLeft,
    SettingKey::PanelGutterRight,
    SettingKey::PanelAccent,
    SettingKey::PanelAccentColor,
    SettingKey::PanelAccentWidth,
];

/// Main-page cursor ordinal to `SettingKey`.
#[must_use]
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

/// Panel-destination cursor ordinal to `SettingKey` (design D6, row 4.3).
#[must_use]
pub fn panel_cursor_to_key(cursor: usize) -> SettingKey {
    PANEL_SETTING_KEYS
        .get(cursor)
        .copied()
        .unwrap_or(SettingKey::PanelSide)
}

/// Step a column count by `delta`, clamped to `PANEL_COLS_MIN..=u16::MAX`
/// (a step past `u16::MAX` fails `try_from` and saturates at the top).
fn stepped_cols(cols: u16, delta: i32) -> u16 {
    u16::try_from(
        i32::from(cols)
            .saturating_add(delta)
            .max(i32::from(mbv_config::PANEL_COLS_MIN)),
    )
    .unwrap_or(u16::MAX)
}

/// Step the accent stroke's width by `delta`, clamped to `1..=65535`
/// (`pinned-panel-focus-accent`, design D3).
fn stepped_accent_width(width: std::num::NonZeroU16, delta: i32) -> std::num::NonZeroU16 {
    let next = i32::from(width.get())
        .saturating_add(delta)
        .clamp(1, i32::from(u16::MAX));
    // `next` is within 1..=u16::MAX, so both conversions succeed.
    std::num::NonZeroU16::new(u16::try_from(next).unwrap_or(1)).unwrap_or(std::num::NonZeroU16::MIN)
}

/// The next `Accent color` value in the row's fixed cycle palette (design
/// D3); a custom colour outside the palette steps to the palette's first.
fn next_accent_color(configured: mbv_config::PanelAccentColor) -> mbv_config::PanelAccentColor {
    const CYCLE: [mbv_config::PanelAccentColor; 5] = [
        mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
        mbv_config::PanelAccentColor([0x7f, 0xc8, 0xda]),
        mbv_config::PanelAccentColor([0xa3, 0xbe, 0x8c]),
        mbv_config::PanelAccentColor([0xbf, 0x61, 0x6a]),
        mbv_config::PanelAccentColor([0xff, 0xff, 0xff]),
    ];
    let next = CYCLE
        .iter()
        .position(|candidate| *candidate == configured)
        .map_or(0, |index| (index + 1) % CYCLE.len());
    CYCLE[next]
}

/// Apply one Panel-row interaction (design D6, row 4.3): `Side` toggles
/// left/right, and each numeric row steps by `delta` (already scaled to
/// ±1/±10 by the caller) clamped to its Rust range. `None` for any key that
/// is not a Panel value row.
#[must_use]
pub fn changed_panel_config(
    key: SettingKey,
    panel: mbv_config::PanelConfig,
    delta: i32,
) -> Option<mbv_config::PanelConfig> {
    use mbv_config::PanelSide;

    let mut next = panel;
    match key {
        SettingKey::PanelSide => {
            next.side = match panel.side {
                PanelSide::Left => PanelSide::Right,
                PanelSide::Right => PanelSide::Left,
            };
        }
        SettingKey::PanelCols => next.cols = stepped_cols(panel.cols, delta),
        SettingKey::PanelColsExpanded => {
            next.cols_expanded = stepped_cols(panel.cols_expanded, delta);
        }
        SettingKey::PanelGutterTop => next.gutter_top = panel.gutter_top.saturating_add(delta),
        SettingKey::PanelGutterBottom => {
            next.gutter_bottom = panel.gutter_bottom.saturating_add(delta);
        }
        SettingKey::PanelGutterLeft => {
            next.gutter_left = panel.gutter_left.saturating_add(delta);
        }
        SettingKey::PanelGutterRight => {
            next.gutter_right = panel.gutter_right.saturating_add(delta);
        }
        SettingKey::PanelAccent => next.accent = !panel.accent,
        SettingKey::PanelAccentColor => {
            next.accent_color = next_accent_color(panel.accent_color);
        }
        SettingKey::PanelAccentWidth => {
            next.accent_width = stepped_accent_width(panel.accent_width, delta);
        }
        _ => return None,
    }
    Some(next)
}

#[cfg(test)]
mod tests;
