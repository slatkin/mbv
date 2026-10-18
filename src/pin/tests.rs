//! Unit tests for the pinwin boundary (`pinwin-app-host` design D3/D4) and the
//! retired pty start-up (`pin-mbv-in-pinwin`).

use super::{
    PanelGutter, PanelIdentity, PanelReserve, PinnedWidth, StartFailureAction, failure_action,
    layout_from_config, request_result, run_argv, set_edge_argv, set_gutters_argv, set_size_argv,
    terminal_argv, toggle_argv,
};
use mbv_config::{PanelConfig, PanelSide};
use std::ffi::OsStr;
use std::num::NonZeroU16;

/// The argv a test expects, as owned strings.
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// Design D2/D4: the active width selects `width` (collapsed) or
/// `width_expanded` (expanded); nothing constrains which value is larger.
#[rstest::rstest]
#[case::collapsed_uses_width(PinnedWidth::Collapsed, 40, 80, 40)]
#[case::expanded_uses_width_expanded(PinnedWidth::Expanded, 40, 80, 80)]
#[case::expanded_below_collapsed_still_wins(PinnedWidth::Expanded, 60, 30, 30)]
fn layout_from_config_selects_the_active_width(
    #[case] width: PinnedWidth,
    #[case] collapsed: u16,
    #[case] expanded: u16,
    #[case] expected: u16,
) {
    let config = PanelConfig {
        width: collapsed,
        width_expanded: expanded,
        ..PanelConfig::default()
    };
    assert_eq!(
        layout_from_config(&config, width).cols(),
        NonZeroU16::new(expected).expect("test columns are non-zero")
    );
}

/// Design D5: a failed pinned start falls back to the terminal only when
/// there is one; otherwise it notifies and exits 1.
#[test]
fn start_failure_falls_back_to_the_terminal_only_for_a_tty_stdin() {
    assert_eq!(failure_action(true), StartFailureAction::WarnInTerminal);
    assert_eq!(failure_action(false), StartFailureAction::NotifyAndExit);
}

/// Issue #898 (`pinned-expand-covers-collapsed-pushes`): the collapsed
/// panel pushes and the expanded panel covers; no setting takes part.
#[rstest::rstest]
#[case::collapsed_pushes(PinnedWidth::Collapsed, pinwin::layout::Coverage::Push)]
#[case::expanded_covers(PinnedWidth::Expanded, pinwin::layout::Coverage::Cover)]
fn layout_from_config_coverage_follows_the_width(
    #[case] width: PinnedWidth,
    #[case] expected: pinwin::layout::Coverage,
) {
    assert_eq!(
        layout_from_config(&PanelConfig::default(), width).coverage(),
        expected
    );
}

/// Design D3: a set panel name makes the launch pinned; an empty or unset
/// value leaves it unpinned.
#[rstest::rstest]
#[case::set(Some("mbv"), true)]
#[case::empty(Some(""), false)]
#[case::unset(None, false)]
fn panel_identity_is_pinned_only_for_a_set_panel_name(
    #[case] panel: Option<&str>,
    #[case] expected: bool,
) {
    let identity = PanelIdentity::new(panel.map(str::to_owned), None);
    assert_eq!(identity.is_pinned(), expected);
}

/// Design D3: an empty socket counts as unset, so a request never points at
/// an empty path.
#[rstest::rstest]
#[case::set(
    Some("/run/user/1000/pinwin/wayland-0.sock"),
    Some("/run/user/1000/pinwin/wayland-0.sock")
)]
#[case::empty(Some(""), None)]
#[case::unset(None, None)]
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
#[case::terminal_env_falls_back(None, Some("wezterm"), vec!["wezterm", "-e"])]
fn terminal_argv_prefers_the_config_prefix_then_the_environment(
    #[case] pin_terminal: Option<Vec<&str>>,
    #[case] terminal_env: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    let pin_terminal = pin_terminal.map(|argv| strings(&argv));
    assert_eq!(
        terminal_argv(pin_terminal.as_deref(), terminal_env),
        Ok(strings(&expected))
    );
}

/// Design D1: with neither source set, the reason names both.
#[test]
fn terminal_argv_without_a_source_names_both() {
    let error = terminal_argv(None, None).expect_err("no terminal must fail");
    assert!(
        error.contains("[panel] pin_terminal"),
        "error must name config"
    );
    assert!(error.contains("TERMINAL"), "error must name environment");
}

/// Design D6: one width change carries its reserve mode.
#[rstest::rstest]
#[case::collapsed_pushes("mbv", 400, PanelReserve::Push, vec!["set-size", "mbv", "400px", "--reserve", "push"])]
#[case::expanded_covers("tunes", 1100, PanelReserve::Cover, vec!["set-size", "tunes", "1100px", "--reserve", "cover"])]
fn set_size_argv_sends_the_width_with_its_reserve_mode(
    #[case] panel: &str,
    #[case] width: u16,
    #[case] reserve: PanelReserve,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(set_size_argv(panel, width, reserve), strings(&expected));
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
