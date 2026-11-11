//! The Services section's frame content (`add-gui-settings-screen` design D9):
//! each Service's state and the actions it offers, and the open setup form's
//! progress and error. No field carries a password, an API key, or any other
//! Service credential — the setup form carries the server URL only.

use mbv::GuiSettingsServiceProjection;
use serde::{Deserialize, Serialize};

/// The Services section's content (design D9).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsService {
    pub entry: GuiServiceEntry,
    pub name: String,
    /// The Service's state label, as the TUI paints it.
    pub state: String,
    /// The Service's context line (the feed subscription count, or empty).
    pub context: String,
    /// The actions the TUI offers for the Service's current state, each with
    /// the intent the `serviceAction` command sends.
    pub actions: Vec<GuiServiceAction>,
}

impl From<&GuiSettingsServiceProjection> for GuiSettingsService {
    fn from(service: &GuiSettingsServiceProjection) -> Self {
        Self {
            entry: GuiServiceEntry::from(service.entry),
            name: service.name.to_string(),
            state: service.state.to_string(),
            context: service.context.clone(),
            actions: service
                .actions
                .iter()
                .map(|(intent, label)| GuiServiceAction {
                    intent: GuiServiceActionIntent::from(*intent),
                    label: (*label).to_string(),
                })
                .collect(),
        }
    }
}

/// The three Services (design D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiServiceEntry {
    Emby,
    Audiobookshelf,
    Feeds,
}

impl From<mbv_ui_model::settings::ServiceEntry> for GuiServiceEntry {
    fn from(entry: mbv_ui_model::settings::ServiceEntry) -> Self {
        match entry {
            mbv_ui_model::settings::ServiceEntry::Emby => Self::Emby,
            mbv_ui_model::settings::ServiceEntry::Audiobookshelf => Self::Audiobookshelf,
            mbv_ui_model::settings::ServiceEntry::Feeds => Self::Feeds,
        }
    }
}

/// One action a Service row offers (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiServiceAction {
    /// The intent the `serviceAction` command carries.
    pub intent: GuiServiceActionIntent,
    pub label: String,
}

/// One Service action the window content can run (design D5): the wire names
/// of `mbv_ui_model::settings::ServiceActionIntent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiServiceActionIntent {
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

impl From<mbv_ui_model::settings::ServiceActionIntent> for GuiServiceActionIntent {
    fn from(intent: mbv_ui_model::settings::ServiceActionIntent) -> Self {
        match intent {
            mbv_ui_model::settings::ServiceActionIntent::SetupEmby => Self::SetupEmby,
            mbv_ui_model::settings::ServiceActionIntent::RetryEmby => Self::RetryEmby,
            mbv_ui_model::settings::ServiceActionIntent::RepairEmby => Self::RepairEmby,
            mbv_ui_model::settings::ServiceActionIntent::RemoveEmby => Self::RemoveEmby,
            mbv_ui_model::settings::ServiceActionIntent::SetupAudiobookshelf => {
                Self::SetupAudiobookshelf
            }
            mbv_ui_model::settings::ServiceActionIntent::TestAudiobookshelf => {
                Self::TestAudiobookshelf
            }
            mbv_ui_model::settings::ServiceActionIntent::RemoveAudiobookshelf => {
                Self::RemoveAudiobookshelf
            }
            mbv_ui_model::settings::ServiceActionIntent::ReplaceAudiobookshelf => {
                Self::ReplaceAudiobookshelf
            }
            mbv_ui_model::settings::ServiceActionIntent::ManageFeeds => Self::ManageFeeds,
        }
    }
}

impl From<GuiServiceActionIntent> for mbv_ui_model::settings::ServiceActionIntent {
    fn from(intent: GuiServiceActionIntent) -> Self {
        match intent {
            GuiServiceActionIntent::SetupEmby => Self::SetupEmby,
            GuiServiceActionIntent::RetryEmby => Self::RetryEmby,
            GuiServiceActionIntent::RepairEmby => Self::RepairEmby,
            GuiServiceActionIntent::RemoveEmby => Self::RemoveEmby,
            GuiServiceActionIntent::SetupAudiobookshelf => Self::SetupAudiobookshelf,
            GuiServiceActionIntent::TestAudiobookshelf => Self::TestAudiobookshelf,
            GuiServiceActionIntent::RemoveAudiobookshelf => Self::RemoveAudiobookshelf,
            GuiServiceActionIntent::ReplaceAudiobookshelf => Self::ReplaceAudiobookshelf,
            GuiServiceActionIntent::ManageFeeds => Self::ManageFeeds,
        }
    }
}

/// The open setup form's facts (design D9): the service it belongs to, the
/// server URL, and its progress and error. The typed password and API key stay
/// in the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsSetup {
    pub service: GuiSetupService,
    pub server_url: String,
    pub busy: bool,
    pub error: String,
}

impl From<&mbv::GuiSettingsSetupProjection> for GuiSettingsSetup {
    fn from(setup: &mbv::GuiSettingsSetupProjection) -> Self {
        Self {
            service: GuiSetupService::from(setup.service),
            server_url: setup.server_url.clone(),
            busy: setup.busy,
            error: setup.error.clone(),
        }
    }
}

/// The setup form the `cancelSetup` command names (design D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiSetupService {
    Emby,
    Audiobookshelf,
}

impl From<mbv::GuiSetupService> for GuiSetupService {
    fn from(service: mbv::GuiSetupService) -> Self {
        match service {
            mbv::GuiSetupService::Emby => Self::Emby,
            mbv::GuiSetupService::Audiobookshelf => Self::Audiobookshelf,
        }
    }
}

impl From<GuiSetupService> for mbv::GuiSetupService {
    fn from(service: GuiSetupService) -> Self {
        match service {
            GuiSetupService::Emby => Self::Emby,
            GuiSetupService::Audiobookshelf => Self::Audiobookshelf,
        }
    }
}
