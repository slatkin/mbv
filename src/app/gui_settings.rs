//! The GUI Settings screen's Client-core state (`add-gui-settings-screen`
//! design D2, D3): whether the screen is open, the section it shows, the
//! sections the sidebar lists, and the open actions' toggle rule. The window
//! content owns the screen's interaction state (design D1, ADR 0037); this is
//! the core half the drain (`App::pending_overlay`'s GUI half) drives and the
//! frame projects. The TUI Client never sets any of it — its Settings sidebar
//! keeps its own shell state.

use mbv_ui_model::settings::{SETTING_SECTIONS, SettingKey, SettingScope, SettingsDestination};

use super::App;

/// The one Settings screen's sections (design D3): the closed list in the
/// spec's sidebar order. `Sessions` and `Playlists` are the GUI's own sidebar
/// sections; the rest map one-to-one onto the F2 sections of
/// `SETTING_SECTIONS` by name, with the TUI's `Panel` page absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiSettingsSection {
    Services,
    Playback,
    Display,
    Session,
    Library,
    Queue,
    Mpv,
    Feeds,
    Actions,
    Keys,
    Sessions,
    Playlists,
}

impl GuiSettingsSection {
    /// The closed sidebar order (the spec's sidebar-list requirement).
    pub const ALL: [GuiSettingsSection; 12] = [
        Self::Services,
        Self::Playback,
        Self::Display,
        Self::Session,
        Self::Library,
        Self::Queue,
        Self::Mpv,
        Self::Feeds,
        Self::Actions,
        Self::Keys,
        Self::Sessions,
        Self::Playlists,
    ];

    /// The `SETTING_SECTIONS` entry the section draws its F2 rows from.
    /// `Sessions` and `Playlists` carry no F2 rows: their content is the
    /// playback-target chooser and the playlist list (design D3).
    fn setting_section(self) -> Option<&'static str> {
        match self {
            Self::Services => Some("Services"),
            Self::Playback => Some("Playback"),
            Self::Display => Some("Display"),
            Self::Session => Some("Session"),
            Self::Library => Some("Library"),
            Self::Queue => Some("Queue"),
            Self::Mpv => Some("Mpv"),
            Self::Feeds => Some("Feeds"),
            Self::Actions => Some("Actions"),
            Self::Keys => Some("Keys"),
            Self::Sessions | Self::Playlists => None,
        }
    }

    /// The F2 rows the section shows (design D4): the `SETTING_SECTIONS` rows
    /// of the section's name, filtered to the keys whose scope is
    /// `AllClients`. Empty for `Sessions` and `Playlists`.
    #[must_use]
    pub fn shown_rows(self) -> Vec<SettingKey> {
        let Some(name) = self.setting_section() else {
            return Vec::new();
        };
        SETTING_SECTIONS
            .iter()
            .find(|(section, _)| *section == name)
            .map(|(_, keys)| {
                keys.iter()
                    .copied()
                    .filter(|key| key.scope() == SettingScope::AllClients)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The sections the sidebar lists (design D3): the closed order, without
    /// a section whose every F2 row the GUI hides.
    #[must_use]
    pub fn shown_sections() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|section| {
                section
                    .setting_section()
                    .is_none_or(|_| !section.shown_rows().is_empty())
            })
            .collect()
    }
}

/// The open actions' groups (`add-gui-settings-screen` design D2): the
/// sections one open action's toggle rule compares. `F2` is the group the
/// `settings_open` action toggles — every section except `Sessions` and
/// `Playlists` (the spec's open-action rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiSettingsGroup {
    F2,
    Sessions,
    Playlists,
}

impl GuiSettingsGroup {
    /// The section the group's open action opens at when the screen is
    /// closed: `settings_open` at Services, the other two at their own
    /// sections (the spec's open-action requirement).
    #[must_use]
    pub fn section(self) -> GuiSettingsSection {
        match self {
            Self::F2 => GuiSettingsSection::Services,
            Self::Sessions => GuiSettingsSection::Sessions,
            Self::Playlists => GuiSettingsSection::Playlists,
        }
    }

    /// Whether the shown section belongs to the group — the "same group
    /// closes" half of the toggle rule (design D2).
    #[must_use]
    pub fn contains(self, section: GuiSettingsSection) -> bool {
        match self {
            Self::F2 => !matches!(
                section,
                GuiSettingsSection::Sessions | GuiSettingsSection::Playlists
            ),
            Self::Sessions => section == GuiSettingsSection::Sessions,
            Self::Playlists => section == GuiSettingsSection::Playlists,
        }
    }
}

/// The open Settings screen's core state (design D2): the section it shows.
/// The per-section caches of design D9 join as the frame rows land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiSettingsState {
    /// The section the screen shows.
    pub section: GuiSettingsSection,
}

impl App {
    /// The open Settings screen's state, `None` while the screen is closed
    /// (design D2).
    #[must_use]
    pub fn gui_settings(&self) -> Option<GuiSettingsState> {
        self.gui_settings
    }

    /// Whether the Settings screen is open (design D2): the fact the router
    /// snapshot's queue-scoped gates read and the core's focus sync mirrors.
    #[must_use]
    pub fn gui_settings_is_open(&self) -> bool {
        self.gui_settings.is_some()
    }

    /// The Settings destination the App-side openers recorded (design D2):
    /// the `OpenSidebar(Settings)` drain arm reads it to pick the section.
    /// The TUI shell reads the same field for its own sidebar.
    #[must_use]
    pub fn gui_settings_destination(&self) -> SettingsDestination {
        self.settings_destination
    }

    /// The toggle rule of design D2: with the screen closed, open it at the
    /// group's section; with the screen showing a section of the same group,
    /// close it; otherwise show the group's section and stay open.
    pub fn toggle_gui_settings(&mut self, group: GuiSettingsGroup) {
        match self.gui_settings {
            Some(state) if group.contains(state.section) => self.close_gui_settings(),
            Some(mut state) => {
                state.section = group.section();
                self.gui_settings = Some(state);
            }
            None => self.open_gui_settings_at(group.section()),
        }
    }

    /// Open the screen at `section` (design D2): the `OpenSidebar(Settings)`
    /// drain arm's open, and the open half of the toggle rule.
    pub fn open_gui_settings_at(&mut self, section: GuiSettingsSection) {
        self.gui_settings = Some(GuiSettingsState { section });
    }

    /// Close the screen (design D2): the state clears, and the core's focus
    /// sync pops `ComponentId::Overlay(OverlayId::Settings)` so the Panel
    /// focus returns.
    pub fn close_gui_settings(&mut self) {
        self.gui_settings = None;
    }
}
