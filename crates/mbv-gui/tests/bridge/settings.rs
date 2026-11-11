//! Public-API tests of the `settings` command's wire translation
//! (`add-gui-settings-screen` design D5): one variant of each payload shape —
//! a unit variant, a key name, a nested target, and a `DomKey` — so the wire
//! vocabulary's four kinds of field all reach their neutral form.

use mbv_gui::bridge::settings::{GuiSettingsCommand, neutral};
use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};
use mbv_ui_model::settings::SettingKey;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// The unit payload shape: `close` carries no field.
#[test]
fn the_close_command_carries_no_payload() {
    let command: GuiSettingsCommand =
        serde_json::from_str(r#"{"command":"close"}"#).expect("the wire command parses");

    assert_eq!(neutral(command), Some(mbv::GuiSettingsCommand::Close));
}

/// The key-name payload shape: `activate` carries the row's stable setting
/// name, which resolves to the `SettingKey` the App activates.
#[test]
fn the_activate_command_names_its_setting_key() {
    let command: GuiSettingsCommand =
        serde_json::from_str(r#"{"command":"activate","setting":"consumeAudio"}"#)
            .expect("the wire command parses");

    assert_eq!(
        neutral(command),
        Some(mbv::GuiSettingsCommand::Activate {
            setting: SettingKey::ConsumeAudio
        })
    );
}

/// A setting name this build declares no key for resolves to no command: the
/// frame that carried it is stale, so the edit changes nothing.
#[test]
fn an_unknown_setting_name_resolves_to_no_command() {
    let command: GuiSettingsCommand =
        serde_json::from_str(r#"{"command":"activate","setting":"noSuchSetting"}"#)
            .expect("the wire command parses");

    assert_eq!(neutral(command), None);
}

/// The nested-target payload shape: `selectTarget` carries the chooser's
/// stable target key, and the local output is the unit kind inside it.
#[test]
fn the_select_target_command_carries_the_nested_target_key() {
    let command: GuiSettingsCommand = serde_json::from_str(
        r#"{"command":"selectTarget","target":{"kind":"emby","id":"session-2"}}"#,
    )
    .expect("the wire command parses");
    assert_eq!(
        neutral(command),
        Some(mbv::GuiSettingsCommand::SelectTarget {
            target: ChooserTargetKey::Target(SessionTargetKey::Emby("session-2".to_string()))
        })
    );

    let command: GuiSettingsCommand =
        serde_json::from_str(r#"{"command":"selectTarget","target":{"kind":"local"}}"#)
            .expect("the wire command parses");
    assert_eq!(
        neutral(command),
        Some(mbv::GuiSettingsCommand::SelectTarget {
            target: ChooserTargetKey::Local
        })
    );
}

/// The `DomKey` payload shape: `keysCapture` carries the raw DOM key, and the
/// bridge translates it to the neutral key the router normalizes.
#[test]
fn the_keys_capture_command_translates_its_dom_key() {
    let command: GuiSettingsCommand = serde_json::from_str(
        r#"{"command":"keysCapture","target":"settings_open",
            "key":{"key":"F3","code":"F3","metaKey":false,"repeat":false,
                   "shiftKey":false,"ctrlKey":false,"altKey":false}}"#,
    )
    .expect("the wire command parses");

    assert_eq!(
        neutral(command),
        Some(mbv::GuiSettingsCommand::KeysCapture {
            target: "settings_open".to_string(),
            key: KeyEvent::new(Key::Function(3), KeyModifiers::NONE),
        })
    );
}
