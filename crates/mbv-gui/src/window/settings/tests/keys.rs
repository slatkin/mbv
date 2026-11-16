//! The Settings screen's key router (the contracts of `keys.test.ts`, plus
//! the pick-menu key contracts of `SettingsPickButton.test.ts`): an active
//! capture swallows its key and sends `keysCapture`, Esc cancels a capture
//! without a send, Esc walks the pane ladder (content to sidebar, then
//! `close`), Space on the sidebar stays the core's play/pause, Enter on a
//! pick row opens its menu, and the open menu's Enter sends `setValue` while
//! its Esc closes with no send.

use dioxus_native::prelude::*;
use mbv_ui_msg::{Key, KeyModifiers};

use crate::bridge::frame::settings::rows::GuiSettingRow;
use crate::bridge::frame::settings::services::GuiSettingsSetup;
use crate::bridge::frame::settings::{GuiSettingsContent, GuiSettingsFrame, GuiSettingsSection};
use crate::window::settings::state::{SettingsPane, SettingsState};

use super::{Facts, frame, press, run, settings_commands, toggle_row};

/// A settings frame over the Queue section: the sidebar's full section list
/// and a two-row content.
fn settings() -> GuiSettingsFrame {
    frame(
        GuiSettingsSection::Queue,
        &[
            GuiSettingsSection::Services,
            GuiSettingsSection::Playback,
            GuiSettingsSection::Queue,
            GuiSettingsSection::Actions,
            GuiSettingsSection::Keys,
            GuiSettingsSection::Sessions,
            GuiSettingsSection::Playlists,
        ],
        GuiSettingsContent::Rows {
            rows: vec![
                toggle_row("consumeAudio", "Consume audio", true),
                toggle_row("alwaysPlayNext", "Always play next", false),
            ],
        },
    )
}

/// A settings frame over the Playback section with one pick row.
fn pick_settings() -> GuiSettingsFrame {
    frame(
        GuiSettingsSection::Playback,
        &[GuiSettingsSection::Playback],
        GuiSettingsContent::Rows {
            rows: vec![GuiSettingRow {
                key: "subtitleLanguage".to_owned(),
                label: "Subtitle language".to_owned(),
                kind: crate::bridge::frame::settings::rows::GuiSettingRowKind::Pick,
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
        },
    )
}

/// Contract (`keys.test.ts`): a capture swallows F2 and sends keysCapture —
/// F2 is its candidate, not the core's settings toggle, and the sent command
/// carries the capture target and the raw DOM key.
#[test]
fn a_capture_swallows_f2_and_sends_keys_capture() {
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        state.start_keys_capture("next_track");
        press(state, facts, "f2", "F2", &settings());
    });
    assert_eq!(facts, vec!["f2=Handled"]);
    assert_eq!(
        settings_commands(&rx),
        vec![mbv::GuiSettingsCommand::KeysCapture {
            target: "next_track".to_owned(),
            key: mbv_ui_msg::KeyEvent::new(Key::Function(2), KeyModifiers::NONE),
        }],
    );
}

/// Contract (`keys.test.ts`): Esc cancels a capture and sends nothing.
#[test]
fn esc_cancels_a_capture_and_sends_nothing() {
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        state.start_keys_capture("next_track");
        press(state, facts, "escape", "Escape", &settings());
        let held = state.ui.peek().capture.is_none();
        facts
            .lock()
            .expect("facts")
            .push(format!("captureCleared={held}"));
    });
    assert_eq!(facts, vec!["escape=Handled", "captureCleared=true"]);
    assert_eq!(settings_commands(&rx), [] as [mbv::GuiSettingsCommand; 0]);
}

/// Contract (`keys.test.ts`): the Esc ladder — content hands the keyboard
/// back to the sidebar, and the sidebar's Esc closes the screen.
#[test]
fn esc_moves_content_to_the_sidebar_then_sends_close() {
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        let mut ui = state.ui;
        ui.with_mut(|ui| {
            ui.pane = SettingsPane::Content;
            ui.section = Some(GuiSettingsSection::Queue);
        });
        press(state, facts, "esc1", "Escape", &settings());
        let pane = state.ui.peek().pane;
        facts.lock().expect("facts").push(format!("pane={pane:?}"));
        press(state, facts, "esc2", "Escape", &settings());
    });
    assert_eq!(facts, vec!["esc1=Handled", "pane=Sidebar", "esc2=Handled"],);
    assert_eq!(settings_commands(&rx), vec![mbv::GuiSettingsCommand::Close],);
}

