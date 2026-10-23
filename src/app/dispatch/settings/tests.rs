//! Panel-row contracts of the pinned settings path.
//!
//! The `pinwin` request runner spawns a real process, so the save/flash half
//! takes the request outcome as its argument: `apply_panel_setting_outcome`
//! owns the "only an accepted request saves" rule, and `pinned_setting_request`
//! is the pure row-to-request mapping.

use super::*;
use crate::app::tests::make_app_stub;
use crate::pin::{
    PINNED_REQUEST_OVERRIDE, PINNED_REQUEST_TEST_LOCK, PINNED_REQUESTS, PanelGutter,
    PinnedSettingRequest,
};
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

/// A width row asks for the width it edited, a `Side` row moves the
/// panel, a gutter row changes only that gutter, and an accent row takes no
/// panel request. These rows' requests never depend on the active width, so
/// the mapping is read at the collapsed width.
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
    assert_eq!(
        pinned_setting_request(key, &candidate(), PinnedWidth::Collapsed),
        expected
    );
}

/// The `Reserve space` row asks for the candidate reserve mode only from the
/// collapsed width; from the expanded width it takes the accent rows'
/// no-request path, there a push would reserve the full expanded strip
/// (design D4). Both directions carry their saved value.
#[rstest]
#[case::enable_while_collapsed(
    PinnedWidth::Collapsed,
    true,
    Some(PinnedSettingRequest::Reserve(true))
)]
#[case::disable_while_collapsed(
    PinnedWidth::Collapsed,
    false,
    Some(PinnedSettingRequest::Reserve(false))
)]
#[case::enable_while_expanded(PinnedWidth::Expanded, true, None)]
#[case::disable_while_expanded(PinnedWidth::Expanded, false, None)]
fn the_reserve_row_requests_only_from_the_collapsed_width(
    #[case] active_width: PinnedWidth,
    #[case] saved: bool,
    #[case] expected: Option<PinnedSettingRequest>,
) {
    let candidate = PanelConfig {
        reserve: saved,
        ..candidate()
    };

    assert_eq!(
        pinned_setting_request(SettingKey::PanelReserve, &candidate, active_width),
        expected
    );
}

/// An accepted request saves the candidate, arms the settings save,
/// and records the width the row switched the panel to.
#[test]
fn accepted_pinned_request_saves_the_candidate() {
    let mut app = make_app_stub();

    app.apply_panel_setting_outcome(
        true,
        SettingKey::PanelWidthExpanded,
        Some(PinnedWidth::Expanded),
        candidate(),
        Ok(()),
    );

    assert_eq!(app.config.lock().unwrap().panel.width_expanded, 1200);
    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    assert!(app.settings_save_at.is_some(), "the save is armed");
}

/// A refused request flashes pinwin's reason and saves nothing, so
/// the row keeps its previous value and the panel keeps its width.
#[test]
fn refused_pinned_request_keeps_the_value_and_the_panel() {
    let mut app = make_app_stub();
    let previous = app.config.lock().unwrap().panel.width_expanded;

    app.apply_panel_setting_outcome(
        true,
        SettingKey::PanelWidthExpanded,
        Some(PinnedWidth::Expanded),
        candidate(),
        Err("the panel does not fit this output".into()),
    );

    assert_eq!(app.config.lock().unwrap().panel.width_expanded, previous);
    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert!(app.settings_save_at.is_none(), "no save is armed");
    assert_eq!(app.status, "the panel does not fit this output");
    assert_eq!(app.status_severity, ToastSeverity::Warning);
}

