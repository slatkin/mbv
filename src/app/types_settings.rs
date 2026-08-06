#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum PanelFocus {
    Queue, // queue column (queue list below the card)
    #[default]
    Library, // library side (library browser); driven by library_tab
}

impl PanelFocus {
    pub(super) fn from_pref(value: Option<&str>) -> Self {
        match value {
            Some("queue_side") => Self::Queue,
            Some("library_side") => Self::Library,
            _ => Self::default(),
        }
    }

    pub(super) fn pref_value(self) -> &'static str {
        match self {
            Self::Queue => "queue_side",
            Self::Library => "library_side",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum PanelMode {
    #[default]
    Both,        // both panels: card+queue left, library right
    LibraryOnly, // left (queue) column hidden; library spans full window
    QueueOnly,   // right (library) column hidden; queue spans full window
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingKey {
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
    HiddenLatest,
    ShowAudioWindow,
    UseMpvConfig,
    NoScripts,
    Autoload,
    ShowSysTrayIcon,
    SystemNotifications,
    MyLanguages,
    SubtitleMode,
    FeedViewLibraries,
    LibraryRoutes,
    SubtitleLanguage,
    AudioLanguage,
    LogOut,
}

// Sections rendered as YELLOW blocks in a 2×2 grid.
// LogOut is rendered separately as a plain line below the grid.
pub(super) static SETTING_SECTIONS: &[(&str, &[SettingKey])] = &[
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
        &[SettingKey::ImageProtocol, SettingKey::SystemNotifications],
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
            SettingKey::HiddenLatest,
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
    ("Actions", &[SettingKey::LogOut]),
];
