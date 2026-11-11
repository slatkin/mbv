//! The GUI Settings screen's semantic requests (`add-gui-settings-screen`
//! design D5): the `settings` command's neutral form, and the `App` entry that
//! applies one. Identities are stable — a `SettingKey` name, a playlist id, a
//! feed URL, a chord — never row positions, so a command built from a stale
//! frame addresses what that frame painted or nothing.

use mbv_queue::FeedKind;
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::panel_targets::ChooserTargetKey;
use mbv_ui_model::settings::ServiceActionIntent;
use mbv_ui_msg::{KeyEvent, MusicTreeAction};

use super::GuiSettingsSection;
use crate::app::App;

/// One setup form a Services section can show (design D5's `cancelSetup`
/// carries it): the Emby form, or the Audiobookshelf form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiSetupService {
    Emby,
    Audiobookshelf,
}

/// The text a setup form's secret field shows in the command's `Debug` output.
/// The GUI Client's log sink is a file, so a `?command` line at debug level
/// must never carry a typed password or API key (design D9, the spec's
/// *A failed setup does not echo the password*).
const REDACTED: &str = "<redacted>";

/// One request from the GUI Settings screen's window content (design D5). The
/// window content sends intent; every arm that runs calls the existing `App`
/// action for it.
#[derive(Clone, PartialEq, Eq)]
pub enum GuiSettingsCommand {
    /// Close the screen (design D2).
    Close,
    /// Show a section, and start its load (design D9).
    SelectSection { section: GuiSettingsSection },
    /// Activate an F2 row through `App::handle_settings_activate` (row 5.1).
    Activate {
        setting: mbv_ui_model::settings::SettingKey,
    },
    /// Apply a choice list's marked ids through `App::commit_settings_choices`
    /// (row 5.1).
    ApplyChoices {
        kind: MultiSelectKind,
        selected: Vec<String>,
    },
    /// Assign a library route, or clear it when `endpoint` is `None`
    /// (row 5.1).
    AssignRoute {
        library: String,
        endpoint: Option<String>,
    },
    /// Load the candidate devices of `library` for the Library section's
    /// routes list (row 5.1, design D9).
    RouteDevices { library: String },
    /// Fill the Emby setup form and submit it (row 5.2).
    EmbySetup {
        server_url: String,
        username: String,
        password: String,
    },
    /// Fill the Audiobookshelf setup form and submit it (row 5.2).
    AudiobookshelfSetup { server_url: String, api_key: String },
    /// Run one Service action (row 5.2).
    ServiceAction { action: ServiceActionIntent },
    /// Cancel the named setup form (row 5.2).
    CancelSetup { service: GuiSetupService },
    /// Start a feed add (row 5.3).
    FeedAdd {
        name: String,
        url: String,
        kind: FeedKind,
    },
    /// Edit a subscription, addressed by its URL (row 5.3).
    FeedEdit {
        url: String,
        name: String,
        kind: FeedKind,
    },
    /// Raise the remove confirmation for a subscription (row 5.3).
    FeedRemove { url: String },
    /// Capture a key press for a binding target (rows 5.4, design D6).
    KeysCapture { target: String, key: KeyEvent },
    /// Reset one action's binding to its declared default (row 5.4).
    KeysReset { action: String },
    /// Clear one prefix-namespace target (row 5.4).
    KeysClear { target: String },
    /// Select a playback target (row 5.5).
    SelectTarget { target: ChooserTargetKey },
    /// Open a playlist, or the list when `id` is `None` (row 5.5).
    PlaylistOpen { id: Option<String> },
    /// Return from the open playlist to the list (row 5.5).
    PlaylistBack,
    /// Refresh the open playlist, or the one `id` names (row 5.5).
    PlaylistRefresh { id: Option<String> },
    /// Play, shuffle, or enqueue a whole playlist (row 5.5).
    PlaylistAction { id: String, action: MusicTreeAction },
    /// Play, shuffle, or enqueue one item of the open playlist (row 5.5).
    PlaylistItemAction {
        id: String,
        item_id: String,
        action: MusicTreeAction,
    },
    /// Rename a playlist (row 5.5).
    PlaylistRename { id: String, name: String },
    /// Raise the delete confirmation for a playlist (row 5.5).
    PlaylistDelete { id: String },
    /// Reset the GUI's UI state (row 5.6, design D10).
    ResetUiState,
}

