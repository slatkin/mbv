//! The Settings screen's own state (add-gui-settings-screen design D1, ADR
//! 0037, the port of `lib/settings/state.ts`): what the window content owns
//! and the core never reads — the keyboard pane, the sidebar and content
//! cursors, the Mini pane (which is the keyboard pane), the form drafts, the
//! choice marks before apply, and the capture. The core owns the open section
//! and every section's data, so nothing here mirrors them: the screen paints
//! the frame and keeps only the facts the user's own input moves.
//!
//! One pane fact serves both jobs: in Mini only the pane that holds the
//! keyboard paints, so opening a section moves the keyboard to the content
//! and Esc's content-to-sidebar step is what shows the sidebar again.
//!
//! `state.ts`'s module-level `settingsUi` store is one [`SettingsState`] the
//! root provides as context (D15); its functions are the struct's methods.

use dioxus_native::prelude::*;

use crate::bridge::frame::playback::GuiTargetChoice;
use crate::bridge::frame::settings::rows::{GuiSettingChoice, GuiSettingRow, GuiSettingRowKind};
use crate::bridge::frame::settings::services::GuiSettingsSetup;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame};
use crate::bridge::keys::DomKey;
use crate::bridge::settings::{GuiChoicesKind, GuiSettingsCommand};
use crate::window::channel::CoreLink;
use crate::window::library::list::{ListMove, move_index};
use crate::window::sections::Sections;

use super::entries::{SettingsEntry, settings_entries};
use super::pick_menu::PickMenuState;

/// The rows a Page Up or Page Down step spans in a Settings pane (`state.ts`'s
/// `SETTINGS_PAGE`).
const SETTINGS_PAGE: usize = 10;

/// The pane the keyboard is on — and the one Mini paints (`state.ts`'s
/// `SettingsPane`, design D11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPane {
    Sidebar,
    Content,
}

/// The form a focused text field belongs to (`state.ts`'s `SettingsForm`,
/// design D11, step 3): the fact the key router reads before it types into
/// the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsForm {
    EmbySetup,
    AudiobookshelfSetup,
    Feed,
    Rename,
}

/// The Emby setup form's fields, as the user typed them (`state.ts`'s
/// `EmbySetupDraft`, design D9). The password lives here until the submit
/// carries it to the core; no frame brings it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbySetupDraft {
    pub server_url: String,
    pub username: String,
    pub password: String,
}

/// The Audiobookshelf setup form's fields (`state.ts`'s
/// `AudiobookshelfSetupDraft`, design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookshelfSetupDraft {
    pub server_url: String,
    pub api_key: String,
}

/// The feed add or edit form's fields (`state.ts`'s `FeedFormDraft`, design
/// D9). `identity` is the subscription URL an edit addresses; `None` on an
/// add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedFormDraft {
    pub identity: Option<String>,
    pub name: String,
    pub url: String,
    pub kind: mbv_queue::FeedKind,
    /// The open kind menu's cursor, an index into [`FEED_KINDS`]; `None`
    /// while the menu is closed. the former client `<select>` drew its own options,
    /// so the port keeps the menu's one UI fact beside the draft it edits
    /// and lets it die with the form.
    pub kind_menu: Option<usize>,
}

/// The kinds the feed form offers, in the former client `<select>`'s option order.
pub const FEED_KINDS: [mbv_queue::FeedKind; 2] =
    [mbv_queue::FeedKind::Audio, mbv_queue::FeedKind::Video];

/// The kind `index` names, while it is one of [`FEED_KINDS`].
#[must_use]
pub fn feed_kind_at(index: usize) -> Option<mbv_queue::FeedKind> {
    FEED_KINDS.get(index).copied()
}

/// The playlist rename field's draft (`state.ts`'s `RenameDraft`, design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameDraft {
    pub id: String,
    pub name: String,
}

