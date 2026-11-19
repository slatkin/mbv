//! The Settings screen's content entries (add-gui-settings-screen design
//! D11, the port of `lib/settings/entries.ts`): the flat, ordered list of
//! activatable controls the content cursor walks. Each entry carries the
//! stable identity its command needs — a setting name, a choice id, a library
//! name, a feed URL, a playlist id — never a row position.
//!
//! The [`entry_id`] builders are the one grammar for an entry's `id`: the
//! section components paint their own structured rows and mark the row under
//! the cursor by resolving the same id, so the keyboard's order and the
//! paint's order cannot drift.

use crate::bridge::frame::playback::GuiTargetChoice;
use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::feeds::GuiSettingsFeed;
use crate::bridge::frame::settings::library::{GuiSettingsRoute, GuiSettingsRouteDevice};
use crate::bridge::frame::settings::rows::{GuiSettingChoice, GuiSettingRow, GuiSettingRowKind};
use crate::bridge::frame::settings::services::{GuiServiceActionIntent, GuiServiceEntry};
use crate::bridge::settings::GuiChoicesKind;

/// The one grammar for an entry's `id` (`entries.ts`'s `entryId`): the
/// entries builder and every section component name the same control with
/// these builders.
pub mod entry_id {
    use super::{GuiServiceActionIntent, GuiServiceEntry};

    /// The wire name of one Service entry, as the settings command names it.
    fn service_entry_name(entry: GuiServiceEntry) -> &'static str {
        match entry {
            GuiServiceEntry::Emby => "emby",
            GuiServiceEntry::Audiobookshelf => "audiobookshelf",
            GuiServiceEntry::Feeds => "feeds",
        }
    }

    /// The wire name of one Service action intent, as the settings command
    /// names it.
    fn service_intent_name(intent: GuiServiceActionIntent) -> &'static str {
        match intent {
            GuiServiceActionIntent::SetupEmby => "setupEmby",
            GuiServiceActionIntent::RetryEmby => "retryEmby",
            GuiServiceActionIntent::RepairEmby => "repairEmby",
            GuiServiceActionIntent::RemoveEmby => "removeEmby",
            GuiServiceActionIntent::SetupAudiobookshelf => "setupAudiobookshelf",
            GuiServiceActionIntent::TestAudiobookshelf => "testAudiobookshelf",
            GuiServiceActionIntent::RemoveAudiobookshelf => "removeAudiobookshelf",
            GuiServiceActionIntent::ReplaceAudiobookshelf => "replaceAudiobookshelf",
            GuiServiceActionIntent::ManageFeeds => "manageFeeds",
        }
    }

    /// An F2 row's activation entry.
    #[must_use]
    pub fn setting(key: &str) -> String {
        format!("setting:{key}")
    }

    /// One mark of a choice list.
    #[must_use]
    pub fn choice(key: &str, id: &str) -> String {
        format!("choice:{key}:{id}")
    }

    /// A choice list's Apply control.
    #[must_use]
    pub fn choices_apply(key: &str) -> String {
        format!("apply:{key}")
    }

    /// One routable library.
    #[must_use]
    pub fn route(library: &str) -> String {
        format!("route:{library}")
    }

    /// The routes list's automatic entry.
    #[must_use]
    pub fn route_clear(library: &str) -> String {
        format!("routeClear:{library}")
    }

    /// One candidate device of the selected library.
    #[must_use]
    pub fn route_device(library: &str, name: &str) -> String {
        format!("routeDevice:{library}:{name}")
    }

    /// One Service action button.
    #[must_use]
    pub fn service_action(entry: GuiServiceEntry, intent: GuiServiceActionIntent) -> String {
        format!(
            "service:{}:{}",
            service_entry_name(entry),
            service_intent_name(intent)
        )
    }

    /// The feed section's add control.
    #[must_use]
    pub fn feed_add() -> String {
        "feedAdd".to_owned()
    }

    /// One feed subscription.
    #[must_use]
    pub fn feed(url: &str) -> String {
        format!("feed:{url}")
    }

    /// One feed subscription's remove control.
    #[must_use]
    pub fn feed_remove(url: &str) -> String {
        format!("feedRemove:{url}")
    }

    /// One Keys binding target.
    #[must_use]
    pub fn key_target(target: &str) -> String {
        format!("keyTarget:{target}")
    }

    /// One action's reset-to-default control.
    #[must_use]
    pub fn key_reset(action: &str) -> String {
        format!("keyReset:{action}")
    }

    /// One prefix-namespace clear control.
    #[must_use]
    pub fn key_clear(target: &str) -> String {
        format!("keyClear:{target}")
    }

    /// One playback target of the Sessions section.
    #[must_use]
    pub fn target(index: usize) -> String {
        format!("target:{index}")
    }
}

