use super::*;
use mbv_config::PanelConfig;
use rstest::rstest;

/// The Tray row (labelled `Show systray icon`) shows whether the Tray is
/// enabled, not the stored preference: with stay-alive on the row reads on
/// even when the preference is off (contract `local-daemon-tray` "Row while
/// stay-alive is on", design D3).
#[test]
fn tray_row_reads_on_while_stay_alive_is_on() {
    let cfg = mbv_config::Config {
        stay_alive: true,
        show_systray_icon: false,
        ..mbv_config::Config::default()
    };

    let value = setting_value(SettingKey::ShowSysTrayIcon, &cfg, &UiConfig::default());

    assert_eq!(value, bool_val(true));
}

/// The `Accent color` row's cycle contract (change
/// `pinned-panel-focus-accent`, design D3; issue #875): the row steps through
/// the fixed palette, then through the custom colour the shell holds (if
/// any), then wraps to the palette's first entry — so a custom colour is
/// reachable again after a full round trip.
#[rstest]
#[case::default_steps_to_next_list_entry(
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
    None,
    mbv_config::PanelAccentColor([0x7f, 0xc8, 0xda]),
)]
#[case::last_entry_wraps_to_first(
    mbv_config::PanelAccentColor([0xff, 0xff, 0xff]),
    None,
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
)]
#[case::custom_steps_to_first(
    mbv_config::PanelAccentColor([0x12, 0x34, 0x56]),
    Some(mbv_config::PanelAccentColor([0x12, 0x34, 0x56])),
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
)]
#[case::last_palette_entry_steps_to_custom(
    mbv_config::PanelAccentColor([0xff, 0xff, 0xff]),
    Some(mbv_config::PanelAccentColor([0x12, 0x34, 0x56])),
    mbv_config::PanelAccentColor([0x12, 0x34, 0x56]),
)]
fn accent_color_cycle_steps_through_the_palette_then_the_held_custom_colour(
    #[case] configured: mbv_config::PanelAccentColor,
    #[case] custom: Option<mbv_config::PanelAccentColor>,
    #[case] expected: mbv_config::PanelAccentColor,
) {
    let panel = PanelConfig {
        accent_color: configured,
        ..PanelConfig::default()
    };
    let stepped = changed_panel_config(SettingKey::PanelAccentColor, panel, 1, custom)
        .expect("an Accent color step changes the panel config");
    assert_eq!(stepped.accent_color, expected);
}