/// The one-in-flight contract: a second F2 step while the first request is
/// pending sends nothing, and the accepted candidate saves only when the
/// first outcome arrives.
#[test]
fn a_second_panel_step_while_a_request_is_pending_is_ignored() {
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let mut app = make_app_stub();
    let previous = app.config.lock().unwrap().panel.width_expanded;

    app.apply_panel_setting_pinned(SettingKey::PanelWidthExpanded, 1, true);
    app.apply_panel_setting_pinned(SettingKey::PanelWidthExpanded, 1, true);

    assert_eq!(
        PINNED_REQUESTS.lock().unwrap().len(),
        1,
        "the pending step is ignored, not queued"
    );
    assert!(
        app.settings_save_at.is_none(),
        "nothing saves before acceptance"
    );
    assert_eq!(app.config.lock().unwrap().panel.width_expanded, previous);

    assert!(app.drain_pinned_requests());
    assert_eq!(
        app.config.lock().unwrap().panel.width_expanded,
        previous + 10
    );
    assert!(app.settings_save_at.is_some(), "the accepted value saves");
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

/// A refusal delivered through the worker channel flashes pinwin's reason and
/// saves nothing, so the row keeps its previous value.
#[test]
fn a_refused_panel_request_saves_nothing_when_its_outcome_arrives() {
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() =
        Some(Err("the panel does not fit this output".into()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let mut app = make_app_stub();
    let previous = app.config.lock().unwrap().panel.width_expanded;

    app.apply_panel_setting_pinned(SettingKey::PanelWidthExpanded, 1, true);
    assert!(app.drain_pinned_requests());

    assert_eq!(app.config.lock().unwrap().panel.width_expanded, previous);
    assert!(app.settings_save_at.is_none(), "a refusal arms no save");
    assert_eq!(app.status, "the panel does not fit this output");
    assert_eq!(app.status_severity, ToastSeverity::Warning);
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

/// An accent row saves without a panel request and, while pinned,
/// announces that the change applies on the next launch.
#[test]
fn accent_row_while_pinned_saves_and_announces_the_next_launch() {
    let mut app = make_app_stub();
    let next = PanelConfig {
        accent: false,
        ..candidate()
    };

    app.apply_panel_setting_outcome(true, SettingKey::PanelAccent, None, next, Ok(()));

    assert!(!app.config.lock().unwrap().panel.accent);
    assert_eq!(
        app.status,
        "Accent changes apply on the next mbv --pin launch"
    );
    assert_eq!(app.status_severity, ToastSeverity::Neutral);
}

/// A launch outside the panel saves the value with no request, no
/// width record, and no launch toast.
#[test]
fn unpinned_row_saves_without_recording_a_pinned_width() {
    let mut app = make_app_stub();

    app.apply_panel_setting_outcome(
        false,
        SettingKey::PanelWidth,
        Some(PinnedWidth::Expanded),
        candidate(),
        Ok(()),
    );

    assert_eq!(app.config.lock().unwrap().panel.width, 480);
    assert_eq!(app.pinned_width, PinnedWidth::Collapsed);
    assert!(app.status.is_empty(), "no toast outside the panel");
    assert!(app.settings_save_at.is_some(), "the save is armed");
}

/// While collapsed, an accepted enable sends one `set-reserve push`, saves
/// the value, and announces nothing: the strip visibly appears (design D3;
/// spec "Enable reserve while pinned").
#[test]
fn the_reserve_row_enable_while_collapsed_saves_without_an_announcement() {
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let mut app = make_app_stub();
    app.config.lock().unwrap().panel.reserve = false;

    app.apply_panel_setting_pinned(SettingKey::PanelReserve, 0, true);
    app.drain_pinned_requests();

    assert_eq!(
        *PINNED_REQUESTS.lock().unwrap(),
        vec![PinnedSettingRequest::Reserve(true)],
        "the collapsed enable asks to push"
    );
    assert!(app.config.lock().unwrap().panel.reserve, "the saved enable");
    assert!(app.settings_save_at.is_some(), "the save is armed");
    assert!(app.status.is_empty(), "enabling needs no toast");
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

/// A saved disable while pinned keeps the held strip here and releases it
/// only on the next `mbv --pin` launch, so the neutral toast names that
/// (spec "Disable reserve while pinned").
#[test]
fn the_reserve_row_disable_while_pinned_saves_and_announces_the_release() {
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let mut app = make_app_stub();

    app.apply_panel_setting_pinned(SettingKey::PanelReserve, 0, true);
    app.drain_pinned_requests();

    assert_eq!(
        *PINNED_REQUESTS.lock().unwrap(),
        vec![PinnedSettingRequest::Reserve(false)],
        "the collapsed disable asks to cover"
    );
    assert!(
        !app.config.lock().unwrap().panel.reserve,
        "the saved disable"
    );
    assert_eq!(
        app.status,
        "The held strip is released on the next mbv --pin launch"
    );
    assert_eq!(app.status_severity, ToastSeverity::Neutral);
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

/// While expanded, the `Reserve space` row takes the accent rows'
/// no-request path: the flipped value saves and no `pinwin` request goes
/// out, because a push there would reserve the full expanded strip
/// (design D4; spec "Enable reserve while expanded").
#[test]
fn the_reserve_row_while_expanded_saves_without_a_request() {
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let mut app = make_app_stub();
    app.note_pinned_width_applied(PinnedWidth::Expanded);

    app.apply_panel_setting_pinned(SettingKey::PanelReserve, 0, true);

    assert!(
        PINNED_REQUESTS.lock().unwrap().is_empty(),
        "no request leaves the expanded width"
    );
    assert!(!app.config.lock().unwrap().panel.reserve, "the saved value");
    assert!(app.settings_save_at.is_some(), "the save is armed");
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

/// A saved enable while expanded sends no request and announces that the
/// strip is reserved on the next collapse (design D4; spec "Enable reserve
/// while expanded").
#[test]
fn a_saved_enable_while_expanded_announces_the_next_collapse() {
    let mut app = make_app_stub();
    app.note_pinned_width_applied(PinnedWidth::Expanded);

    app.apply_panel_setting_outcome(true, SettingKey::PanelReserve, None, candidate(), Ok(()));

    assert!(app.config.lock().unwrap().panel.reserve, "the saved enable");
    assert_eq!(app.status, "The strip is reserved on the next collapse");
    assert_eq!(app.status_severity, ToastSeverity::Neutral);
}
