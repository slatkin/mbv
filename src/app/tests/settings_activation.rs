use super::make_app_stub;
use crate::app::dispatch::notify::ToastSeverity;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_model::settings::SettingKey;
use rstest::rstest;

#[rstest]
#[case(None, Some("halfblocks"))]
#[case(Some("kitty"), Some("iterm2"))]
fn image_protocol_activation_cycles_through_supported_values(
    #[case] current: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let mut app = make_app_stub();
    app.images
        .configure_protocol(current.map(str::to_owned), current.is_some());

    app.handle_settings_activate(SettingKey::ImageProtocol);

    assert_eq!(app.images.protocol_override(), expected);
    assert_eq!(app.images.protocol_enabled(), expected.is_some());
}

fn owner_setting_value(config: &crate::config::Config, key: SettingKey) -> bool {
    match key {
        SettingKey::StayAlive => config.stay_alive,
        SettingKey::ConsumeVideos => config.consume_videos,
        SettingKey::ConsumeAudio => config.consume_audio,
        _ => unreachable!(),
    }
}

#[rstest]
#[case(SettingKey::StayAlive)]
#[case(SettingKey::ConsumeVideos)]
#[case(SettingKey::ConsumeAudio)]
fn owner_settings_are_persisted_to_disk_before_toggle_returns(#[case] key: SettingKey) {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let expected = !owner_setting_value(&app.config.lock().unwrap(), key);

    app.handle_settings_activate(key);

    let saved = crate::config::load_config().unwrap();
    assert_eq!(owner_setting_value(&saved, key), expected);
}

#[test]
fn mouse_support_activation_updates_config_and_defers_terminal_flip() {
    let mut app = make_app_stub();
    let initial = app.config.lock().unwrap().mouse_support;

    app.handle_settings_activate(SettingKey::MouseSupport);

    assert_eq!(app.config.lock().unwrap().mouse_support, !initial);
    assert_eq!(app.mouse_capture_pending, Some(!initial));
    assert!(app.settings_save_at.is_some());
}

/// The Tray row (labelled `Show systray icon`) is refused while stay-alive
/// is on: the stored preference is unchanged and a Neutral toast explains
/// why (contract `local-daemon-tray` "Toggle refused while stay-alive is
/// on", design D3, change `stay-alive-is-lifetime-only`).
#[test]
fn tray_toggle_is_refused_while_stay_alive_is_on() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().stay_alive = true;
    app.config.lock().unwrap().show_systray_icon = false;

    app.handle_settings_activate(SettingKey::ShowSysTrayIcon);

    assert!(!app.config.lock().unwrap().show_systray_icon);
    assert_eq!(app.status, "Tray stays on while Stay alive is on");
    assert_eq!(app.status_severity, ToastSeverity::Neutral);
}

#[test]
fn navigation_settings_raise_their_expected_overlay_requests() {
    let mut app = make_app_stub();

    app.handle_settings_activate(SettingKey::HiddenLibraries);

    assert!(matches!(
        app.pending_overlay,
        Some(OverlayRequest::OpenMultiselect(
            crate::app::MultiSelectKind::HiddenLibraries
        ))
    ));
}

/// `set_settings_value` sets the chosen value in the configuration, writes it
/// through to the player's prefs as the cycle handler does, and arms the
/// deferred save (row 9.1, design D13): the GUI's pick and the TUI's cycle end
/// in one effect.
#[test]
fn set_settings_value_sets_the_config_value_and_the_player_prefs() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().my_languages = vec!["English".into(), "Chinese".into()];

    app.set_settings_value(SettingKey::SubtitleLanguage, "Chinese");

    assert_eq!(app.config.lock().unwrap().subtitle_lang, "Chinese");
    assert_eq!(
        app.player.subtitle_prefs_snapshot().subtitle_lang,
        "Chinese",
        "the player's prefs take the picked value"
    );
    assert!(
        app.settings_save_at.is_some(),
        "the picked value arms the deferred save"
    );
}

/// A version row is read-only: activating it arms no save and changes no
/// setting (`owner-gate-on-wire-identity` design D4, spec "A version row is
/// activated").
#[test]
fn owner_version_row_activation_arms_no_save_and_changes_no_setting() {
    let mut app = make_app_stub();
    let before = format!("{:?}", app.config.lock().unwrap());

    app.handle_settings_activate(SettingKey::OwnerVersion);

    assert!(app.settings_save_at.is_none());
    assert_eq!(format!("{:?}", app.config.lock().unwrap()), before);
}
