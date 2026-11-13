//! The `settings` command's wire form (`add-gui-settings-screen` design D5):
//! the Settings screen's window content sends one semantic request, and the
//! neutral [`mbv::GuiSettingsCommand`] this module builds from it is what the
//! Client core applies. Identities are stable — a setting name, a playlist id,
//! a feed URL, a chord — never row positions, so a command built from a stale
//! window addresses what that window painted or nothing.

use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::settings::SettingKey;
use mbv_ui_msg::MusicTreeAction;
use serde::{Deserialize, Serialize};

use crate::bridge::frame::playback::{GuiTargetKey, chooser_target_key};
use crate::bridge::frame::settings::GuiSettingsSection;
use crate::bridge::frame::settings::services::{GuiServiceActionIntent, GuiSetupService};
use crate::bridge::keys::{DomKey, neutral_key};

/// One request from the Settings screen's window content (design D5). The
/// variants and their field names are the wire vocabulary the front end sends;
/// the translation into [`mbv::GuiSettingsCommand`] is the only place the two
/// shapes meet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GuiSettingsCommand {
    /// Open the screen at Services, as the `settings_open` action does, and
    /// change nothing while it is open (design D5's `open`, D11
    /// *Mouse-centric*). The Library top bar's Settings control sends it.
    Open,
    /// Close the screen.
    Close,
    /// Show a section, and start its load.
    SelectSection { section: GuiSettingsSection },
    /// Activate an F2 row: `setting` is the row's stable setting name.
    Activate { setting: String },
    /// Pick one value for a value setting: `setting` is the row's stable
    /// setting name and `value` one entry of the row's projected list.
    SetValue { setting: String, value: String },
    /// Apply a choice list's marked ids.
    ApplyChoices {
        kind: GuiChoicesKind,
        selected: Vec<String>,
    },
    /// Assign a library route, or clear it when `endpoint` is absent.
    AssignRoute {
        library: String,
        endpoint: Option<String>,
    },
    /// Load the candidate devices of `library` for the Library section's
    /// routes list.
    RouteDevices { library: String },
    /// Fill the Emby setup form and submit it.
    EmbySetup {
        server_url: String,
        username: String,
        password: String,
    },
    /// Fill the Audiobookshelf setup form and submit it.
    AudiobookshelfSetup { server_url: String, api_key: String },
    /// Run one Service action.
    ServiceAction { action: GuiServiceActionIntent },
    /// Cancel the named setup form.
    CancelSetup { service: GuiSetupService },
    /// Start a feed add.
    FeedAdd {
        name: String,
        url: String,
        kind: mbv_queue::FeedKind,
    },
    /// Edit a subscription, addressed by its URL.
    FeedEdit {
        url: String,
        name: String,
        kind: mbv_queue::FeedKind,
    },
    /// Raise the remove confirmation for a subscription.
    FeedRemove { url: String },
    /// Capture a key press for a binding target: the target's stable name and
    /// the raw DOM key the front end read.
    KeysCapture { target: String, key: DomKey },
    /// Reset one action's binding to its declared default.
    KeysReset { action: String },
    /// Clear one prefix-namespace target.
    KeysClear { target: String },
    /// Select a playback target.
    SelectTarget { target: GuiTargetKey },
    /// Open a playlist, or the list when `id` is absent.
    PlaylistOpen { id: Option<String> },
    /// Return from the open playlist to the list.
    PlaylistBack,
    /// Refresh the open playlist, or the one `id` names.
    PlaylistRefresh { id: Option<String> },
    /// Play, shuffle, or enqueue a whole playlist.
    PlaylistAction {
        id: String,
        action: GuiPlaylistAction,
    },
    /// Play, shuffle, or enqueue one item of the open playlist.
    PlaylistItemAction {
        id: String,
        item_id: String,
        action: GuiPlaylistAction,
    },
    /// Rename a playlist.
    PlaylistRename { id: String, name: String },
    /// Raise the delete confirmation for a playlist.
    PlaylistDelete { id: String },
    /// Reset the GUI's UI state.
    ResetUiState,
}

/// One choice-list kind the `applyChoices` command names (design D5): the
/// three lists the Settings screen marks. Each projected choice-list row
/// carries its kind (design D11 *Choice kind*), and the command sends it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiChoicesKind {
    HiddenLibraries,
    FeedViewLibraries,
    MyLanguages,
}

impl From<MultiSelectKind> for GuiChoicesKind {
    fn from(kind: MultiSelectKind) -> Self {
        match kind {
            MultiSelectKind::HiddenLibraries => Self::HiddenLibraries,
            MultiSelectKind::FeedViewLibraries => Self::FeedViewLibraries,
            MultiSelectKind::MyLanguages => Self::MyLanguages,
        }
    }
}

impl From<GuiChoicesKind> for MultiSelectKind {
    fn from(kind: GuiChoicesKind) -> Self {
        match kind {
            GuiChoicesKind::HiddenLibraries => Self::HiddenLibraries,
            GuiChoicesKind::FeedViewLibraries => Self::FeedViewLibraries,
            GuiChoicesKind::MyLanguages => Self::MyLanguages,
        }
    }
}