/// The `Debug` form masks the two setup secrets (design D9): every log site
/// formats the whole command, so the mask must live here and not at those
/// sites.
impl std::fmt::Debug for GuiSettingsCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Close => f.write_str("Close"),
            Self::SelectSection { section } => f
                .debug_struct("SelectSection")
                .field("section", section)
                .finish(),
            Self::Activate { setting } => f
                .debug_struct("Activate")
                .field("setting", setting)
                .finish(),
            Self::ApplyChoices { kind, selected } => f
                .debug_struct("ApplyChoices")
                .field("kind", kind)
                .field("selected", selected)
                .finish(),
            Self::AssignRoute { library, endpoint } => f
                .debug_struct("AssignRoute")
                .field("library", library)
                .field("endpoint", endpoint)
                .finish(),
            Self::RouteDevices { library } => f
                .debug_struct("RouteDevices")
                .field("library", library)
                .finish(),
            Self::EmbySetup { server_url, .. } => f
                .debug_struct("EmbySetup")
                .field("server_url", server_url)
                .field("password", &REDACTED)
                .finish(),
            Self::AudiobookshelfSetup { server_url, .. } => f
                .debug_struct("AudiobookshelfSetup")
                .field("server_url", server_url)
                .field("api_key", &REDACTED)
                .finish(),
            Self::ServiceAction { action } => f
                .debug_struct("ServiceAction")
                .field("action", action)
                .finish(),
            Self::CancelSetup { service } => f
                .debug_struct("CancelSetup")
                .field("service", service)
                .finish(),
            Self::FeedAdd { name, url, kind } => f
                .debug_struct("FeedAdd")
                .field("name", name)
                .field("url", url)
                .field("kind", kind)
                .finish(),
            Self::FeedEdit { url, name, kind } => f
                .debug_struct("FeedEdit")
                .field("url", url)
                .field("name", name)
                .field("kind", kind)
                .finish(),
            Self::FeedRemove { url } => f.debug_struct("FeedRemove").field("url", url).finish(),
            Self::KeysCapture { target, key } => f
                .debug_struct("KeysCapture")
                .field("target", target)
                .field("key", key)
                .finish(),
            Self::KeysReset { action } => {
                f.debug_struct("KeysReset").field("action", action).finish()
            }
            Self::KeysClear { target } => {
                f.debug_struct("KeysClear").field("target", target).finish()
            }
            Self::SelectTarget { target } => f
                .debug_struct("SelectTarget")
                .field("target", target)
                .finish(),
            Self::PlaylistOpen { id } => f.debug_struct("PlaylistOpen").field("id", id).finish(),
            Self::PlaylistBack => f.write_str("PlaylistBack"),
            Self::PlaylistRefresh { id } => {
                f.debug_struct("PlaylistRefresh").field("id", id).finish()
            }
            Self::PlaylistAction { id, action } => f
                .debug_struct("PlaylistAction")
                .field("id", id)
                .field("action", action)
                .finish(),
            Self::PlaylistItemAction {
                id,
                item_id,
                action,
            } => f
                .debug_struct("PlaylistItemAction")
                .field("id", id)
                .field("item_id", item_id)
                .field("action", action)
                .finish(),
            Self::PlaylistRename { id, name } => f
                .debug_struct("PlaylistRename")
                .field("id", id)
                .field("name", name)
                .finish(),
            Self::PlaylistDelete { id } => {
                f.debug_struct("PlaylistDelete").field("id", id).finish()
            }
            Self::ResetUiState => f.write_str("ResetUiState"),
        }
    }
}

