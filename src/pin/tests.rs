//! Unit tests for the pinwin boundary (`pinwin-app-host` design D1 to D8).

use super::{
    PanelExitAction, PanelGutter, PanelIdentity, PinSpawnError, StartFailureAction, exit_action,
    failure_action, pinned_width_argv, request_result, run_argv, set_edge_argv, set_gutters_argv,
    terminal_prefix, toggle_argv,
};
use mbv_config::{PanelConfig, PanelSide};
use mbv_ui_model::settings::PinnedWidth;
use std::ffi::OsStr;

/// The argv a test expects, as owned strings.
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// Design D5: a failed pinned start falls back to the terminal only when
/// there is one; otherwise it notifies and exits 1.
#[test]
fn start_failure_falls_back_to_the_terminal_only_for_a_tty_stdin() {
    assert_eq!(failure_action(true), StartFailureAction::WarnInTerminal);
    assert_eq!(failure_action(false), StartFailureAction::NotifyAndExit);
}

/// Design D2: a successful `pinwin run` exits quietly; a failure is logged,
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

/// Design D3: a set panel name makes the launch pinned; an empty or unset
/// value leaves it unpinned.
#[rstest::rstest]
#[case::set(Some("mbv"), true)]
#[case::empty(Some(""), false)]
fn panel_identity_is_pinned_only_for_a_set_panel_name(
    #[case] panel: Option<&str>,
    #[case] expected: bool,
) {
    let identity = PanelIdentity::new(panel.map(str::to_owned), None);
    assert_eq!(identity.name.is_some(), expected);
}

/// Design D3: an empty socket counts as unset, so a request never points at
/// an empty path.
#[rstest::rstest]
#[case::set(
    Some("/run/user/1000/pinwin/wayland-0.sock"),
    Some("/run/user/1000/pinwin/wayland-0.sock")
)]
#[case::empty(Some(""), None)]
fn panel_identity_keeps_a_socket_only_when_set(
    #[case] socket: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let identity = PanelIdentity::new(Some("mbv".to_string()), socket.map(str::to_owned));
    assert_eq!(identity.socket.as_deref(), expected);
}

/// Design D1/D2/D5/D8: the launcher argv names the panel, shows an existing
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
        "/usr/bin/mbv",
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
        "--log-level", "mbv=debug",
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

/// Design D1: `[panel] pin_terminal` wins over `TERMINAL`, and an empty
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

/// Design D1: with neither source set, the reason names both.
#[test]
fn terminal_prefix_without_a_source_names_both() {
    let error = terminal_prefix(None, None, "pin_terminal").expect_err("no terminal must fail");
    assert!(
        error.contains("[panel] pin_terminal"),
        "error must name config"
    );
    assert!(error.contains("TERMINAL"), "error must name environment");
}

/// Design D6: the requested view picks both the saved width and the reserve
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

/// Design D7: a side change names the panel and the new edge.
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

/// Design D7: one gutter row edits one gutter, including a negative inset.
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

/// Design D8: `--toggle` names the panel it toggles.
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

/// Design D4: exit 0 is success, exit 1 is pinwin's own reason, exit 2 is a
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
