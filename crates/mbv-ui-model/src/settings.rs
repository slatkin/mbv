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

/// Which Clients a setting has an effect in. Each `SettingKey` declares its
/// scope once, beside its own declaration (`add-gui-settings-screen` D4); the
/// GUI Settings screen lists only `AllClients` rows, and the TUI ignores the
/// scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingScope {
    /// The setting has an effect in every Client.
    AllClients,
    /// The setting has an effect in the TUI only (pinned-panel geometry,
    /// terminal image protocol, mouse support).
    TuiOnly,
    /// No Client acts on the setting (`LogOut`; a separate bug owns it).
    Inert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKey {
    Services,
    Keys,
    Panel,
    PanelSide,
    PanelWidth,
    PanelWidthExpanded,
    PanelReserve,
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

impl SettingKey {
    /// The key's stable wire name (`add-gui-settings-screen` design D5): the
    /// name a Settings frame row carries, and the name the bridge's
    /// `activate` command sends back, so an edit addresses the key the frame
    /// painted and never a row position.
    #[must_use]
    pub fn name(self) -> &'static str {
        use SettingKey as K;
        match self {
            K::Services => "services",
            K::Keys => "keys",
            K::Panel => "panel",
            K::PanelSide => "panelSide",
            K::PanelWidth => "panelWidth",
            K::PanelWidthExpanded => "panelWidthExpanded",
            K::PanelReserve => "panelReserve",
            K::PanelGutterTop => "panelGutterTop",
            K::PanelGutterBottom => "panelGutterBottom",
            K::PanelGutterLeft => "panelGutterLeft",
            K::PanelGutterRight => "panelGutterRight",
            K::PanelAccent => "panelAccent",
            K::PanelAccentColor => "panelAccentColor",
            K::PanelAccentWidth => "panelAccentWidth",
            K::StayAlive => "stayAlive",
            K::AutoReconnect => "autoReconnect",
            K::SavePlaylistOnQuit => "savePlaylistOnQuit",
            K::AlwaysPlayNext => "alwaysPlayNext",
            K::ConsumeVideos => "consumeVideos",
            K::ConsumeAudio => "consumeAudio",
            K::SavePlaylistOnConsume => "savePlaylistOnConsume",
            K::SavePlaylistOnConsumeAudio => "savePlaylistOnConsumeAudio",
            K::AlwaysSkipIntro => "alwaysSkipIntro",
            K::ImageProtocol => "imageProtocol",
            K::HiddenLibraries => "hiddenLibraries",
            K::ShowAudioWindow => "showAudioWindow",
            K::UseMpvConfig => "useMpvConfig",
            K::NoScripts => "noScripts",
            K::Autoload => "autoload",
            K::ShowSysTrayIcon => "showSysTrayIcon",
            K::SystemNotifications => "systemNotifications",
            K::MouseSupport => "mouseSupport",
            K::MyLanguages => "myLanguages",
            K::SubtitleMode => "subtitleMode",
            K::FeedViewLibraries => "feedViewLibraries",
            K::LibraryRoutes => "libraryRoutes",
            K::SubtitleLanguage => "subtitleLanguage",
            K::AudioLanguage => "audioLanguage",
            K::ManageFeeds => "manageFeeds",
            K::ResetUiState => "resetUiState",
            K::LogOut => "logOut",
        }
    }

    /// The key a stable wire name names, `None` for a name no key declares —
    /// a command built from a stale frame changes nothing.
    #[must_use]
    pub fn from_name(name: &str) -> Option<SettingKey> {
        use SettingKey as K;
        Some(match name {
            "services" => K::Services,
            "keys" => K::Keys,
            "panel" => K::Panel,
            "panelSide" => K::PanelSide,
            "panelWidth" => K::PanelWidth,
            "panelWidthExpanded" => K::PanelWidthExpanded,
            "panelReserve" => K::PanelReserve,
            "panelGutterTop" => K::PanelGutterTop,
            "panelGutterBottom" => K::PanelGutterBottom,
            "panelGutterLeft" => K::PanelGutterLeft,
            "panelGutterRight" => K::PanelGutterRight,
            "panelAccent" => K::PanelAccent,
            "panelAccentColor" => K::PanelAccentColor,
            "panelAccentWidth" => K::PanelAccentWidth,
            "stayAlive" => K::StayAlive,
            "autoReconnect" => K::AutoReconnect,
            "savePlaylistOnQuit" => K::SavePlaylistOnQuit,
            "alwaysPlayNext" => K::AlwaysPlayNext,
            "consumeVideos" => K::ConsumeVideos,
            "consumeAudio" => K::ConsumeAudio,
            "savePlaylistOnConsume" => K::SavePlaylistOnConsume,
            "savePlaylistOnConsumeAudio" => K::SavePlaylistOnConsumeAudio,
            "alwaysSkipIntro" => K::AlwaysSkipIntro,
            "imageProtocol" => K::ImageProtocol,
            "hiddenLibraries" => K::HiddenLibraries,
            "showAudioWindow" => K::ShowAudioWindow,
            "useMpvConfig" => K::UseMpvConfig,
            "noScripts" => K::NoScripts,
            "autoload" => K::Autoload,
            "showSysTrayIcon" => K::ShowSysTrayIcon,
            "systemNotifications" => K::SystemNotifications,
            "mouseSupport" => K::MouseSupport,
            "myLanguages" => K::MyLanguages,
            "subtitleMode" => K::SubtitleMode,
            "feedViewLibraries" => K::FeedViewLibraries,
            "libraryRoutes" => K::LibraryRoutes,
            "subtitleLanguage" => K::SubtitleLanguage,
            "audioLanguage" => K::AudioLanguage,
            "manageFeeds" => K::ManageFeeds,
            "resetUiState" => K::ResetUiState,
            "logOut" => K::LogOut,
            _ => return None,
        })
    }

    /// The Clients this setting has an effect in. An exhaustive match with
    /// no wildcard: a new key must declare its scope when it is added.
    #[must_use]
    pub fn scope(self) -> SettingScope {
        use SettingKey as K;
        match self {
            // The pinned-panel page: pinwin geometry, gutters, and accent are
            // terminal-panel state the GUI window does not paint.
            K::Panel
            | K::PanelSide
            | K::PanelWidth
            | K::PanelWidthExpanded
            | K::PanelReserve
            | K::PanelGutterTop
            | K::PanelGutterBottom
            | K::PanelGutterLeft
            | K::PanelGutterRight
            | K::PanelAccent
            | K::PanelAccentColor
            | K::PanelAccentWidth
            | K::ImageProtocol
            | K::MouseSupport => SettingScope::TuiOnly,
            // No Client acts on it: the row sets a flag nothing reads. A
            // separate bug owns the fix or removal.
            K::LogOut => SettingScope::Inert,
            K::Services
            | K::Keys
            | K::StayAlive
            | K::AutoReconnect
            | K::SavePlaylistOnQuit
            | K::AlwaysPlayNext
            | K::ConsumeVideos
            | K::ConsumeAudio
            | K::SavePlaylistOnConsume
            | K::SavePlaylistOnConsumeAudio
            | K::AlwaysSkipIntro
            | K::HiddenLibraries
            | K::ShowAudioWindow
            | K::UseMpvConfig
            | K::NoScripts
            | K::Autoload
            | K::ShowSysTrayIcon
            | K::SystemNotifications
            | K::MyLanguages
            | K::SubtitleMode
            | K::FeedViewLibraries
            | K::LibraryRoutes
            | K::SubtitleLanguage
            | K::AudioLanguage
            | K::ManageFeeds
            | K::ResetUiState => SettingScope::AllClients,
        }
    }
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
    RemoveEmby,
    SetupAudiobookshelf,
    TestAudiobookshelf,
    RemoveAudiobookshelf,
    ReplaceAudiobookshelf,
    ManageFeeds,
}

