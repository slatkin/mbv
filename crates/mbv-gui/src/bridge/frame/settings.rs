//! The Settings screen's frame section (`add-gui-settings-screen` design D9):
//! the open section's identity, the sidebar's shown sections, and the content
//! of the section the user has open. The settings section's value is `None`
//! while the screen is closed, so the window mounts it only for a `Some` value.
//!
//! The content is a tagged union with one variant per section kind: the F2
//! rows, Library, Services, Feeds, Keys, and Sessions. No variant
//! carries a password, an API key, or any other Service credential.

pub mod feeds;
pub mod keys;
pub mod library;
pub mod rows;
pub mod services;
pub mod sessions;

use mbv::{GuiCore, GuiSettingsSection as CoreSettingsSection, QueuePlaybackProjection};

use self::feeds::GuiSettingsFeed;
use self::keys::{GuiSettingsKeyRefusal, GuiSettingsKeyRow};
use self::library::{GuiSettingsRoute, GuiSettingsRouteDevice};
use self::rows::GuiSettingRow;
use self::services::{GuiSettingsService, GuiSettingsSetup};
use crate::bridge::frame::playback::GuiTargetChoice;

/// The Settings screen's section value (design D9). `None` while the screen is
/// closed.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiSettingsFrame {
    /// The section the window shows.
    pub section: GuiSettingsSection,
    /// The sections the sidebar lists, in order (design D3): a section whose
    /// every row the GUI hides is absent.
    pub sections: Vec<GuiSettingsSection>,
    /// The shown section's content.
    pub content: GuiSettingsContent,
}

impl GuiSettingsFrame {
    /// The section value the App's open Settings screen projects, `None` while
    /// the screen is closed (design D9). The Sessions section reads the
    /// playback projection the same tick already read, so the Settings
    /// chooser's targets and the header's chooser targets are one read.
    #[must_use]
    pub fn build(core: &GuiCore, playback: &QueuePlaybackProjection) -> Option<GuiSettingsFrame> {
        let state = core.gui_settings()?;
        Some(Self {
            section: state.section.into(),
            sections: CoreSettingsSection::shown_sections()
                .into_iter()
                .map(GuiSettingsSection::from)
                .collect(),
            content: GuiSettingsContent::for_section(core, playback, state.section),
        })
    }
}

/// One Settings section (design D3), in sidebar order. The wire names are the
/// section names the window content sends back in `selectSection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl From<CoreSettingsSection> for GuiSettingsSection {
    fn from(section: CoreSettingsSection) -> Self {
        match section {
            CoreSettingsSection::Services => Self::Services,
            CoreSettingsSection::Playback => Self::Playback,
            CoreSettingsSection::Display => Self::Display,
            CoreSettingsSection::Session => Self::Session,
            CoreSettingsSection::Library => Self::Library,
            CoreSettingsSection::Queue => Self::Queue,
            CoreSettingsSection::Mpv => Self::Mpv,
            CoreSettingsSection::Feeds => Self::Feeds,
            CoreSettingsSection::Actions => Self::Actions,
            CoreSettingsSection::Keys => Self::Keys,
            CoreSettingsSection::Sessions => Self::Sessions,
        }
    }
}

impl From<GuiSettingsSection> for CoreSettingsSection {
    fn from(section: GuiSettingsSection) -> Self {
        match section {
            GuiSettingsSection::Services => Self::Services,
            GuiSettingsSection::Playback => Self::Playback,
            GuiSettingsSection::Display => Self::Display,
            GuiSettingsSection::Session => Self::Session,
            GuiSettingsSection::Library => Self::Library,
            GuiSettingsSection::Queue => Self::Queue,
            GuiSettingsSection::Mpv => Self::Mpv,
            GuiSettingsSection::Feeds => Self::Feeds,
            GuiSettingsSection::Actions => Self::Actions,
            GuiSettingsSection::Keys => Self::Keys,
            GuiSettingsSection::Sessions => Self::Sessions,
        }
    }
}

