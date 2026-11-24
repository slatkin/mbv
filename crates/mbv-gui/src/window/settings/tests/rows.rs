//! The F2-row sections and the Library routes list (the contracts of
//! `rows.test.ts`): Space on a toggle sends `activate` with the row's key
//! name; applying a choice list sends `applyChoices` with the marked ids and
//! the row-carried kind; selecting a library in the routes list sends
//! `routeDevices`, and assigning a device sends `assignRoute`.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::library::{GuiSettingsRoute, GuiSettingsRouteDevice};
use crate::bridge::frame::settings::rows::GuiSettingRow;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::bridge::settings::GuiChoicesKind;
use crate::window::settings::entries::{entry_id, settings_entries};
use crate::window::settings::state::{SettingsPane, SettingsState};

use super::{Facts, choice, frame, press, run, settings_commands, toggle_row};

/// Put the keyboard on the content pane's `index` entry, as the TS
/// `cursorAt` does.
fn cursor_at(state: &SettingsState, section: GuiSettingsSection, index: usize) {
    let mut ui = state.ui;
    ui.with_mut(|ui| {
        ui.pane = SettingsPane::Content;
        ui.content_cursor = index;
        ui.section = Some(section);
    });
}

fn rows_frame(rows: Vec<GuiSettingRow>) -> GuiSettingsFrame {
    frame(
        GuiSettingsSection::Queue,
        &[
            GuiSettingsSection::Services,
            GuiSettingsSection::Queue,
            GuiSettingsSection::Keys,
        ],
        GuiSettingsContent::Rows { rows },
    )
}

/// Contract (`rows.test.ts`): Space on a toggle sends activate with the row
/// key.
#[test]
fn space_on_a_toggle_sends_activate_with_the_row_key() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        let frame = rows_frame(vec![toggle_row("consumeAudio", "Consume audio", true)]);
        cursor_at(state, GuiSettingsSection::Queue, 0);
        press(state, facts, "space", " ", &frame);
    });
    assert_eq!(facts, vec!["space=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::Activate {
            setting: mbv_ui_model::settings::SettingKey::from_name("consumeAudio")
                .expect("the fixture names a declared setting"),
        }],
    );
}

/// Contract (`rows.test.ts`): applying a choice list sends applyChoices with
/// the row-carried kind. The row's key is a different choice list's name, so
/// a kind derived from the key would send `myLanguages` instead. Space marks
/// a choice and sends nothing; the Apply entry's Enter commits.
#[test]
fn applying_a_choice_list_sends_apply_choices_with_the_row_carried_kind() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        let row = GuiSettingRow {
            key: "myLanguages".to_owned(),
            label: "Languages".to_owned(),
            kind: crate::bridge::frame::settings::rows::GuiSettingRowKind::ChoiceList,
            value: "eng".to_owned(),
            flag: None,
            choices: Some(vec![
                choice("eng", "English", true),
                choice("deu", "German", false),
            ]),
            choices_kind: Some(GuiChoicesKind::HiddenLibraries),
            values: None,
        };
        let frame = rows_frame(vec![row]);
        // Entries: the eng mark, the deu mark, the Apply control.
        cursor_at(state, GuiSettingsSection::Queue, 1);
        press(state, facts, "space", " ", &frame);
        cursor_at(state, GuiSettingsSection::Queue, 2);
        press(state, facts, "enter", "Enter", &frame);
        let marks = state.ui.peek().choice_marks.clone();
        facts
            .lock()
            .expect("facts")
            .push(format!("marks={marks:?}"));
    });
    assert_eq!(
        facts,
        vec![
            "space=Handled",
            "enter=Handled",
            "marks=[(\"myLanguages\", [\"eng\", \"deu\"])]"
        ]
    );
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::ApplyChoices {
            kind: mbv_ui_model::context_menu::MultiSelectKind::HiddenLibraries,
            selected: vec!["eng".to_owned(), "deu".to_owned()],
        }],
    );
}

