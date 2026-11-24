//! Unit tests for the panel identity and the setting requests.

use super::{
    PanelGutter, PanelIdentity, PanelReserveMode, claim_panel_identity, is_pinned,
    pinned_width_argv, set_edge_argv, set_gutters_argv, set_reserve_argv,
};
use mbv_config::{PanelConfig, PanelSide};
use mbv_ui_model::settings::PinnedWidth;

/// The argv a test expects, as owned strings.
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// A non-empty panel name makes the launch pinned only together with the
/// hidden launch flag. The socket is kept even without the flag, so the
/// variables are scrubbed whether the launch is pinned or not.
#[rstest::rstest]
#[case::panel_and_flag(
    Some("mbv"),
    Some("/run/user/1000/pinwin/wayland-0.sock"),
    true,
    Some("mbv")
)]
#[case::panel_without_flag(Some("mbv"), Some("/run/user/1000/pinwin/wayland-0.sock"), false, None)]
#[case::empty_panel_with_flag(Some(""), None, true, None)]
#[case::unset_panel_with_flag(None, None, true, None)]
fn panel_identity_is_pinned_only_for_a_panel_name_and_the_launch_flag(
    #[case] panel: Option<&str>,
    #[case] socket: Option<&str>,
    #[case] in_panel: bool,
    #[case] expected: Option<&str>,
) {
    let identity = PanelIdentity::new(
        panel.map(str::to_owned),
        socket.map(str::to_owned),
        in_panel,
    );
    assert_eq!(identity.name.as_deref(), expected);
    assert_eq!(
        identity.socket.as_deref(),
        socket.filter(|socket| !socket.is_empty())
    );
}

/// A panel environment without the launch flag leaves the launch unpinned,
/// and both variables leave this process's environment so no child inherits
/// the panel identity.
#[test]
fn panel_environment_without_the_flag_is_scrubbed_and_unpinned() {
    let _env = crate::app::config::tests::SYS_ENV_LOCK.lock().unwrap();
    mbv_config::set_test_env_var("PINWIN_PANEL", "mbv");
    mbv_config::set_test_env_var("PINWIN_SOCKET", "/run/user/1000/pinwin/wayland-0.sock");

    claim_panel_identity(false);

    assert!(!is_pinned());
    assert!(std::env::var_os("PINWIN_PANEL").is_none());
    assert!(std::env::var_os("PINWIN_SOCKET").is_none());
}

/// The requested view picks both the saved width and the reserve
/// mode, so one `x` press is one panel change and no tiled window moves.
#[rstest::rstest]
#[case::collapsed_pushes_at_width(
    PinnedWidth::Collapsed,
    vec!["set-size", "mbv", "320px", "--reserve", "push"]
)]
#[case::expanded_covers_at_width_expanded(
    PinnedWidth::Expanded,
    vec!["set-size", "mbv", "1100px", "--reserve", "cover"]
)]
fn pinned_width_argv_sends_the_views_width_and_reserve_mode(
    #[case] width: PinnedWidth,
    #[case] expected: Vec<&str>,
) {
    let config = PanelConfig {
        width: 320,
        width_expanded: 1100,
        ..PanelConfig::default()
    };
    assert_eq!(pinned_width_argv("mbv", &config, width), strings(&expected));
}

/// With `reserve` off no `set-size` request pushes: the collapse covers
/// instead of reserving a strip, and the expanded width covers as always,
/// so no request re-reserves the strip a launch or earlier collapse held
/// (change `panel-reserve-space`, design D2).
#[rstest::rstest]
#[case::collapsed(PinnedWidth::Collapsed, vec!["set-size", "mbv", "320px", "--reserve", "cover"])]
#[case::expanded(PinnedWidth::Expanded, vec!["set-size", "mbv", "1100px", "--reserve", "cover"])]
fn pinned_width_argv_with_reserve_off_never_pushes(
    #[case] width: PinnedWidth,
    #[case] expected: Vec<&str>,
) {
    let config = PanelConfig {
        width: 320,
        width_expanded: 1100,
        reserve: false,
        ..PanelConfig::default()
    };
    assert_eq!(pinned_width_argv("mbv", &config, width), strings(&expected));
}

/// A side change names the panel and the new edge.
#[rstest::rstest]
#[case::left("mbv", PanelSide::Left, vec!["set-edge", "mbv", "left"])]
#[case::right("mbv", PanelSide::Right, vec!["set-edge", "mbv", "right"])]
fn set_edge_argv_sends_the_side(
    #[case] panel: &str,
    #[case] side: PanelSide,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(set_edge_argv(panel, side), strings(&expected));
}

/// One gutter row edits one gutter, including a negative inset.
#[rstest::rstest]
#[case::top("mbv", PanelGutter::Top, -10, vec!["set-gutters", "mbv", "--top", "-10"])]
#[case::right("tunes", PanelGutter::Right, 6, vec!["set-gutters", "tunes", "--right", "6"])]
fn set_gutters_argv_sends_the_edited_gutter(
    #[case] panel: &str,
    #[case] gutter: PanelGutter,
    #[case] value: i32,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(set_gutters_argv(panel, gutter, value), strings(&expected));
}

/// The `Reserve space` row names the panel and the candidate mode.
#[rstest::rstest]
#[case::reserve_on(PanelReserveMode::Push, vec!["set-reserve", "mbv", "push"])]
#[case::reserve_off(PanelReserveMode::Cover, vec!["set-reserve", "mbv", "cover"])]
fn set_reserve_argv_sends_the_mode(#[case] reserve: PanelReserveMode, #[case] expected: Vec<&str>) {
    assert_eq!(set_reserve_argv("mbv", reserve), strings(&expected));
}
