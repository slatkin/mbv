//! Unit tests for the pinwin boundary.

use super::{
    PanelExitAction, PanelGutter, PanelIdentity, PanelReserveMode, PinSpawnError,
    StartFailureAction, claim_panel_identity, exit_action, failure_action, is_pinned,
    pinned_width_argv, request_result, run_argv, set_edge_argv, set_gutters_argv, set_reserve_argv,
    terminal_prefix, toggle_argv,
};
use mbv_config::{PanelConfig, PanelSide};
use mbv_ui_model::settings::PinnedWidth;
use std::ffi::OsStr;

/// The argv a test expects, as owned strings.
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// A failed pinned start falls back to the terminal only when
/// there is one; otherwise it notifies and exits 1.
#[test]
fn start_failure_falls_back_to_the_terminal_only_for_a_tty_stdin() {
    assert_eq!(failure_action(true), StartFailureAction::WarnInTerminal);
    assert_eq!(failure_action(false), StartFailureAction::NotifyAndExit);
}

/// A successful `pinwin run` exits quietly; a failure is logged,
/// and a desktop launch also notifies because it has no stderr to read.
#[rstest::rstest]
#[case::success_is_quiet(true, true, PanelExitAction::Quiet)]
#[case::terminal_failure_is_logged(false, true, PanelExitAction::Log)]
#[case::desktop_failure_notifies(false, false, PanelExitAction::LogAndNotify)]
fn exit_action_reports_a_failure_only_outside_a_terminal(
    #[case] success: bool,
    #[case] stdin_is_terminal: bool,
    #[case] expected: PanelExitAction,
) {
    assert_eq!(exit_action(success, stdin_is_terminal), expected);
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
    let _env = crate::config::tests::SYS_ENV_LOCK.lock().unwrap();
    mbv_config::set_test_env_var("PINWIN_PANEL", "mbv");
    mbv_config::set_test_env_var("PINWIN_SOCKET", "/run/user/1000/pinwin/wayland-0.sock");

    claim_panel_identity(false);

    assert!(!is_pinned());
    assert!(std::env::var_os("PINWIN_PANEL").is_none());
    assert!(std::env::var_os("PINWIN_SOCKET").is_none());
}

/// The launcher argv names the panel, shows an existing
/// panel, passes every layout value mbv owns, and puts the terminal, the mbv
/// exe and the log level after `--`.
#[rstest::rstest]
#[case::default_layout(
    PanelConfig::default(),
    None,
    vec![
        "run", "--name", "mbv", "--show-existing", "--edge", "left", "--size", "400px",
        "--reserve", "push", "--gutter-top", "0", "--gutter-bottom", "0", "--gutter-left", "0",
        "--gutter-right", "0", "--focus-ring", "#dabc7f:1", "--", "ghostty", "-e",
        "/usr/bin/mbv", "--in-panel",
    ]
)]
#[case::right_side_gutters_accent_off_and_log_level(
    PanelConfig {
        side: PanelSide::Right,
        width: 320,
        gutter_top: 4,
        gutter_bottom: -8,
        gutter_left: 2,
        gutter_right: 6,
        accent: false,
        ..PanelConfig::default()
    },
    Some("mbv=debug"),
    vec![
        "run", "--name", "mbv", "--show-existing", "--edge", "right", "--size", "320px",
        "--reserve", "push", "--gutter-top", "4", "--gutter-bottom", "-8", "--gutter-left", "2",
        "--gutter-right", "6", "--focus-ring", "off", "--", "ghostty", "-e", "/usr/bin/mbv",
        "--in-panel", "--log-level", "mbv=debug",
    ]
)]
#[case::reserve_off_starts_covering(
    PanelConfig {
        width: 400,
        reserve: false,
        ..PanelConfig::default()
    },
    None,
    vec![
        "run", "--name", "mbv", "--show-existing", "--edge", "left", "--size", "400px",
        "--reserve", "cover", "--gutter-top", "0", "--gutter-bottom", "0", "--gutter-left", "0",
        "--gutter-right", "0", "--focus-ring", "#dabc7f:1", "--", "ghostty", "-e",
        "/usr/bin/mbv", "--in-panel",
    ]
)]
fn run_argv_carries_the_layout_the_terminal_and_the_log_level(
    #[case] config: PanelConfig,
    #[case] log_level: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(
        run_argv(
            &config,
            &strings(&["ghostty", "-e"]),
            OsStr::new("/usr/bin/mbv"),
            log_level,
        ),
        strings(&expected)
    );
}

/// `[panel] pin_terminal` wins over `TERMINAL`, and an empty
/// prefix counts as unset.
#[rstest::rstest]
#[case::config_prefix_wins(
    Some(vec!["ghostty", "--gtk-single-instance=false", "-e"]),
    Some("wezterm"),
    vec!["ghostty", "--gtk-single-instance=false", "-e"]
)]
#[case::empty_config_prefix_falls_back(Some(vec![]), Some("wezterm"), vec!["wezterm", "-e"])]
fn terminal_prefix_prefers_the_config_prefix_then_the_environment(
    #[case] pin_terminal: Option<Vec<&str>>,
    #[case] terminal_env: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    let pin_terminal = pin_terminal.map(|argv| strings(&argv));
    assert_eq!(
        terminal_prefix(pin_terminal.as_deref(), terminal_env, "pin_terminal"),
        Ok(strings(&expected))
    );
}

/// With neither source set, the reason names both.
#[test]
fn terminal_prefix_without_a_source_names_both() {
    let error = terminal_prefix(None, None, "pin_terminal").expect_err("no terminal must fail");
    assert!(
        error.contains("[panel] pin_terminal"),
        "error must name config"
    );
    assert!(error.contains("TERMINAL"), "error must name environment");
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

/// `--toggle` names the panel it toggles.
#[test]
fn toggle_argv_names_the_panel() {
    assert_eq!(toggle_argv("mbv"), strings(&["toggle", "mbv"]));
}

/// A missing `pinwin` asks for an install; any other spawn failure names its
/// cause.
#[rstest::rstest]
#[case::missing(std::io::ErrorKind::NotFound, "pinwin is not installed")]
#[case::denied(
    std::io::ErrorKind::PermissionDenied,
    "pinwin could not start: permission denied"
)]
fn pin_spawn_error_names_the_cause(#[case] kind: std::io::ErrorKind, #[case] expected: &str) {
    assert_eq!(
        PinSpawnError::new(std::io::Error::from(kind)).to_string(),
        expected
    );
}

/// Exit 0 is success, exit 1 is pinwin's own reason, exit 2 is a
/// usage mismatch, and any other ending names the ending.
#[rstest::rstest]
#[case::success(Some(0), "", Ok(()))]
#[case::refused(
    Some(1),
    "pinwin: no panel named mbv\n",
    Err("pinwin: no panel named mbv".to_string())
)]
#[case::refused_without_a_reason(Some(1), "  \n", Err("pinwin refused the request".to_string()))]
#[case::usage_mismatch(
    Some(2),
    "usage: pinwin <command> [options]\n",
    Err("pinwin rejected the command line: mbv and pinwin disagree on the pinwin CLI".to_string())
)]
#[case::other_status(Some(3), "", Err("pinwin exited with status 3".to_string()))]
#[case::signal(None, "", Err("pinwin ended by a signal".to_string()))]
fn request_result_maps_the_exit_code_and_stderr(
    #[case] code: Option<i32>,
    #[case] stderr: &str,
    #[case] expected: Result<(), String>,
) {
    assert_eq!(request_result(code, stderr), expected);
}