/// One activatable control of the content pane (`entries.ts`'s
/// `SettingsEntry`). The `id` is the [`entry_id`] grammar's name for the
/// control; `pressSettingsEntry` and the painted rows resolve the cursor
/// against it.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsEntry {
    /// An F2 row's activation.
    Setting { id: String, row: GuiSettingRow },
    /// One mark of a choice list: Space toggles it.
    Choice {
        id: String,
        /// The row's own setting name, which keys the marks.
        row_key: String,
        choice: GuiSettingChoice,
        /// The row's full choice list, which seeds the marks.
        choices: Option<Vec<GuiSettingChoice>>,
        /// Whether the user (or the frame) marks this choice.
        marked: bool,
    },
    /// A choice list's Apply control.
    ChoicesApply {
        id: String,
        setting: GuiChoicesKind,
        row: GuiSettingRow,
    },
    /// One routable library: selecting it loads its devices.
    Route { id: String, route: GuiSettingsRoute },
    /// The routes list's automatic entry: assigning it clears the selected
    /// library's route.
    RouteClear { id: String, library: String },
    /// One candidate device of the selected library.
    RouteDevice {
        id: String,
        library: String,
        device: GuiSettingsRouteDevice,
    },
    /// One Service action button.
    ServiceAction {
        id: String,
        intent: GuiServiceActionIntent,
        label: String,
    },
    /// The feed section's add control.
    FeedAdd { id: String },
    /// One feed subscription: Enter opens its edit form.
    Feed { id: String, feed: GuiSettingsFeed },
    /// One feed subscription's remove control.
    FeedRemove { id: String, feed: GuiSettingsFeed },
    /// One Keys binding target: activating it arms the capture.
    KeyTarget {
        id: String,
        /// The frame's target grammar: an action id, `prefix:<action id>`,
        /// or `prefix`.
        target: String,
    },
    /// One action's reset-to-default control.
    KeyReset { id: String, action: String },
    /// One prefix-namespace clear control (a prefix assignment or the
    /// prefix chord).
    KeyClear { id: String, target: String },
    /// One playback target of the Sessions section.
    Target { id: String, target: GuiTargetChoice },
}

