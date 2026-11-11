//! The GUI Settings screen's frame content (`add-gui-settings-screen` design
//! D9): the F2 rows of the open section, and the Services, Feeds, Keys, and
//! Playlists sections' data. The frame's wire DTOs in `mbv-gui` translate
//! these one-to-one. Every projection reads stable identities — the
//! `SettingKey` name, the feed URL, the action id, the playlist id — and the
//! setup form carries no password and no API key.

use mbv_emby_model::EmbyItem;
use mbv_queue::FeedKind;
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::settings::{
    SERVICE_ENTRIES, ServiceActionIntent, ServiceEntry, SettingKey, SettingValueKind, setting_flag,
    setting_kind, setting_label, setting_value,
};

use crate::app::App;

use super::{GuiSettingsState, GuiSetupService};

/// One F2 row the Settings frame carries (design D9): the setting's stable
/// name, its label, how the row edits it, its value text, and — for a toggle —
/// the flag the row paints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsRowProjection {
    pub key: SettingKey,
    pub label: String,
    pub kind: GuiSettingsRowKind,
    pub value: String,
    pub flag: Option<bool>,
    /// The choice list's loaded choices (design D9). `None` until the
    /// section's load lands (row 5.1), and for a row that is no choice list.
    pub choices: Option<Vec<GuiSettingsChoiceProjection>>,
}

/// The four ways an F2 row is edited (design D9): a toggle, a cycle, a choice
/// list, or an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiSettingsRowKind {
    Toggle,
    Cycle,
    ChoiceList,
    Action,
}

/// One choice of a choice-list row (design D9); `id` is the value the config
/// list stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsChoiceProjection {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

/// One routable library of the Library section's routes list (design D9):
/// `library` is the lowercased name the saved route table is keyed by — the
/// `assignRoute` command's identity — and `endpoint` the device route the
/// library is assigned to, `None` while it uses the automatic route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsRouteProjection {
    pub library: String,
    pub name: String,
    pub endpoint: Option<String>,
}

impl From<(String, String, Option<String>)> for GuiSettingsRouteProjection {
    fn from((library, name, endpoint): (String, String, Option<String>)) -> Self {
        Self {
            library,
            name,
            endpoint,
        }
    }
}

/// One candidate device of the selected library (design D9): `name` is the
/// device the `assignRoute` command names and `endpoint` the direct-connect
/// endpoint it saves, `None` when the device is not currently routable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsRouteDeviceProjection {
    pub name: String,
    pub endpoint: Option<String>,
}

impl From<(String, Option<String>)> for GuiSettingsRouteDeviceProjection {
    fn from((name, endpoint): (String, Option<String>)) -> Self {
        Self { name, endpoint }
    }
}

/// The Library section's content (design D9): its F2 choice rows, the routable
/// libraries with their assigned devices, the selected library, and that
/// library's candidate devices once `routeDevices` ran for it. The
/// `LibraryRoutes` F2 row is absent: its TUI popup's work lives inline here,
/// and its activation would raise an overlay the GUI drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsLibraryProjection {
    pub rows: Vec<GuiSettingsRowProjection>,
    pub routes: Option<Vec<GuiSettingsRouteProjection>>,
    pub selected: Option<String>,
    pub devices: Option<Vec<GuiSettingsRouteDeviceProjection>>,
}

/// The Services section's content (design D9): each Service's state and the
/// actions it offers now, plus the open setup form's progress and error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsServicesProjection {
    pub services: Vec<GuiSettingsServiceProjection>,
    pub setup: Option<GuiSettingsSetupProjection>,
}

/// One Service row (design D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsServiceProjection {
    pub entry: ServiceEntry,
    pub name: &'static str,
    pub state: &'static str,
    pub context: String,
    pub actions: Vec<(ServiceActionIntent, &'static str)>,
}

/// The open setup form's facts (design D9): no secret field exists on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsSetupProjection {
    pub service: GuiSetupService,
    pub server_url: String,
    pub busy: bool,
    pub error: String,
}

/// The Feeds section's content (design D9): the subscriptions, addressed by
/// URL, and whether an add is in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsFeedsProjection {
    pub subscriptions: Vec<GuiSettingsFeedProjection>,
    pub adding: bool,
}

/// One feed subscription (design D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsFeedProjection {
    pub name: String,
    pub url: String,
    pub kind: FeedKind,
}

/// The Keys section's content (design D9): the prefix chord, one row for each
/// `AllClients` action, and the last refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsKeysProjection {
    pub prefix: Option<String>,
    pub rows: Vec<GuiSettingsKeyRowProjection>,
    /// The last refused edit (design D9). `None` until row 5.4 stores one.
    pub refusal: Option<GuiSettingsKeyRefusalProjection>,
}

/// One Keys row (design D9): the action's stable id and label, its section,
/// the effective router chords, its prefix-namespace chord, and the two flags
/// the row marks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsKeyRowProjection {
    pub id: &'static str,
    pub label: String,
    pub section: &'static str,
    pub chords: Vec<String>,
    pub prefix_chord: Option<String>,
    pub changed: bool,
    pub prefix_addressable: bool,
}

