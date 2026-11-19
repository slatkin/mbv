//! Headless tests of the Settings screen's window-content logic (D16): the
//! contracts of `rows.test.ts`, `keys.test.ts`, `feeds.test.ts`, and
//! `playlists.test.ts` under `lib/settings/`, plus the pick-menu key
//! contracts of `SettingsPickButton.test.ts`. The harness builds a
//! `DioxusDocument` whose root creates the store and runs each scenario in
//! one effect over a mock core link, so no test opens a window or sleeps,
//! and no IPC runs. The asserted commands are the neutral shapes the core
//! thread receives.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::settings::rows::{GuiSettingChoice, GuiSettingRow, GuiSettingRowKind};
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::bridge::keys::{DomKey, DomModifiers, KeyTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::settings::state::SettingsState;

mod feeds;
mod keys;
mod rows;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&SettingsState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| SettingsState::new(CoreLink::new(tx)));
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "settings-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded
/// and the commands the core link received. Bounded by construction: the
/// effect runs once and the poll loop stops when the document goes idle.
fn run(
    scenario: impl Fn(&SettingsState, &Facts) + 'static,
) -> (Vec<String>, mpsc::Receiver<CoreInput>) {
    let (tx, rx) = mpsc::channel::<CoreInput>();
    let facts: Facts = Arc::new(Mutex::new(Vec::new()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            harness,
            (
                tx,
                Arc::clone(&facts),
                std::rc::Rc::new(move |state: &SettingsState, recorded: &Facts| {
                    scenario(state, recorded);
                }),
            ),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let recorded = facts.lock().expect("facts").clone();
    (recorded, rx)
}

/// Poll the document until its effects have run and their re-renders landed.
fn poll_until_idle(doc: &mut DioxusDocument) {
    for _ in 0..8 {
        if !doc.poll(None) {
            return;
        }
    }
    panic!("document did not become idle");
}

/// The Settings commands the scenario sent, as the core thread receives them.
fn settings_commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<mbv::GuiSettingsCommand> {
    let mut commands = Vec::new();
    while let Ok(input) = rx.try_recv() {
        if let CoreInput::Settings(command) = input {
            commands.push(command);
        }
    }
    commands
}

/// One keydown as the DOM names it.
fn dom(value: &str) -> DomKey {
    DomKey {
        key: value.to_owned(),
        code: String::new(),
        meta_key: false,
        repeat: false,
        modifiers: DomModifiers {
            shift_key: false,
            ctrl_key: false,
            alt_key: false,
        },
    }
}

/// Route one keydown against the frame and record the outcome.
fn press(state: &SettingsState, facts: &Facts, name: &str, value: &str, frame: &GuiSettingsFrame) {
    let outcome = super::keys::route_settings_key(&dom(value), frame, state, KeyTarget::None);
    facts
        .lock()
        .expect("facts")
        .push(format!("{name}={outcome:?}"));
}

/// A settings frame over `section` with the sidebar's full section list and
/// the carried content.
fn frame(
    section: GuiSettingsSection,
    sections: &[GuiSettingsSection],
    content: GuiSettingsContent,
) -> GuiSettingsFrame {
    GuiSettingsFrame {
        section,
        sections: sections.to_vec(),
        content,
    }
}

/// A toggle row as the frame carries one.
fn toggle_row(key: &str, label: &str, flag: bool) -> GuiSettingRow {
    GuiSettingRow {
        key: key.to_owned(),
        label: label.to_owned(),
        kind: GuiSettingRowKind::Toggle,
        value: if flag { "on" } else { "off" }.to_owned(),
        flag: Some(flag),
        choices: None,
        choices_kind: None,
        values: None,
    }
}

/// One choice of a choice-list row.
fn choice(id: &str, label: &str, selected: bool) -> GuiSettingChoice {
    GuiSettingChoice {
        id: id.to_owned(),
        label: label.to_owned(),
        selected,
    }
}
