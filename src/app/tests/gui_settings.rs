//! The Settings screen's core state (add-gui-settings-screen rows 3.1): the
//! open actions' toggle rule (`add-gui-settings-screen` design D2, the spec's
//! open-action requirement) and the section rows the scope filter shows.

use super::*;
use crate::app::gui_settings::{GuiSettingsGroup, GuiSettingsSection};
use rstest::rstest;

/// The toggle rule the open actions follow (design D2): with the screen
/// closed the action opens it at the group's section; with the screen showing
/// a section of the same group the action closes it; otherwise the screen
/// stays open and shows the group's section. One `#[case]` per outcome:
/// (the section shown before, the toggled group, the section after — `None`
/// closed).
#[rstest]
#[case::open_from_closed(None, GuiSettingsGroup::F2, Some(GuiSettingsSection::Services))]
#[case::the_same_group_closes(Some(GuiSettingsSection::Keys), GuiSettingsGroup::F2, None)]
#[case::another_group_switches(
    Some(GuiSettingsSection::Services),
    GuiSettingsGroup::Playlists,
    Some(GuiSettingsSection::Playlists)
)]
fn the_open_actions_follow_the_toggle_rule(
    #[case] shown: Option<GuiSettingsSection>,
    #[case] group: GuiSettingsGroup,
    #[case] expected: Option<GuiSettingsSection>,
) {
    let mut app = make_app_stub();
    if let Some(section) = shown {
        app.open_gui_settings_at(section);
    }

    app.toggle_gui_settings(group);

    assert_eq!(app.gui_settings().map(|state| state.section), expected);
}

/// A setup secret never reaches a log line (design D9, the spec's *A failed
/// setup does not echo the password*): the log sites format the whole command
/// and the GUI Client's sink is a file, so the command's `Debug` form masks the
/// password and the API key. One `#[case]` per setup form.
#[rstest]
#[case::emby(
    GuiSettingsCommand::EmbySetup {
        server_url: "http://emby.test".to_owned(),
        username: "ada".to_owned(),
        password: "hunter2".to_owned(),
    },
    "hunter2"
)]
#[case::audiobookshelf(
    GuiSettingsCommand::AudiobookshelfSetup {
        server_url: "http://abs.test".to_owned(),
        api_key: "abs-key-42".to_owned(),
    },
    "abs-key-42"
)]
fn the_setup_commands_mask_their_secret(#[case] command: GuiSettingsCommand, #[case] secret: &str) {
    let logged = format!("{command:?}");

    assert!(!logged.contains(secret));
    assert!(logged.contains("<redacted>"));
}

/// The Display section hides the terminal settings the GUI has no effect in
/// (the spec's *Settings with no effect on the GUI are hidden*): the image
/// protocol and mouse support rows are `TuiOnly` (design D4), so only the
/// system notifications row shows.
#[test]
fn the_display_section_lists_only_system_notifications() {
    assert_eq!(
        GuiSettingsSection::Display.shown_rows(),
        vec![SettingKey::SystemNotifications]
    );
}