impl App {
    /// Apply one GUI Settings command (design D5): each variant runs through
    /// the shared `App` method for its intent, and a command built from a
    /// stale frame logs its receipt at debug level and changes nothing.
    pub fn apply_gui_settings_command(&mut self, command: GuiSettingsCommand) {
        tracing::debug!(
            name: "gui_settings.command",
            target: "gui_settings",
            command = ?command,
            "the GUI Settings screen sent a command"
        );
        match command {
            GuiSettingsCommand::Close => {
                if self.gui_settings_is_open() {
                    self.close_gui_settings();
                } else {
                    Self::log_ignored_gui_settings_command(
                        "close",
                        "the Settings screen is closed",
                    );
                }
            }
            GuiSettingsCommand::SelectSection { section } => {
                self.select_gui_settings_section(section);
            }
            GuiSettingsCommand::Activate { setting } => {
                self.apply_gui_settings_activate(setting);
            }
            GuiSettingsCommand::ApplyChoices { kind, selected } => {
                self.apply_gui_settings_choices(kind, &selected);
            }
            GuiSettingsCommand::AssignRoute { library, endpoint } => {
                self.apply_gui_settings_route(&library, endpoint);
            }
            GuiSettingsCommand::RouteDevices { library } => {
                self.apply_gui_settings_route_devices(&library);
            }
            GuiSettingsCommand::EmbySetup {
                server_url,
                username,
                password,
            } => self.apply_gui_settings_emby_setup(server_url, username, password),
            GuiSettingsCommand::AudiobookshelfSetup {
                server_url,
                api_key,
            } => self.apply_gui_settings_audiobookshelf_setup(server_url, api_key),
            GuiSettingsCommand::ServiceAction { action } => {
                self.apply_gui_settings_service_action(action);
            }
            GuiSettingsCommand::CancelSetup { service } => {
                self.apply_gui_settings_cancel_setup(service);
            }
            GuiSettingsCommand::FeedAdd { name, url, kind } => {
                self.apply_gui_settings_feed_add(&name, &url, kind);
            }
            GuiSettingsCommand::FeedEdit { url, name, kind } => {
                self.apply_gui_settings_feed_edit(&url, &name, kind);
            }
            GuiSettingsCommand::FeedRemove { url } => {
                self.apply_gui_settings_feed_remove(&url);
            }
            GuiSettingsCommand::KeysCapture { target, key } => {
                self.apply_gui_settings_keys_capture(&target, key);
            }
            GuiSettingsCommand::KeysReset { action } => {
                self.apply_gui_settings_keys_reset(&action);
            }
            GuiSettingsCommand::KeysClear { target } => {
                self.apply_gui_settings_keys_clear(&target);
            }
            GuiSettingsCommand::SelectTarget { target } => {
                self.apply_gui_settings_select_target(&target);
            }
            GuiSettingsCommand::PlaylistOpen { id } => {
                self.apply_gui_settings_playlist_open(id.as_deref());
            }
            GuiSettingsCommand::PlaylistBack => self.apply_gui_settings_playlist_back(),
            GuiSettingsCommand::PlaylistRefresh { id } => {
                self.apply_gui_settings_playlist_refresh(id.as_deref());
            }
            GuiSettingsCommand::PlaylistAction { id, action } => {
                self.apply_gui_settings_playlist_action(&id, action);
            }
            GuiSettingsCommand::PlaylistItemAction {
                id,
                item_id,
                action,
            } => self.apply_gui_settings_playlist_item_action(&id, &item_id, action),
            GuiSettingsCommand::PlaylistRename { id, name } => {
                self.apply_gui_settings_playlist_rename(&id, &name);
            }
            GuiSettingsCommand::PlaylistDelete { id } => {
                self.apply_gui_settings_playlist_delete(&id);
            }
            // The reset's steps span the App and the core thread (design D10):
            // the GUI's tick host applies the whole reset through
            // `CoreThread::reset_gui_ui_state`, so the App-side arm starts it
            // by running the App-owned steps in their order.
            GuiSettingsCommand::ResetUiState => self.reset_gui_ui_state(),
        }
    }
}