/// The marked ids a row's choice list holds, seeded from the frame's
/// `selected` flags and overridden by the user's marks (`entries.ts`'s
/// `markedIds`).
fn marked_ids(row: &GuiSettingRow, marks: &[(String, Vec<String>)]) -> Vec<String> {
    match marks.iter().find(|(key, _)| *key == row.key) {
        Some((_, ids)) => ids.clone(),
        None => row
            .choices
            .as_ref()
            .map(|choices| {
                choices
                    .iter()
                    .filter(|choice| choice.selected)
                    .map(|choice| choice.id.clone())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The entries one F2 row produces (`entries.ts`'s `rowEntries`): a toggle,
/// cycle, or action row is one entry; a choice list is its marks plus its
/// Apply control, painted under the row's own header line. The apply kind is
/// the one the frame carries (design D11 *Choice kind*), never a derivation
/// from the row's name.
fn row_entries(row: &GuiSettingRow, marks: &[(String, Vec<String>)]) -> Vec<SettingsEntry> {
    if row.kind != GuiSettingRowKind::ChoiceList {
        return vec![SettingsEntry::Setting {
            id: entry_id::setting(&row.key),
            row: row.clone(),
        }];
    }
    let Some(kind) = row.choices_kind else {
        return Vec::new();
    };
    let marked = marked_ids(row, marks);
    let mut entries: Vec<SettingsEntry> = row
        .choices
        .iter()
        .flatten()
        .map(|choice| SettingsEntry::Choice {
            id: entry_id::choice(&row.key, &choice.id),
            row_key: row.key.clone(),
            choice: choice.clone(),
            choices: row.choices.clone(),
            marked: marked.contains(&choice.id),
        })
        .collect();
    entries.push(SettingsEntry::ChoicesApply {
        id: entry_id::choices_apply(&row.key),
        setting: kind,
        row: row.clone(),
    });
    entries
}

/// The entries one section's content produces, in paint order
/// (`entries.ts`'s `settingsEntries`). `marks` is the window content's held
/// choice marks.
#[must_use]
pub fn settings_entries(
    content: &GuiSettingsContent,
    marks: &[(String, Vec<String>)],
) -> Vec<SettingsEntry> {
    match content {
        GuiSettingsContent::Rows { rows } => rows
            .iter()
            .flat_map(|row| row_entries(row, marks))
            .collect(),
        GuiSettingsContent::Library {
            rows,
            routes,
            selected,
            devices,
        } => library_entries(
            rows,
            routes.as_deref(),
            selected.as_deref(),
            devices.as_deref(),
            marks,
        ),
        GuiSettingsContent::Services { services, .. } => services
            .iter()
            .flat_map(|service| {
                service
                    .actions
                    .iter()
                    .map(move |action| SettingsEntry::ServiceAction {
                        id: entry_id::service_action(service.entry, action.intent),
                        intent: action.intent,
                        label: action.label.clone(),
                    })
            })
            .collect(),
        GuiSettingsContent::Feeds { subscriptions, .. } => {
            let mut entries = vec![SettingsEntry::FeedAdd {
                id: entry_id::feed_add(),
            }];
            for feed in subscriptions {
                entries.push(SettingsEntry::Feed {
                    id: entry_id::feed(&feed.url),
                    feed: feed.clone(),
                });
                entries.push(SettingsEntry::FeedRemove {
                    id: entry_id::feed_remove(&feed.url),
                    feed: feed.clone(),
                });
            }
            entries
        }
        GuiSettingsContent::Keys { prefix, rows, .. } => keys_entries(prefix.as_deref(), rows),
        GuiSettingsContent::Sessions { targets } => targets
            .iter()
            .enumerate()
            .map(|(index, target)| SettingsEntry::Target {
                id: entry_id::target(index),
                target: target.clone(),
            })
            .collect(),
    }
}

/// The Library section's entries (`entries.ts`'s `library` arm): the choice
/// rows, the routable libraries, and — while one is selected — the automatic
/// clear entry and that library's candidate devices.
fn library_entries(
    rows: &[GuiSettingRow],
    routes: Option<&[GuiSettingsRoute]>,
    selected: Option<&str>,
    devices: Option<&[GuiSettingsRouteDevice]>,
    marks: &[(String, Vec<String>)],
) -> Vec<SettingsEntry> {
    let mut entries: Vec<SettingsEntry> = rows
        .iter()
        .flat_map(|row| row_entries(row, marks))
        .collect();
    for route in routes.into_iter().flatten() {
        entries.push(SettingsEntry::Route {
            id: entry_id::route(&route.library),
            route: route.clone(),
        });
    }
    if let Some(library) = selected {
        entries.push(SettingsEntry::RouteClear {
            id: entry_id::route_clear(library),
            library: library.to_owned(),
        });
        for device in devices.into_iter().flatten() {
            entries.push(SettingsEntry::RouteDevice {
                id: entry_id::route_device(library, &device.name),
                library: library.to_owned(),
                device: device.clone(),
            });
        }
    }
    entries
}

/// The Keys section's entries (`entries.ts`'s `keys` arm): the prefix
/// chord's target and clear, then each action row's targets, resets, and
/// clears.
fn keys_entries(
    prefix: Option<&str>,
    rows: &[crate::bridge::frame::settings::keys::GuiSettingsKeyRow],
) -> Vec<SettingsEntry> {
    let mut entries = vec![SettingsEntry::KeyTarget {
        id: entry_id::key_target("prefix"),
        target: "prefix".to_owned(),
    }];
    if prefix.is_some() {
        entries.push(SettingsEntry::KeyClear {
            id: entry_id::key_clear("prefix"),
            target: "prefix".to_owned(),
        });
    }
    for row in rows {
        entries.push(SettingsEntry::KeyTarget {
            id: entry_id::key_target(&row.id),
            target: row.id.clone(),
        });
        if row.prefix_addressable {
            entries.push(SettingsEntry::KeyTarget {
                id: entry_id::key_target(&format!("prefix:{}", row.id)),
                target: format!("prefix:{}", row.id),
            });
        }
        if row.changed {
            entries.push(SettingsEntry::KeyReset {
                id: entry_id::key_reset(&row.id),
                action: row.id.clone(),
            });
        }
        if row.prefix_chord.is_some() {
            entries.push(SettingsEntry::KeyClear {
                id: entry_id::key_clear(&format!("prefix:{}", row.id)),
                target: format!("prefix:{}", row.id),
            });
        }
    }
    entries
}
