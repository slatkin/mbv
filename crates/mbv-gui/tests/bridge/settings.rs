//! Public-API tests of the `settings` command's translation
//! (`add-gui-settings-screen` design D5, `replace-tauri-with-blitz` D18): one
//! request of each payload shape — a unit variant, a key name, a nested
//! target, and a `DomKey` — so the vocabulary's four kinds of field all reach
//! their neutral form. The window builds these values as Rust; the wire form
//! the window sent went at the switch.

use mbv_gui::bridge::frame::playback::GuiTargetKey;
use mbv_gui::bridge::keys::{DomKey, DomModifiers};
use mbv_gui::bridge::settings::{GuiSettingsCommand, neutral};
use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};
use mbv_ui_model::settings::SettingKey;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// The unit payload shape: `close` carries no field.
#[test]
fn the_close_command_carries_no_payload() {
    assert_eq!(
        neutral(GuiSettingsCommand::Close),
        Some(mbv_gui::gui_core::GuiSettingsCommand::Close)
    );
}

/// The key-name payload shape: `activate` carries the row's stable setting
/// name, which resolves to the `SettingKey` the App activates.
#[test]
fn the_activate_command_names_its_setting_key() {
    let command = GuiSettingsCommand::Activate {
        setting: "consumeAudio".to_string(),
    };

    assert_eq!(
        neutral(command),
        Some(mbv_gui::gui_core::GuiSettingsCommand::Activate {
            setting: SettingKey::ConsumeAudio
        })
    );
}

/// A setting name this build declares no key for resolves to no command: the
/// frame that carried it is stale, so the edit changes nothing.
#[test]
fn an_unknown_setting_name_resolves_to_no_command() {
    let command = GuiSettingsCommand::Activate {
        setting: "noSuchSetting".to_string(),
    };

    assert_eq!(neutral(command), None);
}

/// The nested-target payload shape: `selectTarget` carries the chooser's
/// stable target key, and the local output is the unit kind inside it.
#[test]
fn the_select_target_command_carries_the_nested_target_key() {
    let command = GuiSettingsCommand::SelectTarget {
        target: GuiTargetKey::Emby("session-2".to_string()),
    };
    assert_eq!(
        neutral(command),
        Some(mbv_gui::gui_core::GuiSettingsCommand::SelectTarget {
            target: ChooserTargetKey::Target(SessionTargetKey::Emby("session-2".to_string()))
        })
    );

    let command = GuiSettingsCommand::SelectTarget {
        target: GuiTargetKey::Local,
    };
    assert_eq!(
        neutral(command),
        Some(mbv_gui::gui_core::GuiSettingsCommand::SelectTarget {
            target: ChooserTargetKey::Local
        })
    );
}

/// The `DomKey` payload shape: `keysCapture` carries the raw DOM key, and the
/// bridge translates it to the neutral key the router normalizes.
#[test]
fn the_keys_capture_command_translates_its_dom_key() {
    let command = GuiSettingsCommand::KeysCapture {
        target: "settings_open".to_string(),
        key: DomKey {
            key: "F3".to_string(),
            code: "F3".to_string(),
            meta_key: false,
            repeat: false,
            modifiers: DomModifiers::default(),
        },
    };

    assert_eq!(
        neutral(command),
        Some(mbv_gui::gui_core::GuiSettingsCommand::KeysCapture {
            target: "settings_open".to_string(),
            key: KeyEvent::new(Key::Function(3), KeyModifiers::NONE),
        })
    );
}

/// The library-name payload shape: `routeDevices` carries the routes list's
/// selected library name, which is the stable identity the core loads devices
/// for (design D5, design D9).
#[test]
fn the_route_devices_command_names_its_library() {
    let command = GuiSettingsCommand::RouteDevices {
        library: "music".to_string(),
    };

    assert_eq!(
        neutral(command),
        Some(mbv_gui::gui_core::GuiSettingsCommand::RouteDevices {
            library: "music".to_string()
        })
    );
}