/// The binding target a capture waits on (`state.ts`'s `capture`, design D6):
/// the frame's own target grammar — an action id, `prefix:<action id>`, or
/// `prefix`. `sent` marks the capture that sent its `keysCapture` and waits
/// for the next frame's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub target: String,
    pub sent: bool,
}

/// The Settings screen's interaction state (`state.ts`'s `SettingsUiState`).
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsUiState {
    /// The pane the keyboard is on (design D11): the sidebar or the content.
    /// In Mini it is also the pane that paints.
    pub pane: SettingsPane,
    /// The sidebar's cursor, an index into the frame's `sections`.
    pub sidebar_cursor: usize,
    /// The content pane's cursor, an index into the section's entries.
    pub content_cursor: usize,
    /// Whether the cursor paints: a screen key shows it and a pointer press
    /// hides it, like `:focus-visible`. A pointer user sees no row that looks
    /// picked.
    pub keyboard: bool,
    /// The section the last frame showed: a change re-anchors the sidebar
    /// cursor and drops the content cursor and every draft. `None` before the
    /// first open frame.
    pub section: Option<crate::bridge::frame::settings::GuiSettingsSection>,
    /// The binding target a capture waits on, or `None`.
    pub capture: Option<Capture>,
    /// The marked ids of each touched choice list, by the row's setting name,
    /// in mark order; a row the user has not marked yet falls back to the
    /// frame's `selected` flags. The pair list keeps the TypeScript record's
    /// insertion order, which the apply command's `selected` preserves.
    pub choice_marks: Vec<(String, Vec<String>)>,
    /// The form whose text field holds the keyboard (design D11, step 3), or
    /// `None`. A field's DOM focus events set and clear it.
    pub text_field: Option<SettingsForm>,
    /// The Emby setup draft; `None` while the frame opens no Emby form.
    pub emby_setup: Option<EmbySetupDraft>,
    /// The Audiobookshelf setup draft; `None` while the frame opens no
    /// Audiobookshelf form.
    pub audiobookshelf_setup: Option<AudiobookshelfSetupDraft>,
    /// The feed add or edit draft; `None` while no feed form is open.
    pub feed_form: Option<FeedFormDraft>,
    /// The playlist rename draft; `None` while no rename field is open.
    pub rename: Option<RenameDraft>,
}

impl SettingsUiState {
    /// The state a freshly opened screen starts with (`state.ts`'s
    /// `initialSettingsUi`).
    #[must_use]
    pub fn initial() -> Self {
        Self {
            pane: SettingsPane::Sidebar,
            sidebar_cursor: 0,
            content_cursor: 0,
            keyboard: false,
            section: None,
            capture: None,
            choice_marks: Vec::new(),
            text_field: None,
            emby_setup: None,
            audiobookshelf_setup: None,
            feed_form: None,
            rename: None,
        }
    }
}

/// The Settings screen's interaction store (D15): the port of `state.ts`'s
/// module-level stores as one context-provided struct. The root creates it
/// once and provides it as context; the key router and the painters read and
/// write it. A clone shares every field's storage, so the key router's
/// closure and the painters see one state.
#[derive(Clone)]
pub struct SettingsState {
    /// The screen's interaction state (`state.ts`'s `settingsUi`).
    pub ui: Signal<SettingsUiState>,
    /// The open pick menu (`pick-menu.ts`'s `pickMenu`), or `None` while
    /// every menu is closed.
    pub pick_menu: Signal<Option<PickMenuState>>,
    /// The core link the Settings commands go through
    /// (`lib/bridge/settings.ts`'s `sendSettingsCommand`).
    pub link: CoreLink,
}

impl std::fmt::Debug for SettingsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsState")
            .field("ui", &self.ui.peek().clone())
            .field("pick_menu", &self.pick_menu.peek().clone())
            .finish_non_exhaustive()
    }
}