impl ServiceActionIntent {
    /// The action's label as a section paints it (`add-gui-settings-screen`
    /// design D9): the GUI's Services section offers one button per intent,
    /// so each intent names itself instead of sharing the TUI's one
    /// key-hint sentence.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SetupEmby => "Set up Emby",
            Self::RetryEmby => "Retry connection",
            Self::RepairEmby => "Repair",
            Self::RemoveEmby | Self::RemoveAudiobookshelf => "Remove",
            Self::SetupAudiobookshelf => "Set up Audiobookshelf",
            Self::TestAudiobookshelf => "Test connection",
            Self::ReplaceAudiobookshelf => "Replace",
            Self::ManageFeeds => "Manage feeds",
        }
    }
}

// Sections rendered as YELLOW blocks in a 2×2 grid.
// LogOut is rendered separately as a plain line below the grid.
pub static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
    ("Services", &[SettingKey::Services]),
    // Navigation-only entry (opens the read-only Keys destination); not a
    // config section, so `KeySection` has no `Keys` variant.
    ("Keys", &[SettingKey::Keys]),
    // Navigation-only entry for the pinned-panel page.
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

mod presentation;

pub use self::presentation::{
    SettingValueKind, bool_val, fmt_feed_view_list, fmt_feeds_list, fmt_hidden_list, fmt_lang_list,
    fmt_library_routes, setting_flag, setting_kind, setting_label, setting_value,
};

