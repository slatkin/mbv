//! Headless tests of the F2-row sections' pane (D16): the paint contracts of
//! `SettingsRows.test.ts` — a toggle row paints a switch, a pick row paints
//! its pick button, an info row paints its value with no control, the action
//! rows paint buttons, a choice list paints its checkbox-grid card with the
//! frame's marks and its Apply, and the content cursor paints on the box it
//! rests on. The press contracts of the former file — a press sends the
//! row's own command and moves the content cursor onto it — ride the
//! `press_settings_entry` handler the pane's controls call one line each, and
//! the paint's `data-settings-entry` ids pin the controls to the entries that
//! handler resolves. The core channel is a mock pair; no test opens a
//! window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsRows;
use crate::bridge::frame::settings::rows::{GuiSettingChoice, GuiSettingRow, GuiSettingRowKind};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::sections::{SectionRef, Sections};
use crate::window::settings::entries::{entry_id, settings_entries};
use crate::window::settings::state::{SettingsPane, SettingsState};

/// What the root hands the test: the section stores and the Settings store.
type FixtureHandles = (Sections, SettingsState);

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides them, and paints the rows
/// pane.
fn rows_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let state = use_hook(|| SettingsState::new(CoreLink::new(props.sender)));
    provide_context(state.clone());
    props.handle.borrow_mut().replace((sections, state));
    rsx! {
        SettingsRows {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    state: SettingsState,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// Write the Settings section's frame and fold it into the interaction
    /// state, as the drain task and the root's sync do.
    fn open(&mut self, frame: GuiSettingsFrame) {
        let mut settings = self.sections.settings;
        let carried = frame.clone();
        self.doc
            .vdom
            .in_runtime(move || settings.set(Some(SectionRef::new(carried))));
        let state = self.state.clone();
        self.doc
            .vdom
            .in_runtime(move || state.sync_settings_frame(Some(&frame)));
        self.poll_until_idle();
    }

    /// Put the keyboard on the content pane's `index` entry, as the key
    /// router's movements do.
    fn cursor_at(&mut self, index: usize) {
        let state = self.state.clone();
        self.doc.vdom.in_runtime(move || {
            let mut ui = state.ui;
            ui.with_mut(|ui| {
                ui.pane = SettingsPane::Content;
                ui.content_cursor = index;
                ui.keyboard = true;
            });
        });
        self.poll_until_idle();
    }

    /// The Settings commands the core link received, in send order.
    fn settings_commands(&self) -> Vec<crate::gui_core::GuiSettingsCommand> {
        self.receiver
            .try_iter()
            .filter_map(|input| match input {
                CoreInput::Settings(command) => Some(command),
                _ => None,
            })
            .collect()
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: LocalName) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(attribute)
            .map(str::to_owned)
    }

    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The trimmed text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
            .trim()
            .to_owned()
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }
}

/// A toggle row and a choice-list row, as the frame carries them.
fn rows_content() -> GuiSettingsContent {
    GuiSettingsContent::Rows {
        rows: vec![
            GuiSettingRow {
                key: "consumeAudio".to_owned(),
                label: "Consume audio".to_owned(),
                kind: GuiSettingRowKind::Toggle,
                value: "Off".to_owned(),
                flag: Some(false),
                choices: None,
                choices_kind: None,
                values: None,
            },
            GuiSettingRow {
                key: "hiddenLibraries".to_owned(),
                label: "Hidden libraries".to_owned(),
                kind: GuiSettingRowKind::ChoiceList,
                value: String::new(),
                flag: None,
                choices: Some(vec![
                    GuiSettingChoice {
                        id: "music".to_owned(),
                        label: "Music".to_owned(),
                        selected: false,
                    },
                    GuiSettingChoice {
                        id: "shows".to_owned(),
                        label: "Shows".to_owned(),
                        selected: true,
                    },
                ]),
                choices_kind: Some(crate::bridge::settings::GuiChoicesKind::HiddenLibraries),
                values: None,
            },
        ],
    }
}

/// A pick row over three values, the empty one painted as Any.
fn pick_content() -> GuiSettingsContent {
    GuiSettingsContent::Rows {
        rows: vec![GuiSettingRow {
            key: "subtitleLanguage".to_owned(),
            label: "Subtitle language".to_owned(),
            kind: GuiSettingRowKind::Pick,
            value: "English".to_owned(),
            flag: None,
            choices: None,
            choices_kind: None,
            values: Some(vec![
                String::new(),
                "English".to_owned(),
                "French".to_owned(),
            ]),
        }],
    }
}

/// The built rows pane over the carried content.
fn built(content: GuiSettingsContent) -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            rows_app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let (sections, state) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the stores over");
    let mut fixture = Fixture {
        doc,
        sections,
        state,
        receiver,
    };
    fixture.open(GuiSettingsFrame {
        section: GuiSettingsSection::Playback,
        sections: vec![GuiSettingsSection::Playback],
        content,
    });
    fixture
}

/// Contract (`SettingsRows.test.ts`, the markup): a toggle row paints a
/// switch carrying the row's label and its flag.
#[test]
fn a_toggle_row_paints_a_switch_with_its_label_and_flag() {
    let fixture = built(rows_content());

    assert_eq!(
        fixture.attr_of("[data-settings-switch]", LocalName::from("aria-label")),
        Some("Consume audio".to_owned()),
        "the switch names the row"
    );
    assert_eq!(
        fixture.attr_of("[data-settings-switch]", LocalName::from("aria-checked")),
        Some("false".to_owned()),
        "the switch carries the row's flag"
    );
}

