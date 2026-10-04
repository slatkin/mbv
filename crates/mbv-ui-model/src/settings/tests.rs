use super::*;
use mbv_config::PanelConfig;
use rstest::rstest;

/// The `Accent color` row's cycle contract (change
/// `pinned-panel-focus-accent`, design D3): from the default the row steps
/// to the next fixed list entry, and a configured colour outside the fixed
/// list is kept in the cycle — it leads it, so it is never lost while it is
/// configured.
#[rstest]
#[case::default_steps_to_next_list_entry(
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
    mbv_config::PanelAccentColor([0x7f, 0xc8, 0xda]),
)]
#[case::custom_colour_stays_reachable(
    mbv_config::PanelAccentColor([0x12, 0x34, 0x56]),
    mbv_config::DEFAULT_PANEL_ACCENT_COLOR,
)]
fn accent_color_cycle_steps_to_the_next_entry_and_keeps_a_custom_colour(
    #[case] configured: mbv_config::PanelAccentColor,
    #[case] expected: mbv_config::PanelAccentColor,
) {
    let panel = PanelConfig {
        accent_color: configured,
        ..PanelConfig::default()
    };
    let stepped = changed_panel_config(SettingKey::PanelAccentColor, panel, 1)
        .expect("an Accent color step changes the panel config");
    assert_eq!(stepped.accent_color, expected);
}