/// The entries the section's content pane walks (`state.ts`'s
/// `currentSettingsEntries`, design D11): the flat list the cursor indexes
/// and the components paint, with the choice marks the user holds applied.
#[must_use]
pub fn current_settings_entries(
    content: &GuiSettingsContent,
    ui: &SettingsUiState,
) -> Vec<SettingsEntry> {
    settings_entries(content, &ui.choice_marks)
}

/// The entry the content cursor rests on, or `None` while the section lists
/// none (`state.ts`'s `focusedSettingsEntry`).
#[must_use]
pub fn focused_settings_entry(
    content: &GuiSettingsContent,
    ui: &SettingsUiState,
) -> Option<SettingsEntry> {
    let entries = current_settings_entries(content, ui);
    entries
        .get(ui.content_cursor.min(entries.len().wrapping_sub(1)))
        .cloned()
}

/// The id of the entry the content cursor rests on, or `None` while the
/// section lists none (`state.ts`'s `cursorEntryId`): the fact a component's
/// rows paint their cursor fill from.
#[must_use]
pub fn cursor_entry_id(entries: &[SettingsEntry], ui: &SettingsUiState) -> Option<String> {
    entries
        .get(ui.content_cursor.min(entries.len().wrapping_sub(1)))
        .map(entry_id_string)
}

/// The `id` an entry carries, for the cursor resolution the painters share
/// with the router.
#[must_use]
pub fn entry_id_string(entry: &SettingsEntry) -> String {
    match entry {
        SettingsEntry::Setting { id, .. }
        | SettingsEntry::Choice { id, .. }
        | SettingsEntry::ChoicesApply { id, .. }
        | SettingsEntry::Route { id, .. }
        | SettingsEntry::RouteClear { id, .. }
        | SettingsEntry::RouteDevice { id, .. }
        | SettingsEntry::ServiceAction { id, .. }
        | SettingsEntry::FeedAdd { id }
        | SettingsEntry::Feed { id, .. }
        | SettingsEntry::FeedRemove { id, .. }
        | SettingsEntry::KeyTarget { id, .. }
        | SettingsEntry::KeyReset { id, .. }
        | SettingsEntry::KeyClear { id, .. }
        | SettingsEntry::Target { id, .. } => id.clone(),
    }
}

impl SettingsState {
    /// The store a window start builds: the freshly mounted interaction state
    /// over the core link the commands go through.
    #[must_use]
    pub fn new(link: CoreLink) -> Self {
        Self {
            ui: Signal::new(SettingsUiState::initial()),
            pick_menu: Signal::new(None),
            link,
        }
    }