/// A Library-section frame over the routes list.
fn library_frame(
    routes: Vec<GuiSettingsRoute>,
    selected: Option<&str>,
    devices: Option<Vec<GuiSettingsRouteDevice>>,
) -> GuiSettingsFrame {
    frame(
        GuiSettingsSection::Library,
        &[
            GuiSettingsSection::Services,
            GuiSettingsSection::Library,
            GuiSettingsSection::Keys,
        ],
        GuiSettingsContent::Library {
            rows: Vec::new(),
            routes: Some(routes),
            selected: selected.map(str::to_owned),
            devices,
        },
    )
}

fn route(library: &str, name: &str, endpoint: Option<&str>) -> GuiSettingsRoute {
    GuiSettingsRoute {
        library: library.to_owned(),
        name: name.to_owned(),
        endpoint: endpoint.map(str::to_owned),
    }
}

fn device(name: &str, endpoint: Option<&str>) -> GuiSettingsRouteDevice {
    GuiSettingsRouteDevice {
        name: name.to_owned(),
        endpoint: endpoint.map(str::to_owned),
    }
}

/// Contract (`rows.test.ts`): selecting a library in the routes list sends
/// routeDevices.
#[test]
fn selecting_a_library_in_the_routes_list_sends_route_devices() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        let frame = library_frame(
            vec![
                route("movies", "Movies", None),
                route("shows", "Shows", Some("tcp://tv")),
            ],
            None,
            None,
        );
        cursor_at(state, GuiSettingsSection::Library, 0);
        press(state, facts, "enter", "Enter", &frame);
    });
    assert_eq!(facts, vec!["enter=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::RouteDevices {
            library: "movies".to_owned(),
        }],
    );
}

/// Contract (`rows.test.ts`): assigning a device sends assignRoute with the
/// library and the endpoint.
#[test]
fn assigning_a_device_sends_assign_route_with_the_library_and_the_endpoint() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        // Entries: route, the automatic entry, the two devices.
        let frame = library_frame(
            vec![route("movies", "Movies", None)],
            Some("movies"),
            Some(vec![
                device("Living Room", Some("tcp://tv:8096")),
                device("Phone", None),
            ]),
        );
        cursor_at(state, GuiSettingsSection::Library, 2);
        press(state, facts, "enter", "Enter", &frame);
    });
    assert_eq!(facts, vec!["enter=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::AssignRoute {
            library: "movies".to_owned(),
            endpoint: Some("tcp://tv:8096".to_owned()),
        }],
    );
}

/// Contract (`rows.test.ts`): the automatic entry clears the route.
#[test]
fn the_automatic_entry_clears_the_route() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        let frame = library_frame(
            vec![route("movies", "Movies", Some("tcp://tv:8096"))],
            Some("movies"),
            Some(vec![device("Living Room", Some("tcp://tv:8096"))]),
        );
        cursor_at(state, GuiSettingsSection::Library, 1);
        press(state, facts, "enter", "Enter", &frame);
    });
    assert_eq!(facts, vec!["enter=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::AssignRoute {
            library: "movies".to_owned(),
            endpoint: None,
        }],
    );
}

/// Contract (`rows.test.ts`): a device with no endpoint assigns nothing.
#[test]
fn a_device_with_no_endpoint_assigns_nothing() {
    let (facts, rx) = run(|state: &SettingsState, facts: &Facts| {
        let frame = library_frame(
            vec![route("movies", "Movies", None)],
            Some("movies"),
            Some(vec![device("Phone", None)]),
        );
        cursor_at(state, GuiSettingsSection::Library, 2);
        press(state, facts, "enter", "Enter", &frame);
    });
    assert_eq!(facts, vec!["enter=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        [] as [crate::gui_core::GuiSettingsCommand; 0]
    );
}

/// The entries builder's Library arm keeps the routes list's paint order, so
/// the cursor indexes above address the rows the tests name.
#[test]
fn library_entries_paint_in_frame_order() {
    let content = GuiSettingsContent::Library {
        rows: Vec::new(),
        routes: Some(vec![route("movies", "Movies", None)]),
        selected: Some("movies".to_owned()),
        devices: Some(vec![device("Living Room", None)]),
    };
    let ids: Vec<String> = settings_entries(&content, &[])
        .iter()
        .map(crate::window::settings::state::entry_id_string)
        .collect();
    assert_eq!(
        ids,
        vec![
            entry_id::route("movies"),
            entry_id::route_clear("movies"),
            entry_id::route_device("movies", "Living Room"),
        ]
    );
}
