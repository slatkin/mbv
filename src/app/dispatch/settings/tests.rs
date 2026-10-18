//! Panel-row contracts of the pinned settings path (change `pinwin-app-host`,
//! design D7).
//!
//! The `pinwin` request runner spawns a real process, so the save/flash half
//! takes the request outcome as its argument: `apply_panel_setting_outcome`
//! owns the "only an accepted request saves" rule, and `pinned_setting_request`
//! is the pure row-to-request mapping.

use super::*;
use crate::app::tests::make_app_stub;
use crate::pin::{PanelGutter, PinnedWidth};
use mbv_config::{PanelConfig, PanelSide};
use rstest::rstest;

/// A candidate layout whose every layout value differs from the default, so a
/// saved value is recognisable.
fn candidate() -> PanelConfig {
    PanelConfig {
        side: PanelSide::Right,
        width: 480,
        width_expanded: 1200,
        gutter_top: 4,
        gutter_bottom: -8,
        gutter_left: 2,
        gutter_right: 6,
        ..PanelConfig::default()
    }
}

/// Design D7: a width row asks for the width it edited, a `Side` row moves the
/// panel, a gutter row changes only that gutter, and an accent row takes no
/// panel request.
#[rstest]
#[case::width_row(
    SettingKey::PanelWidth,
    Some(PinnedSettingRequest::Width(PinnedWidth::Collapsed))
)]
#[case::expanded_width_row(
    SettingKey::PanelWidthExpanded,
    Some(PinnedSettingRequest::Width(PinnedWidth::Expanded))
)]
#[case::side_row(
    SettingKey::PanelSide,
    Some(PinnedSettingRequest::Side(PanelSide::Right))
)]
#[case::gutter_top_row(
    SettingKey::PanelGutterTop,
    Some(PinnedSettingRequest::Gutter(PanelGutter::Top, 4))
)]
#[case::gutter_bottom_row(
    SettingKey::PanelGutterBottom,
    Some(PinnedSettingRequest::Gutter(PanelGutter::Bottom, -8))
)]
#[case::gutter_left_row(
    SettingKey::PanelGutterLeft,
    Some(PinnedSettingRequest::Gutter(PanelGutter::Left, 2))
)]
#[case::gutter_right_row(
    SettingKey::PanelGutterRight,
    Some(PinnedSettingRequest::Gutter(PanelGutter::Right, 6))
)]
#[case::accent_row(SettingKey::PanelAccentColor, None)]
fn pinned_setting_request_names_the_rows_panel_change(
    #[case] key: SettingKey,
    #[case] expected: Option<PinnedSettingRequest>,
) {
    assert_eq!(pinned_setting_request(key, &candidate()), expected);
}

/// Design D7: an accepted request saves the candidate, arms the settings save,
/// and records the width the row switched the panel to.
#[test]
fn accepted_pinned_request_saves_the_candidate() {
    let mut app = make_app_stub();

    app.apply_panel_setting_outcome(true, SettingKey::PanelWidthExpanded, candidate(), Ok(()));

    assert_eq!(app.config.lock().unwrap().panel.width_expanded, 1200);
    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    assert!(app.settings_save_at.is_some(), "the save is armed");
}

/// Design D7: a refused request flashes pinwin's reason and saves nothing, so
/// the row keeps its previous value and the panel keeps its width.
#[test]
fn refused_pinned_request_keeps_the_value_and_the_panel() {
    let mut app = make_app_stub();
    let previous = app.config.lock().unwrap().panel.width_expanded;

    app.apply_panel_setting_outcome(
        true,
        SettingKey::PanelWidthExpanded,
        candidate(),
        Err("the panel does not fit this output".into()),
    );

    assert_eq!(app.config.lock().unwrap().panel.width_expanded, previous);
    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert!(app.settings_save_at.is_none(), "no save is armed");
    assert_eq!(app.status, "the panel does not fit this output");
    assert_eq!(app.status_severity, ToastSeverity::Warning);
}

/// Design D3/D7: an accent row saves without a panel request and, while pinned,
/// announces that the change applies on the next launch.
#[test]
fn accent_row_while_pinned_saves_and_announces_the_next_launch() {
    let mut app = make_app_stub();
    let next = PanelConfig {
        accent: false,
        ..candidate()
    };

    app.apply_panel_setting_outcome(true, SettingKey::PanelAccent, next, Ok(()));

    assert!(!app.config.lock().unwrap().panel.accent);
    assert_eq!(
        app.status,
        "Accent changes apply on the next mbv --pin launch"
    );
    assert_eq!(app.status_severity, ToastSeverity::Neutral);
}

/// Design D7: a launch outside the panel saves the value with no request, no
/// width record, and no launch toast.
#[test]
fn unpinned_row_saves_without_recording_a_pinned_width() {
    let mut app = make_app_stub();

    app.apply_panel_setting_outcome(false, SettingKey::PanelWidth, candidate(), Ok(()));

    assert_eq!(app.config.lock().unwrap().panel.width, 480);
    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert!(app.status.is_empty(), "no toast outside the panel");
    assert!(app.settings_save_at.is_some(), "the save is armed");
}