/// The shown section's content (design D9): one variant per kind of section.
#[derive(Debug, Clone, PartialEq)]
pub enum GuiSettingsContent {
    /// The F2 rows of a settings page (Playback, Display, Session, Queue,
    /// Mpv, Actions).
    Rows { rows: Vec<GuiSettingRow> },
    /// The Library section: its choice rows, the routable libraries, the
    /// selected library, and that library's candidate devices (design D9).
    Library {
        rows: Vec<GuiSettingRow>,
        routes: Option<Vec<GuiSettingsRoute>>,
        selected: Option<String>,
        devices: Option<Vec<GuiSettingsRouteDevice>>,
    },
    /// The three Services, their actions, and the open setup form.
    Services {
        services: Vec<GuiSettingsService>,
        setup: Option<GuiSettingsSetup>,
    },
    /// The feed subscriptions and whether an add is in flight.
    Feeds {
        subscriptions: Vec<GuiSettingsFeed>,
        adding: bool,
    },
    /// The prefix chord, the `AllClients` actions, and the last refusal.
    Keys {
        prefix: Option<String>,
        rows: Vec<GuiSettingsKeyRow>,
        refusal: Option<GuiSettingsKeyRefusal>,
    },
    /// The playback targets the chooser lists, and whether a refresh of them
    /// is in flight.
    Sessions {
        targets: Vec<GuiTargetChoice>,
        refreshing: bool,
    },
}

impl GuiSettingsContent {
    /// The content the section shows (design D9). The F2 pages carry their
    /// rows; the five sections with their own data carry theirs, and Sessions
    /// reads the tick's one playback projection.
    fn for_section(
        core: &GuiCore,
        playback: &QueuePlaybackProjection,
        section: CoreSettingsSection,
    ) -> Self {
        match section {
            CoreSettingsSection::Services => Self::services(core),
            CoreSettingsSection::Library => Self::library(core),
            CoreSettingsSection::Feeds => Self::feeds(core),
            CoreSettingsSection::Keys => Self::keys(core),
            CoreSettingsSection::Sessions => Self::Sessions {
                targets: sessions::targets(&playback.target_choices),
                refreshing: core.gui_playback_target_refresh_busy(),
            },
            CoreSettingsSection::Playback
            | CoreSettingsSection::Display
            | CoreSettingsSection::Session
            | CoreSettingsSection::Queue
            | CoreSettingsSection::Mpv
            | CoreSettingsSection::Actions => Self::Rows {
                rows: core
                    .gui_settings_rows()
                    .iter()
                    .map(GuiSettingRow::from)
                    .collect(),
            },
        }
    }

    /// The Library section's content (design D9): the choice rows, the
    /// routable libraries, the selected library, and its candidate devices.
    fn library(core: &GuiCore) -> Self {
        let projection = core.gui_settings_library();
        Self::Library {
            rows: projection.rows.iter().map(GuiSettingRow::from).collect(),
            routes: projection
                .routes
                .as_ref()
                .map(|routes| routes.iter().map(GuiSettingsRoute::from).collect()),
            selected: projection.selected.clone(),
            devices: projection
                .devices
                .as_ref()
                .map(|devices| devices.iter().map(GuiSettingsRouteDevice::from).collect()),
        }
    }

    /// The Services section's content (design D9).
    fn services(core: &GuiCore) -> Self {
        let projection = core.gui_settings_services();
        Self::Services {
            services: projection
                .services
                .iter()
                .map(GuiSettingsService::from)
                .collect(),
            setup: projection.setup.as_ref().map(GuiSettingsSetup::from),
        }
    }

    /// The Feeds section's content (design D9).
    fn feeds(core: &GuiCore) -> Self {
        let projection = core.gui_settings_feeds();
        Self::Feeds {
            subscriptions: projection
                .subscriptions
                .iter()
                .map(GuiSettingsFeed::from)
                .collect(),
            adding: projection.adding,
        }
    }

    /// The Keys section's content (design D9).
    fn keys(core: &GuiCore) -> Self {
        let projection = core.gui_settings_keys();
        Self::Keys {
            prefix: projection.prefix.clone(),
            rows: projection
                .rows
                .iter()
                .map(GuiSettingsKeyRow::from)
                .collect(),
            refusal: projection.refusal.as_ref().map(GuiSettingsKeyRefusal::from),
        }
    }
}
