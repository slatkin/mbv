use super::make_app_stub;
use crate::app::state::types::overlay::OverlayRequest;
use crate::app::state::types::settings::SettingKey;
use rstest::rstest;

#[rstest]
#[case(None, Some("halfblocks"))]
#[case(Some("kitty"), Some("iterm2"))]
fn image_protocol_activation_cycles_through_supported_values(
    #[case] current: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let mut app = make_app_stub();
    app.image_protocol = current.map(str::to_owned);
    app.image_protocol_enabled = current.is_some();

    app.handle_settings_activate(SettingKey::ImageProtocol);

    assert_eq!(app.image_protocol.as_deref(), expected);
    assert_eq!(app.image_protocol_enabled, expected.is_some());
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
