//! The GUI Settings screen's Client-core state (`add-gui-settings-screen`
//! design D2, D3): whether the screen is open, the section it shows, the
//! sections the sidebar lists, and the open actions' toggle rule. The window
//! content owns the screen's interaction state (design D1, ADR 0037); this is
//! the core half the drain (`App::pending_overlay`'s GUI half) drives and the
//! frame projects. The TUI Client never sets any of it — its Settings sidebar
//! keeps its own shell state.
//!
//! The bridge's half lives in the children: `command` holds the semantic
//! requests the window content sends (design D5), and `projection` the
//! section content the frame carries (design D9).

mod command;
mod feeds;
mod keys;
mod playlists;
mod projection;
mod reset;
mod sections;
mod services;

pub use self::command::{GuiSettingsCommand, GuiSetupService, SetupSecret};
pub use self::projection::{
    GuiSettingsChoiceProjection, GuiSettingsFeedProjection, GuiSettingsFeedsProjection,
    GuiSettingsKeyRefusalProjection, GuiSettingsKeyRowProjection, GuiSettingsKeysProjection,
    GuiSettingsLibraryProjection, GuiSettingsOpenPlaylistProjection,
    GuiSettingsPlaylistItemProjection, GuiSettingsPlaylistRowProjection,
    GuiSettingsPlaylistsProjection, GuiSettingsRouteDeviceProjection, GuiSettingsRouteProjection,
    GuiSettingsRowKind, GuiSettingsRowProjection, GuiSettingsServiceProjection,
    GuiSettingsServicesProjection, GuiSettingsSetupProjection,
};

use mbv_ui_model::context_menu::MultiSelectKind;
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

/// The open Settings screen's core state (design D2, D9): the section it
/// shows and the section data the frame reads. The window content owns the
/// screen's interaction state (design D1, ADR 0037); everything here is the
/// core's half, and the caches clear when the screen closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsState {
    /// The section the screen shows.
    pub section: GuiSettingsSection,
    /// The loaded choice lists, one entry per kind the sections opened so far
    /// asked for (design D9). A row whose kind is absent paints no choices
    /// yet.
    pub choices: Vec<(MultiSelectKind, Vec<GuiSettingsChoiceProjection>)>,
    /// The Library section's loaded route rows (design D9), `None` until its
    /// fetch lands and after it fails.
    pub routes: Option<Vec<GuiSettingsRouteProjection>>,
    /// The library the Library section selected, as the lowercased name
    /// `routeDevices` carried (design D9), `None` before the first selection.
    pub route_selection: Option<String>,
    /// The selected library's candidate devices (design D9), `None` until a
    /// `routeDevices` fetch for the selection lands and after one fails.
    pub route_devices: Option<Vec<GuiSettingsRouteDeviceProjection>>,
    /// The last refused Keys edit (design D9): the action's id and the refusal
    /// text, kept until an accepted edit replaces it or the screen closes.
    pub refusal: Option<GuiSettingsKeyRefusalProjection>,
}

impl GuiSettingsState {
    /// The state an open at `section` starts from: no section load has landed
    /// yet.
    fn at(section: GuiSettingsSection) -> Self {
        Self {
            section,
            choices: Vec::new(),
            routes: None,
            route_selection: None,
            route_devices: None,
            refusal: None,
        }
    }

    /// The loaded choices of `kind`, `None` while the list has not loaded
    /// (design D9).
    #[must_use]
    pub fn choices(&self, kind: MultiSelectKind) -> Option<&[GuiSettingsChoiceProjection]> {
        self.choices
            .iter()
            .find(|(loaded, _)| *loaded == kind)
            .map(|(_, choices)| choices.as_slice())
    }

    /// Store one loaded choice list, replacing an earlier load of the same
    /// kind.
    fn set_choices(&mut self, kind: MultiSelectKind, choices: Vec<GuiSettingsChoiceProjection>) {
        match self.choices.iter_mut().find(|(loaded, _)| *loaded == kind) {
            Some((_, stored)) => *stored = choices,
            None => self.choices.push((kind, choices)),
        }
    }
}

impl App {
    /// The open Settings screen's state, `None` while the screen is closed
    /// (design D2).
    #[must_use]
    pub fn gui_settings(&self) -> Option<GuiSettingsState> {
        self.gui_settings.clone()
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
        match self.gui_settings.as_ref() {
            Some(state) if group.contains(state.section) => self.close_gui_settings(),
            Some(_) => self.show_gui_settings_section(group.section()),
            None => self.open_gui_settings_at(group.section()),
        }
    }

    /// Open the screen at `section` (design D2): the `OpenSidebar(Settings)`
    /// drain arm's open, and the open half of the toggle rule. The section's
    /// load starts here (design D9).
    pub fn open_gui_settings_at(&mut self, section: GuiSettingsSection) {
        self.gui_settings = Some(GuiSettingsState::at(section));
        self.load_gui_settings_section(section);
    }

    /// Close the screen (design D2): the state clears — the section caches
    /// with it (design D9) — and the core's focus sync pops
    /// `ComponentId::Overlay(OverlayId::Settings)` so the Panel focus returns.
    pub fn close_gui_settings(&mut self) {
        self.gui_settings = None;
    }

    /// Show a section of the open screen and start its load (design D5's
    /// `selectSection`, design D9). A command whose screen is closed changes
    /// nothing: the frame it rode is stale.
    fn select_gui_settings_section(&mut self, section: GuiSettingsSection) {
        if self.gui_settings_is_open() {
            self.show_gui_settings_section(section);
        } else {
            Self::log_ignored_gui_settings_command(
                "selectSection",
                "the Settings screen is closed",
            );
        }
    }

    /// Switch the open screen to `section`, keeping the caches the screen owns
    /// and starting the new section's load (design D9).
    fn show_gui_settings_section(&mut self, section: GuiSettingsSection) {
        if let Some(state) = self.gui_settings.as_mut() {
            state.section = section;
        }
        self.load_gui_settings_section(section);
    }

    /// The UI-reset counter the frame carries (design D10): the front end
    /// resets its interaction stores when the value changes. Row 5.6 raises
    /// it.
    #[must_use]
    pub fn gui_ui_reset_epoch(&self) -> u64 {
        self.gui_ui_reset_epoch
    }

    /// Log the Settings command whose frame is stale: the screen it addresses
    /// is closed, so the command changes nothing.
    fn log_ignored_gui_settings_command(command: &str, reason: impl std::fmt::Display) {
        tracing::debug!(
            name: "gui_settings.command.ignored",
            target: "gui_settings",
            command,
            reason = %reason,
            "the Settings command changed nothing"
        );
    }
}