    /// A pointer press on an entry's row (`state.ts`'s `pressSettingsEntry`,
    /// design D11): the cursor moves onto it and the keyboard takes the
    /// content pane, then the row's activation runs — the one gesture a click
    /// always means.
    pub fn press_settings_entry(&self, entries: &[SettingsEntry], id: &str) {
        let Some(index) = entries
            .iter()
            .position(|candidate| entry_id_string(candidate) == id)
        else {
            return;
        };
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.pane = SettingsPane::Content;
            ui.content_cursor = index;
        });
        self.activate_settings_entry(&entries[index]);
    }

    /// Open a section (`state.ts`'s `selectSettingsSection`, design D11): the
    /// sidebar entry's activation. The command starts the section's load in
    /// the core (design D9), and the keyboard moves to the content so Mini
    /// shows it (the spec's *Narrow window* requirement).
    pub fn select_settings_section(
        &self,
        section: crate::bridge::frame::settings::GuiSettingsSection,
    ) {
        self.link
            .settings(GuiSettingsCommand::SelectSection { section });
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.pane = SettingsPane::Content);
    }

    /// Switch the keyboard between the sidebar and the content (`state.ts`'s
    /// `switchSettingsPane`, design D11): Tab and Shift+Tab's one move.
    pub fn switch_settings_pane(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.pane = if ui.pane == SettingsPane::Sidebar {
                SettingsPane::Content
            } else {
                SettingsPane::Sidebar
            };
        });
    }

    /// Show the cursor (`state.ts`'s `showSettingsCursor`): a screen key ran.
    pub fn show_settings_cursor(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.keyboard = true);
    }

    /// Hide the cursor (`state.ts`'s `hideSettingsCursor`): a pointer press
    /// on the screen.
    pub fn hide_settings_cursor(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.keyboard = false);
    }

    /// Move the focused pane's cursor one movement along (`state.ts`'s
    /// `moveSettingsCursor`, design D11): Up and Down a row, Page Up and Page
    /// Down a page of rows, Home and End the ends. The cursor stays inside the
    /// list the pane paints.
    pub fn move_settings_cursor(&self, mv: ListMove, count: usize) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if count == 0 {
                return;
            }
            if ui.pane == SettingsPane::Sidebar {
                ui.sidebar_cursor = move_index(count, ui.sidebar_cursor, mv, SETTINGS_PAGE);
            } else {
                ui.content_cursor = move_index(count, ui.content_cursor, mv, SETTINGS_PAGE);
            }
        });
    }

    /// Esc's ladder while the screen shows (`state.ts`'s `settingsEscape`,
    /// design D11, the spec's *Esc closes the screen from the sidebar*): while
    /// the content holds the keyboard, a draft or a form ends first and the
    /// content then hands the keyboard back to the sidebar. The sidebar's Esc
    /// closes the screen whatever is open in the content. The capture's and
    /// the text field's own steps run before this in the router, so an Esc
    /// that reaches this point faces no capture and no field hold.
    pub fn settings_escape(&self, settings: &GuiSettingsFrame) {
        let ui = self.ui.peek().clone();
        if ui.pane == SettingsPane::Sidebar {
            self.link.settings(GuiSettingsCommand::Close);
            return;
        }
        if ui.feed_form.is_some() || ui.rename.is_some() {
            let mut ui = self.ui;
            ui.with_mut(|state| {
                state.feed_form = None;
                state.rename = None;
            });
            return;
        }
        if let Some(setup) = frame_setup_content(&settings.content) {
            self.link.settings(GuiSettingsCommand::CancelSetup {
                service: setup.service,
            });
            return;
        }
        let mut ui = self.ui;
        ui.with_mut(|state| state.pane = SettingsPane::Sidebar);
    }

    /// Arm the capture for one binding target (`state.ts`'s
    /// `startKeysCapture`, design D6): the next key press is its candidate
    /// chord.
    pub fn start_keys_capture(&self, target: &str) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.capture = Some(Capture {
                target: target.to_owned(),
                sent: false,
            });
        });
    }

    /// End the capture without a candidate (`state.ts`'s
    /// `cancelKeysCapture`): Esc's cancel (design D6).
    pub fn cancel_keys_capture(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.capture = None);
    }

    /// Send the pressed key as the capture's candidate chord (`state.ts`'s
    /// `sendKeysCapture`, design D6): the raw DOM key rides `keysCapture`,
    /// and the capture stays painted until the next frame answers it.
    pub fn send_keys_capture(&self, target: String, key: DomKey) {
        self.link
            .settings(GuiSettingsCommand::KeysCapture { target, key });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if let Some(capture) = &mut ui.capture {
                capture.sent = true;
            }
        });
    }

    /// The marked ids of one choice-list row (`state.ts`'s `markedChoices`):
    /// the user's marks while they hold any, else the frame's `selected`
    /// flags.
    #[must_use]
    pub fn marked_choices(&self, key: &str, choices: Option<&[GuiSettingChoice]>) -> Vec<String> {
        seeded_or_held(&self.ui.peek().choice_marks, key, choices)
    }

    /// Flip one mark of a choice list (`state.ts`'s `toggleChoiceMark`,
    /// design D11's Space).
    pub fn toggle_choice_mark(
        &self,
        setting: &str,
        id: &str,
        choices: Option<&[GuiSettingChoice]>,
    ) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            let marked = seeded_or_held(&ui.choice_marks, setting, choices);
            let next: Vec<String> = if marked.iter().any(|candidate| candidate == id) {
                marked
                    .into_iter()
                    .filter(|candidate| candidate != id)
                    .collect()
            } else {
                let mut next = marked;
                next.push(id.to_owned());
                next
            };
            match ui.choice_marks.iter_mut().find(|(name, _)| name == setting) {
                Some((_, ids)) => *ids = next,
                None => ui.choice_marks.push((setting.to_owned(), next)),
            }
        });
    }

    /// Apply a choice list's marked ids (`state.ts`'s `applyChoiceList`,
    /// design D9): the `applyChoices` command runs the TUI's multiselect
    /// commit with the kind the frame carried on the row (design D11 *Choice
    /// kind*).
    pub fn apply_choice_list(
        &self,
        kind: GuiChoicesKind,
        key: &str,
        choices: Option<&[GuiSettingChoice]>,
    ) {
        let selected = self.marked_choices(key, choices);
        self.link
            .settings(GuiSettingsCommand::ApplyChoices { kind, selected });
    }

    /// Run one Service action (`state.ts`'s `runServiceAction`): a button's
    /// press or the cursor's activation.
    pub fn run_service_action(
        &self,
        intent: crate::bridge::frame::settings::services::GuiServiceActionIntent,
    ) {
        self.link
            .settings(GuiSettingsCommand::ServiceAction { action: intent });
    }

    /// Select a playback target in the Sessions section (`state.ts`'s
    /// `selectSettingsTarget`): the chooser's own path, so the screen stays
    /// open (the spec's *Sessions* requirement).
    pub fn select_settings_target(&self, target: &GuiTargetChoice) {
        self.link.settings(GuiSettingsCommand::SelectTarget {
            target: target.key.clone(),
        });
    }

    /// Activate one content entry (`state.ts`'s `activateSettingsEntry`,
    /// design D11's Enter, and Space on a toggle or a choice mark): the
    /// cursor row's own command, or the edit the row opens.
    pub fn activate_settings_entry(&self, entry: &SettingsEntry) {
        match entry {
            SettingsEntry::Setting { row, .. } => self.activate_setting_row(row),
            SettingsEntry::Choice {
                row_key,
                choice,
                choices,
                ..
            } => {
                self.toggle_choice_mark(row_key, &choice.id, choices.as_deref());
            }
            SettingsEntry::ChoicesApply { setting, row, .. } => {
                self.apply_choice_list(*setting, &row.key, row.choices.as_deref());
            }
            SettingsEntry::Route { route, .. } => {
                self.link.settings(GuiSettingsCommand::RouteDevices {
                    library: route.library.clone(),
                });
            }
            SettingsEntry::RouteClear { library, .. } => {
                self.link.settings(GuiSettingsCommand::AssignRoute {
                    library: library.clone(),
                    endpoint: None,
                });
            }
            SettingsEntry::RouteDevice {
                library, device, ..
            } => {
                // A device with no endpoint is not currently routable; the
                // row paints disabled and a press on it assigns nothing, so
                // the GUI never asks the core to clear a route by accident.
                if let Some(endpoint) = &device.endpoint {
                    self.link.settings(GuiSettingsCommand::AssignRoute {
                        library: library.clone(),
                        endpoint: Some(endpoint.clone()),
                    });
                }
            }
            SettingsEntry::ServiceAction { intent, .. } => self.run_service_action(*intent),
            SettingsEntry::FeedAdd { .. } => self.open_feed_form(None),
            SettingsEntry::Feed { feed, .. } => self.open_feed_form(Some(feed)),
            SettingsEntry::FeedRemove { feed, .. } => {
                self.link.settings(GuiSettingsCommand::FeedRemove {
                    url: feed.url.clone(),
                });
            }
            SettingsEntry::KeyTarget { target, .. } => self.start_keys_capture(target),
            SettingsEntry::KeyReset { action, .. } => {
                self.link.settings(GuiSettingsCommand::KeysReset {
                    action: action.clone(),
                });
            }
            SettingsEntry::KeyClear { target, .. } => {
                self.link.settings(GuiSettingsCommand::KeysClear {
                    target: target.clone(),
                });
            }
            SettingsEntry::Target { target, .. } => self.select_settings_target(target),
        }
    }

    /// Activate one F2 row (`state.ts`'s `activateSettingsEntry`'s `setting`
    /// arm): the row's own command, or the edit the row opens.
    fn activate_setting_row(&self, row: &GuiSettingRow) {
        match row.kind {
            GuiSettingRowKind::Pick => {
                // A value setting opens its list (the spec's *Value settings
                // are picked from their list*): the same menu a press on its
                // button opens. The menu's own choice sends `setValue`.
                let values = row.values.clone().unwrap_or_default();
                self.toggle_pick_menu(&row.key, &row.value, &values);
            }
            GuiSettingRowKind::ChoiceList | GuiSettingRowKind::Info => {
                // A choice list's activate would raise the TUI's
                // multiselect, which the overlay drain drops; its marks edit
                // in place. An info row is read-only
                // (owner-gate-on-wire-identity design D5): its activation
                // sends nothing.
            }
            GuiSettingRowKind::Action if row.key == "resetUiState" => {
                // The core's activate of this row is a no-op (its TUI
                // activation runs through the shell's own path), so the
                // GUI's row sends the reset command (design D5).
                self.link.settings(GuiSettingsCommand::ResetUiState);
            }
            _ => {
                self.link.settings(GuiSettingsCommand::Activate {
                    setting: row.key.clone(),
                });
            }
        }
    }
}