/// The Panel destination's eleven value rows, in paint and cursor order,
/// including the accent rows.
pub const PANEL_SETTING_KEYS: [SettingKey; 11] = [
    SettingKey::PanelSide,
    SettingKey::PanelWidth,
    SettingKey::PanelWidthExpanded,
    SettingKey::PanelReserve,
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

/// Panel-destination cursor ordinal to `SettingKey`.
#[must_use]
pub fn panel_cursor_to_key(cursor: usize) -> SettingKey {
    PANEL_SETTING_KEYS
        .get(cursor)
        .copied()
        .unwrap_or(SettingKey::PanelSide)
}

/// Whether a Panel row is one of the three accent rows: accent
/// rows are not part of the layout, never touch the running panel, and apply
/// on the next `mbv --pin` launch.
#[must_use]
pub fn is_panel_accent_row(key: SettingKey) -> bool {
    matches!(
        key,
        SettingKey::PanelAccent | SettingKey::PanelAccentColor | SettingKey::PanelAccentWidth
    )
}

/// Which of the two saved widths the running panel shows. The
/// active width is runtime state on `App`; it is never persisted, so every
/// pinned launch starts collapsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PinnedWidth {
    #[default]
    Collapsed,
    Expanded,
}

impl PinnedWidth {
    /// The other width.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Collapsed => Self::Expanded,
            Self::Expanded => Self::Collapsed,
        }
    }
}

/// Which pinned width the edited Panel row belongs to, or `None` for a row
/// that validates at the caller's current width: `Width` validates
/// `width`, and `Expanded width` validates `width_expanded`.
#[must_use]
pub fn panel_row_edited_width(key: SettingKey) -> Option<PinnedWidth> {
    match key {
        SettingKey::PanelWidth => Some(PinnedWidth::Collapsed),
        SettingKey::PanelWidthExpanded => Some(PinnedWidth::Expanded),
        _ => None,
    }
}

/// A Panel width row's step in logical pixels. The component
/// sends ±1, or ±10 while Shift is held, so a width row moves by 10 pixels
/// and by 100 with Shift.
const PANEL_WIDTH_STEP: i32 = 10;

/// The base step of one Panel stepper row in its own unit: a
/// width row steps by 10 logical pixels, every other row by 1 (gutter rows
/// keep 1, or 10 with Shift). The interaction's own multiplier still applies
/// on top, so a width row moves 100 pixels when Shift is held.
fn panel_row_step(key: SettingKey) -> i32 {
    match key {
        SettingKey::PanelWidth | SettingKey::PanelWidthExpanded => PANEL_WIDTH_STEP,
        _ => 1,
    }
}

/// Step a panel width in logical pixels by `delta`, clamped to
/// `PANEL_WIDTH_MIN..=u16::MAX` (a step past `u16::MAX` fails `try_from` and
/// saturates at the top).
fn stepped_width(width: u16, delta: i32) -> u16 {
    u16::try_from(
        i32::from(width)
            .saturating_add(delta)
            .max(i32::from(mbv_config::PANEL_WIDTH_MIN)),
    )
    .unwrap_or(u16::MAX)
}