/// Contract (`SettingsRows.test.ts`, the markup): a pick row paints its pick
/// button over the row's setting, with the value it shows.
#[test]
fn a_pick_row_paints_its_pick_button() {
    let fixture = built(pick_content());

    assert!(
        fixture.presents("[data-settings-pick=\"subtitleLanguage\"] button"),
        "the pick button paints in the row"
    );
    assert_eq!(
        fixture.attr_of(
            "[data-settings-pick=\"subtitleLanguage\"] button",
            LocalName::from("aria-expanded")
        ),
        Some("false".to_owned()),
        "the closed menu says so"
    );
}

/// Contract (`SettingsRows.test.ts`): an info row paints its label and value
/// with no control — no button and no switch paints (owner-gate-on-wire-
/// identity design D5).
#[test]
fn an_info_row_paints_its_value_with_no_control() {
    let content = GuiSettingsContent::Rows {
        rows: vec![GuiSettingRow {
            key: "clientVersion".to_owned(),
            label: "This client".to_owned(),
            kind: GuiSettingRowKind::Info,
            value: "1.2.3 · abc · def".to_owned(),
            flag: None,
            choices: None,
            choices_kind: None,
            values: None,
        }],
    };
    let fixture = built(content);

    assert_eq!(
        fixture.text_of("[data-settings-info=\"clientVersion\"]"),
        "1.2.3 · abc · def",
        "the value paints"
    );
    assert!(
        !fixture.presents("[data-settings-rows] button")
            && !fixture.presents("[data-settings-rows] [role=\"switch\"]"),
        "no control paints"
    );
}

/// Contract (`SettingsRows.test.ts`, the markup): the choice list paints its
/// checkbox-grid card with the frame's marks and its Apply.
#[test]
fn a_choice_list_paints_its_grid_with_the_frame_marks_and_its_apply() {
    let content = rows_content();
    let fixture = built(content.clone());

    assert_eq!(
        fixture.attr_of(
            "[data-settings-checkbox=\"music\"]",
            LocalName::from("aria-checked")
        ),
        Some("false".to_owned()),
        "the unselected box says so"
    );
    assert_eq!(
        fixture.attr_of(
            "[data-settings-checkbox=\"shows\"]",
            LocalName::from("aria-checked")
        ),
        Some("true".to_owned()),
        "the frame's selected box says so"
    );
    assert!(
        fixture.presents(&format!(
            "[data-settings-entry=\"{}\"]",
            entry_id::choices_apply("hiddenLibraries")
        )),
        "the Apply paints"
    );
}

/// Contract (`SettingsRows.test.ts`): the cursor paints on the box it rests
/// on, with the rows' cursor fact.
#[test]
fn the_cursor_paints_on_the_box_it_rests_on() {
    let content = rows_content();
    // The entries: the consumeAudio row, the music mark, the shows mark, the
    // Apply control. The music mark is entry 1.
    let entries = settings_entries(&content, &[]);
    let index = entries
        .iter()
        .position(|entry| {
            crate::window::settings::state::entry_id_string(entry)
                == entry_id::choice("hiddenLibraries", "music")
        })
        .expect("the fixture names a painted entry");
    let mut fixture = built(content);
    fixture.cursor_at(index);

    assert!(
        fixture.presents("[data-settings-checkbox=\"music\"][data-settings-cursor]"),
        "the box under the cursor paints the cursor"
    );
}

/// Contract (`SettingsRows.test.ts`): a press on the toggle row's switch
/// sends `activate` and moves the content cursor onto the row. The press
/// rides `press_settings_entry`, the handler the switch's `onpress` calls
/// with the entry id the pane paints.
#[test]
fn a_press_on_the_toggle_row_sends_activate_and_moves_the_cursor() {
    let content = rows_content();
    let mut fixture = built(content.clone());
    let entries = settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let id = entry_id::setting("consumeAudio");
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &id));
    fixture.poll_until_idle();

    assert_eq!(
        fixture.settings_commands(),
        vec![crate::gui_core::GuiSettingsCommand::Activate {
            setting: mbv_ui_model::settings::SettingKey::from_name("consumeAudio")
                .expect("the fixture names a declared setting"),
        }],
        "the press sent the row's activate"
    );
    let ui = fixture.state.ui.peek().clone();
    assert_eq!(
        ui.pane,
        SettingsPane::Content,
        "the keyboard took the content"
    );
    assert_eq!(ui.content_cursor, 0, "the cursor moved onto the row");
}

/// Contract (`SettingsRows.test.ts`): a press on the pick button moves the
/// content cursor onto its row and opens its value menu, with no send.
#[test]
fn a_press_on_the_pick_row_opens_its_menu_with_no_send() {
    let content = pick_content();
    let mut fixture = built(content.clone());
    let entries = settings_entries(&content, &[]);

    let state = fixture.state.clone();
    let id = entry_id::setting("subtitleLanguage");
    fixture
        .doc
        .vdom
        .in_runtime(move || state.press_settings_entry(&entries, &id));
    fixture.poll_until_idle();

    assert_eq!(
        fixture
            .state
            .pick_menu
            .peek()
            .as_ref()
            .map(|menu| menu.setting.as_str()),
        Some("subtitleLanguage"),
        "the row's value menu opened"
    );
    assert!(
        fixture.settings_commands().is_empty(),
        "opening a menu sends nothing"
    );
}
