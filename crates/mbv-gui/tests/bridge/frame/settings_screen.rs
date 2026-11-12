//! The Settings frame section's tests (`add-gui-settings-screen` design D9):
//! the closed screen carries no section, an open F2 section carries its rows
//! with their values, and a failed setup carries no password. Split from
//! `frame.rs` to keep that file under the line limit.

use mbv_gui::bridge::frame::settings::GuiSettingsContent;
use mbv_gui::bridge::frame::settings::rows::{GuiSettingRow, GuiSettingRowKind};
use mbv_ui_model::settings::SettingKey;

use super::frame_after_tick;

/// The Settings frame section is `None` while the screen is closed
/// (`add-gui-settings-screen` design D9): the window mounts the screen only for
/// a frame that carries it.
#[test]
fn a_closed_settings_screen_carries_no_frame_section() {
    let mut app = mbv::stub_app_with_queue(0);

    let frame = frame_after_tick(&mut app);

    assert_eq!(frame.settings(), None);
}

/// The F2 row of `rows` whose stable name is `key`.
fn f2_row(rows: &[GuiSettingRow], key: SettingKey) -> &GuiSettingRow {
    rows.iter()
        .find(|row| row.key == key.name())
        .expect("the section projects the row")
}

/// An open F2 section carries its rows with their values: the Queue toggles
/// their TUI `On`/`Off` text (`add-gui-settings-screen` design D9), and the
/// Session version rows are read-only `info` rows with the App's values
/// (`owner-gate-on-wire-identity` design D5).
#[test]
fn an_open_f2_section_carries_its_rows_with_values() {
    let mut app = mbv::stub_app_with_queue(0);
    app.open_gui_settings_at(mbv::GuiSettingsSection::Queue);

    let frame = frame_after_tick(&mut app);
    let settings = frame
        .settings()
        .expect("the open screen carries its section");

    assert_eq!(
        settings.section,
        mbv_gui::bridge::frame::settings::GuiSettingsSection::Queue
    );
    let mbv_gui::bridge::frame::settings::GuiSettingsContent::Rows { rows } =
        settings.content.clone()
    else {
        panic!("the Queue section carries its F2 rows")
    };
    let keys: Vec<&str> = rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "alwaysPlayNext",
            "consumeVideos",
            "consumeAudio",
            "savePlaylistOnConsume",
            "savePlaylistOnConsumeAudio",
        ]
    );
    for row in &rows {
        assert_eq!(
            row.kind,
            mbv_gui::bridge::frame::settings::rows::GuiSettingRowKind::Toggle,
            "the {} row is a toggle",
            row.key
        );
        assert!(row.flag.is_some(), "the {} row carries its flag", row.key);
        assert!(
            matches!(row.value.as_str(), "on" | "off"),
            "the {} row carries its TUI value text",
            row.key
        );
    }

    app.open_gui_settings_at(mbv::GuiSettingsSection::Session);
    let frame = frame_after_tick(&mut app);
    let Some(GuiSettingsContent::Rows { rows }) =
        frame.settings().map(|settings| settings.content.clone())
    else {
        panic!("the Session section carries its F2 rows")
    };
    let client = f2_row(&rows, SettingKey::ClientVersion);
    let owner = f2_row(&rows, SettingKey::OwnerVersion);
    assert_eq!(client.kind, GuiSettingRowKind::Info);
    assert_eq!(
        client.value,
        app.version_row_value(SettingKey::ClientVersion)
    );
    assert_eq!(owner.kind, GuiSettingRowKind::Info);
    assert_eq!(owner.value, app.version_row_value(SettingKey::OwnerVersion));
}

/// A Services frame after a failed Emby setup carries the error and the server
/// URL and no password (`add-gui-settings-screen` design D9, the spec's
/// *A failed setup does not echo the password*): the whole frame serializes
/// with neither the typed secret nor a field that could hold one.
#[test]
fn a_failed_emby_setup_frame_carries_no_password() {
    let mut app = mbv::stub_app_with_failed_emby_setup();

    let frame = frame_after_tick(&mut app);
    let settings = frame
        .settings()
        .expect("the open screen carries its section");
    let mbv_gui::bridge::frame::settings::GuiSettingsContent::Services { services, setup } =
        &settings.content
    else {
        panic!("the Services section carries the Services content")
    };
    assert_eq!(services.len(), 3);
    let setup = setup.as_ref().expect("the failed form rides the frame");
    assert_eq!(setup.server_url, "https://emby.example.test");
    assert_eq!(setup.error, "Invalid username or password");
    assert!(!setup.busy);

    let json = serde_json::to_value(&frame.0).expect("the frame serializes");
    let setup = &json["settings"]["value"]["content"]["setup"];
    let mut fields: Vec<&str> = setup
        .as_object()
        .expect("the setup form is an object")
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["busy", "error", "serverUrl", "service"],
        "no setup field can hold a password"
    );
    assert!(
        !json.to_string().contains("hunter2"),
        "no frame carries the typed password"
    );
}