/// A refused Keys edit (design D9): the action's id and the refusal text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsKeyRefusalProjection {
    pub action: String,
    pub text: String,
}

/// The Playlists section's content (design D9): the saved list, the open
/// playlist's items, and the two loading flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsPlaylistsProjection {
    pub playlists: Vec<GuiSettingsPlaylistRowProjection>,
    pub open: Option<GuiSettingsOpenPlaylistProjection>,
    pub loading: bool,
    pub open_loading: bool,
}

/// One saved playlist (design D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsPlaylistRowProjection {
    pub id: String,
    pub name: String,
    pub item_count: u32,
}

/// The open playlist and its items (design D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsOpenPlaylistProjection {
    pub id: String,
    pub name: String,
    pub items: Vec<GuiSettingsPlaylistItemProjection>,
}

/// One item of the open playlist (design D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiSettingsPlaylistItemProjection {
    pub id: String,
    pub name: String,
}

impl App {
    /// The open section's F2 rows (design D9): the shown keys in paint order,
    /// each with its label, kind, value text, toggle flag, and — for a choice
    /// list — the choices the section's load cached. The `LibraryRoutes` row
    /// is never projected: the GUI's Library section does that work inline
    /// (design D9).
    #[must_use]
    pub fn gui_settings_rows(&self) -> Vec<GuiSettingsRowProjection> {
        let Some(state) = self.gui_settings() else {
            return Vec::new();
        };
        let cfg = self.config_snapshot();
        let ui = self.ui_config_snapshot();
        state
            .section
            .shown_rows()
            .into_iter()
            .filter(|key| *key != SettingKey::LibraryRoutes)
            .map(|key| GuiSettingsRowProjection {
                key,
                label: setting_label(key).to_string(),
                kind: gui_settings_row_kind(key),
                value: setting_value(key, &cfg, &ui),
                flag: setting_flag(key, &cfg),
                choices: gui_settings_row_choices(&state, key),
            })
            .collect()
    }

    /// The Services section's content (design D9): each Service's state, the
    /// actions the TUI offers for it, and the open setup form's progress and
    /// error. The form carries the server URL only — never a password or an
    /// API key.
    #[must_use]
    pub fn gui_settings_services(&self) -> GuiSettingsServicesProjection {
        let services = SERVICE_ENTRIES
            .iter()
            .map(|&entry| GuiSettingsServiceProjection {
                entry,
                name: Self::service_entry_name(entry),
                state: self.service_state_label(entry),
                context: self.service_context(entry),
                actions: self
                    .service_action_intents(entry)
                    .into_iter()
                    .map(|intent| (intent, intent.label()))
                    .collect(),
            })
            .collect();
        GuiSettingsServicesProjection {
            services,
            setup: self.gui_settings_setup(),
        }
    }

    /// The open setup form's facts (design D9), whichever form holds it.
    fn gui_settings_setup(&self) -> Option<GuiSettingsSetupProjection> {
        if let Some(form) = self.setup.emby_setup_form.as_ref() {
            return Some(GuiSettingsSetupProjection {
                service: GuiSetupService::Emby,
                server_url: form.fields[0].clone(),
                busy: form.busy,
                error: form.error.clone(),
            });
        }
        self.setup
            .audiobookshelf_setup_form
            .as_ref()
            .map(|form| GuiSettingsSetupProjection {
                service: GuiSetupService::Audiobookshelf,
                server_url: form.fields[0].clone(),
                busy: form.busy,
                error: form.error.clone(),
            })
    }

    /// The Feeds section's content (design D9): the subscriptions from the
    /// config, and whether an add is in flight.
    #[must_use]
    pub fn gui_settings_feeds(&self) -> GuiSettingsFeedsProjection {
        let subscriptions = self
            .config_snapshot()
            .feeds
            .iter()
            .map(|sub| GuiSettingsFeedProjection {
                name: sub.name.clone(),
                url: sub.url.clone(),
                kind: sub.kind,
            })
            .collect();
        let adding = self
            .feeds_manage
            .as_ref()
            .is_some_and(|popup| popup.pending_add.is_some());
        GuiSettingsFeedsProjection {
            subscriptions,
            adding,
        }
    }