/// The playlist operation a playlist command carries (design D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiPlaylistAction {
    Play,
    Shuffle,
    Enqueue,
}

impl From<GuiPlaylistAction> for MusicTreeAction {
    fn from(action: GuiPlaylistAction) -> Self {
        match action {
            GuiPlaylistAction::Play => Self::Play,
            GuiPlaylistAction::Shuffle => Self::Shuffle,
            GuiPlaylistAction::Enqueue => Self::Enqueue,
        }
    }
}

/// Translate the wire form into the neutral command (design D5). `None` for a
/// request this build cannot address — a setting name no `SettingKey`
/// declares — so a stale window's edit reaches nothing.
#[must_use]
pub fn neutral(command: GuiSettingsCommand) -> Option<mbv::GuiSettingsCommand> {
    Some(match command {
        GuiSettingsCommand::Open => mbv::GuiSettingsCommand::Open,
        GuiSettingsCommand::Close => mbv::GuiSettingsCommand::Close,
        GuiSettingsCommand::SelectSection { section } => mbv::GuiSettingsCommand::SelectSection {
            section: section.into(),
        },
        GuiSettingsCommand::Activate { setting } => mbv::GuiSettingsCommand::Activate {
            setting: SettingKey::from_name(&setting)?,
        },
        GuiSettingsCommand::SetValue { setting, value } => mbv::GuiSettingsCommand::SetValue {
            setting: SettingKey::from_name(&setting)?,
            value,
        },
        GuiSettingsCommand::ApplyChoices { kind, selected } => {
            mbv::GuiSettingsCommand::ApplyChoices {
                kind: kind.into(),
                selected,
            }
        }
        GuiSettingsCommand::AssignRoute { library, endpoint } => {
            mbv::GuiSettingsCommand::AssignRoute { library, endpoint }
        }
        GuiSettingsCommand::RouteDevices { library } => {
            mbv::GuiSettingsCommand::RouteDevices { library }
        }
        GuiSettingsCommand::EmbySetup {
            server_url,
            username,
            password,
        } => mbv::GuiSettingsCommand::EmbySetup {
            server_url,
            username,
            password: mbv::SetupSecret::new(password),
        },
        GuiSettingsCommand::AudiobookshelfSetup {
            server_url,
            api_key,
        } => mbv::GuiSettingsCommand::AudiobookshelfSetup {
            server_url,
            api_key: mbv::SetupSecret::new(api_key),
        },
        GuiSettingsCommand::ServiceAction { action } => mbv::GuiSettingsCommand::ServiceAction {
            action: action.into(),
        },
        GuiSettingsCommand::CancelSetup { service } => mbv::GuiSettingsCommand::CancelSetup {
            service: service.into(),
        },
        GuiSettingsCommand::FeedAdd { name, url, kind } => {
            mbv::GuiSettingsCommand::FeedAdd { name, url, kind }
        }
        GuiSettingsCommand::FeedEdit { url, name, kind } => {
            mbv::GuiSettingsCommand::FeedEdit { url, name, kind }
        }
        GuiSettingsCommand::FeedRemove { url } => mbv::GuiSettingsCommand::FeedRemove { url },
        GuiSettingsCommand::KeysCapture { target, key } => mbv::GuiSettingsCommand::KeysCapture {
            target,
            key: neutral_key(key),
        },
        GuiSettingsCommand::KeysReset { action } => mbv::GuiSettingsCommand::KeysReset { action },
        GuiSettingsCommand::KeysClear { target } => mbv::GuiSettingsCommand::KeysClear { target },
        GuiSettingsCommand::SelectTarget { target } => mbv::GuiSettingsCommand::SelectTarget {
            target: chooser_target_key(&target),
        },
        GuiSettingsCommand::PlaylistOpen { id } => mbv::GuiSettingsCommand::PlaylistOpen { id },
        GuiSettingsCommand::PlaylistBack => mbv::GuiSettingsCommand::PlaylistBack,
        GuiSettingsCommand::PlaylistRefresh { id } => {
            mbv::GuiSettingsCommand::PlaylistRefresh { id }
        }
        GuiSettingsCommand::PlaylistAction { id, action } => {
            mbv::GuiSettingsCommand::PlaylistAction {
                id,
                action: action.into(),
            }
        }
        GuiSettingsCommand::PlaylistItemAction {
            id,
            item_id,
            action,
        } => mbv::GuiSettingsCommand::PlaylistItemAction {
            id,
            item_id,
            action: action.into(),
        },
        GuiSettingsCommand::PlaylistRename { id, name } => {
            mbv::GuiSettingsCommand::PlaylistRename { id, name }
        }
        GuiSettingsCommand::PlaylistDelete { id } => mbv::GuiSettingsCommand::PlaylistDelete { id },
        GuiSettingsCommand::ResetUiState => mbv::GuiSettingsCommand::ResetUiState,
    })
}