/// Step the accent stroke's width by `delta`, clamped to `1..=65535`.
fn stepped_accent_width(width: std::num::NonZeroU16, delta: i32) -> std::num::NonZeroU16 {
    let next = i32::from(width.get())
        .saturating_add(delta)
        .clamp(1, i32::from(u16::MAX));
    // The clamp puts `next` in 1..=u16::MAX, so both conversions are total;
    // a failure would be a bug above, not a bad input to fall back on
    // (M-PANIC-ON-BUG).
    std::num::NonZeroU16::new(u16::try_from(next).expect("clamped to 1..=u16::MAX"))
        .expect("clamped to 1..=u16::MAX")
}

/// The palette the `Accent color` row cycles: sand (the configured default),
/// then frost, moss, ember and white.
const ACCENT_CYCLE: [mbv_config::PanelAccentColor; 5] = [
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR, // sand #dabc7f
    mbv_config::PanelAccentColor([0x7f, 0xc8, 0xda]), // frost #7fc8da
    mbv_config::PanelAccentColor([0xa3, 0xbe, 0x8c]), // moss #a3be8c
    mbv_config::PanelAccentColor([0xbf, 0x61, 0x6a]), // ember #bf616a
    mbv_config::PanelAccentColor([0xff, 0xff, 0xff]), // white #ffffff
];

/// `configured` when it lies outside the `Accent color` row's fixed palette.
/// The shell holds that value while Settings is open and passes it back to
/// [`changed_panel_config`], so cycling can return to it (issue #875).
#[must_use]
pub fn accent_color_outside_cycle(
    configured: mbv_config::PanelAccentColor,
) -> Option<mbv_config::PanelAccentColor> {
    (!ACCENT_CYCLE.contains(&configured)).then_some(configured)
}

/// The next `Accent color` value: the fixed palette followed by
/// the held custom colour, if any, then back to the palette's first. A
/// colour in neither steps to the palette's first.
fn next_accent_color(
    configured: mbv_config::PanelAccentColor,
    custom: Option<mbv_config::PanelAccentColor>,
) -> mbv_config::PanelAccentColor {
    let cycle: Vec<_> = ACCENT_CYCLE.iter().copied().chain(custom).collect();
    let next = cycle
        .iter()
        .position(|candidate| *candidate == configured)
        .map_or(0, |index| (index + 1) % cycle.len());
    cycle[next]
}

/// Apply one Panel-row interaction: `Side` toggles
/// left/right, and each numeric row steps by `delta` times its base step
/// (`panel_row_step`) clamped to its Rust range. `delta` is already scaled
/// to ±1/±10 by the caller for Shift. `None` for any key that is not a Panel
/// value row. `accent_custom` is the colour the shell holds from
/// [`accent_color_outside_cycle`]; it joins the `Accent color` cycle.
#[must_use]
pub fn changed_panel_config(
    key: SettingKey,
    panel: mbv_config::PanelConfig,
    delta: i32,
    accent_custom: Option<mbv_config::PanelAccentColor>,
) -> Option<mbv_config::PanelConfig> {
    use mbv_config::PanelSide;

    let delta = delta.saturating_mul(panel_row_step(key));
    let mut next = panel;
    match key {
        SettingKey::PanelSide => {
            next.side = match next.side {
                PanelSide::Left => PanelSide::Right,
                PanelSide::Right => PanelSide::Left,
            };
        }
        SettingKey::PanelWidth => next.width = stepped_width(next.width, delta),
        SettingKey::PanelWidthExpanded => {
            next.width_expanded = stepped_width(next.width_expanded, delta);
        }
        SettingKey::PanelGutterTop => next.gutter_top = next.gutter_top.saturating_add(delta),
        SettingKey::PanelGutterBottom => {
            next.gutter_bottom = next.gutter_bottom.saturating_add(delta);
        }
        SettingKey::PanelGutterLeft => {
            next.gutter_left = next.gutter_left.saturating_add(delta);
        }
        SettingKey::PanelGutterRight => {
            next.gutter_right = next.gutter_right.saturating_add(delta);
        }
        SettingKey::PanelAccent => next.accent = !next.accent,
        SettingKey::PanelReserve => next.reserve = !next.reserve,
        SettingKey::PanelAccentColor => {
            next.accent_color = next_accent_color(next.accent_color, accent_custom);
        }
        SettingKey::PanelAccentWidth => {
            next.accent_width = stepped_accent_width(next.accent_width, delta);
        }
        _ => return None,
    }
    Some(next)
}

#[cfg(test)]
mod tests;