/// Submit the form that holds the keyboard against the frame the section
/// store carries now (`state.ts`'s `submitSettingsForm(content)` at the
/// press): a form's button reads the shown frame here instead of capturing a
/// clone of it, so a repaint clones no content for a press.
pub fn submit_settings_frame(settings: &SettingsState, sections: &Sections) {
    let Some(frame) = sections.settings.peek().clone() else {
        return;
    };
    settings.submit_settings_form(&frame.content);
}

/// The marks one choice-list row holds (`state.ts`'s `markedChoices` seed
/// read, shared by the reader and the toggle): the user's held marks for the
/// row's setting, else the frame's `selected` flags.
#[must_use]
fn seeded_or_held(
    marks: &[(String, Vec<String>)],
    key: &str,
    choices: Option<&[GuiSettingChoice]>,
) -> Vec<String> {
    match marks.iter().find(|(setting, _)| setting == key) {
        Some((_, ids)) => ids.clone(),
        None => choices
            .unwrap_or(&[])
            .iter()
            .filter(|choice| choice.selected)
            .map(|choice| choice.id.clone())
            .collect(),
    }
}

/// The setup form a content value carries, or `None` (`state.ts`'s inline
/// read of `content.kind === 'services' ? content.setup : null`).
#[must_use]
fn frame_setup_content(content: &GuiSettingsContent) -> Option<&GuiSettingsSetup> {
    match content {
        GuiSettingsContent::Services { setup, .. } => setup.as_ref(),
        _ => None,
    }
}

/// Whether Space activates the focused content entry (`state.ts`'s
/// `spaceActivates`, design D11): a toggle's flip or a choice mark's toggle.
/// Every other entry leaves Space to the core, where it plays or pauses (the
/// spec's *Global keys keep working behind the screen*).
#[must_use]
pub fn space_activates(entry: Option<&SettingsEntry>) -> bool {
    match entry {
        Some(SettingsEntry::Setting { row, .. }) => row.kind == GuiSettingRowKind::Toggle,
        Some(SettingsEntry::Choice { .. }) => true,
        _ => false,
    }
}

mod forms;
mod sync;