/// Contract (`keys.test.ts`): Space on the sidebar goes to the core.
#[test]
fn space_on_the_sidebar_goes_to_the_core() {
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        let mut ui = state.ui;
        ui.with_mut(|ui| {
            ui.pane = SettingsPane::Sidebar;
            ui.section = Some(GuiSettingsSection::Queue);
        });
        press(state, facts, "space", " ", &settings());
    });
    assert_eq!(facts, vec!["space=Core(None)"]);
    assert_eq!(settings_commands(&rx), [] as [mbv::GuiSettingsCommand; 0]);
}

/// Contract (`keys.test.ts`): Enter on a pick row opens its value list (the
/// spec's *Value settings are picked from their list*) and sends nothing;
/// the menu's own choice sends `setValue`.
#[test]
fn enter_on_a_pick_row_opens_its_menu_and_sends_nothing() {
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        let mut ui = state.ui;
        ui.with_mut(|ui| ui.pane = SettingsPane::Content);
        press(state, facts, "enter", "Enter", &pick_settings());
        let menu = state.pick_menu.peek().clone();
        facts
            .lock()
            .expect("facts")
            .push(format!("menu={:?}", menu.map(|menu| menu.setting)));
    });
    assert_eq!(
        facts,
        vec!["enter=Handled", "menu=Some(\"subtitleLanguage\")"]
    );
    assert_eq!(settings_commands(&rx), [] as [mbv::GuiSettingsCommand; 0]);
}

/// Contract (`keys.test.ts`): Esc from the sidebar closes the screen even
/// with a setup form open — the form's cancel steps belong to the content
/// pane (the spec's *Esc closes the screen from the sidebar*).
#[test]
fn esc_from_the_sidebar_with_a_setup_form_open_sends_close_not_cancel_setup() {
    let setup_frame = frame(
        GuiSettingsSection::Services,
        &[GuiSettingsSection::Services],
        GuiSettingsContent::Services {
            services: Vec::new(),
            setup: Some(GuiSettingsSetup {
                service: crate::bridge::frame::settings::services::GuiSetupService::Emby,
                server_url: String::new(),
                busy: false,
                error: String::new(),
            }),
        },
    );
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        press(state, facts, "escape", "Escape", &setup_frame);
    });
    assert_eq!(facts, vec!["escape=Handled"]);
    assert_eq!(settings_commands(&rx), vec![mbv::GuiSettingsCommand::Close],);
}

/// Contract (`SettingsPickButton.test.ts`): Enter on a menu entry sends
/// setValue with that value, and the menu closes.
#[test]
fn enter_on_a_menu_entry_sends_set_value_with_that_value() {
    let values = vec![String::new(), "English".to_owned(), "French".to_owned()];
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        state.open_pick_menu("subtitleLanguage", "English", &values);
        press(state, facts, "down", "ArrowDown", &pick_settings());
        press(state, facts, "enter", "Enter", &pick_settings());
        let open = state.pick_menu.peek().is_some();
        facts
            .lock()
            .expect("facts")
            .push(format!("menuOpen={open}"));
    });
    assert_eq!(
        facts,
        vec!["down=Handled", "enter=Handled", "menuOpen=false"]
    );
    assert_eq!(
        settings_commands(&rx),
        vec![mbv::GuiSettingsCommand::SetValue {
            setting: mbv_ui_model::settings::SettingKey::from_name("subtitleLanguage")
                .expect("the fixture names a declared setting"),
            value: "French".to_owned(),
        }],
    );
}

/// Contract (`SettingsPickButton.test.ts`): Esc closes the menu and sends
/// nothing.
#[test]
fn esc_closes_the_menu_and_sends_nothing() {
    let values = vec![String::new(), "English".to_owned(), "French".to_owned()];
    let (facts, rx) = run(move |state: &SettingsState, facts: &Facts| {
        state.open_pick_menu("subtitleLanguage", "English", &values);
        press(state, facts, "escape", "Escape", &pick_settings());
        let open = state.pick_menu.peek().is_some();
        facts
            .lock()
            .expect("facts")
            .push(format!("menuOpen={open}"));
    });
    assert_eq!(facts, vec!["escape=Handled", "menuOpen=false"]);
    assert_eq!(settings_commands(&rx), [] as [mbv::GuiSettingsCommand; 0]);
}