    /// The Keys section's content (design D9): the prefix, then one row for
    /// each `AllClients` action in the shared section order. A `TuiOnly`
    /// action is absent, and a collision with one is still refused at load
    /// (design D4).
    #[must_use]
    pub fn gui_settings_keys(&self) -> GuiSettingsKeysProjection {
        let keybinds = self.config_snapshot().keybinds;
        let rows = mbv_keybinds::KEY_SECTIONS
            .iter()
            .flat_map(|section| {
                mbv_keybinds::KEYBIND_ACTIONS
                    .iter()
                    .filter(move |action| action.section == *section)
            })
            .filter(|action| action.scope == mbv_keybinds::ActionScope::AllClients)
            .map(|action| GuiSettingsKeyRowProjection {
                id: action.id,
                label: action.id.to_string(),
                section: action.section.name(),
                chords: keybinds
                    .router_chords(action)
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                prefix_chord: keybinds
                    .prefix_assignment(action.id)
                    .map(|chord| chord.to_string()),
                changed: keybinds
                    .router_override(action.id)
                    .is_some_and(|chord| !action.parsed_default_chords().contains(&chord)),
                prefix_addressable: action.prefix_addressable,
            })
            .collect();
        GuiSettingsKeysProjection {
            prefix: keybinds.prefix.map(|chord| chord.to_string()),
            rows,
            refusal: self.gui_settings().and_then(|state| state.refusal),
        }
    }

    /// The Library section's routable libraries (design D9): the loaded route
    /// rows, `None` until the section's fetch lands. The front end paints each
    /// one's assigned device and sends `assignRoute` for the library it edits.
    #[must_use]
    pub fn gui_settings_routes(&self) -> Option<Vec<GuiSettingsRouteProjection>> {
        self.gui_settings().and_then(|state| state.routes)
    }

    /// The Library section's content (design D9): its F2 choice rows, the
    /// routable libraries, the selected library, and that library's candidate
    /// devices. The front end sends `routeDevices` when the user selects a
    /// library, and `assignRoute` for the device the user picks.
    #[must_use]
    pub fn gui_settings_library(&self) -> GuiSettingsLibraryProjection {
        let state = self.gui_settings();
        GuiSettingsLibraryProjection {
            rows: self.gui_settings_rows(),
            routes: state.as_ref().and_then(|state| state.routes.clone()),
            selected: state
                .as_ref()
                .and_then(|state| state.route_selection.clone()),
            devices: state.and_then(|state| state.route_devices),
        }
    }

    /// The Playlists section's content (design D9): the saved playlists with
    /// their item counts, the open playlist and its items, and the two
    /// loading flags.
    #[must_use]
    pub fn gui_settings_playlists(&self) -> GuiSettingsPlaylistsProjection {
        GuiSettingsPlaylistsProjection {
            playlists: self.playlists.iter().map(playlist_row).collect(),
            open: self
                .playlists_open
                .as_ref()
                .map(|playlist| GuiSettingsOpenPlaylistProjection {
                    id: playlist.id.clone(),
                    name: playlist.name.clone(),
                    items: self
                        .playlists_open_items
                        .iter()
                        .map(playlist_item)
                        .collect(),
                }),
            loading: self.playlists_loading,
            open_loading: self.playlists_open_loading,
        }
    }
}

/// One saved playlist's row (design D9): its stable id, name, and item count.
fn playlist_row(item: &EmbyItem) -> GuiSettingsPlaylistRowProjection {
    GuiSettingsPlaylistRowProjection {
        id: item.id.clone(),
        name: item.name.clone(),
        item_count: item.child_count.unwrap_or(0),
    }
}

/// One item of the open playlist (design D9): its stable id and name.
fn playlist_item(item: &EmbyItem) -> GuiSettingsPlaylistItemProjection {
    GuiSettingsPlaylistItemProjection {
        id: item.id.clone(),
        name: item.name.clone(),
    }
}

/// The choices a row paints (design D9): `None` for a row that is no choice
/// list, and for one whose load has not landed yet.
fn gui_settings_row_choices(
    state: &GuiSettingsState,
    key: SettingKey,
) -> Option<Vec<GuiSettingsChoiceProjection>> {
    let kind = match key {
        SettingKey::MyLanguages => MultiSelectKind::MyLanguages,
        SettingKey::HiddenLibraries => MultiSelectKind::HiddenLibraries,
        SettingKey::FeedViewLibraries => MultiSelectKind::FeedViewLibraries,
        _ => return None,
    };
    state
        .choices(kind)
        .map(<[GuiSettingsChoiceProjection]>::to_vec)
}

/// How an F2 row is edited (design D9). A boolean row is a toggle, a text or
/// stepper row a cycle, and a collection row a choice list — except the three
/// collection rows that run an action. The two navigation keys (`Services`,
/// `Keys`) never reach a row: their sections carry their own content.
fn gui_settings_row_kind(key: SettingKey) -> GuiSettingsRowKind {
    match setting_kind(key) {
        SettingValueKind::Boolean => GuiSettingsRowKind::Toggle,
        SettingValueKind::Text | SettingValueKind::Stepper => GuiSettingsRowKind::Cycle,
        SettingValueKind::Collection
            if matches!(
                key,
                SettingKey::ManageFeeds | SettingKey::ResetUiState | SettingKey::LogOut
            ) =>
        {
            GuiSettingsRowKind::Action
        }
        SettingValueKind::Collection => GuiSettingsRowKind::ChoiceList,
        SettingValueKind::Key => GuiSettingsRowKind::Action,
    }
}
