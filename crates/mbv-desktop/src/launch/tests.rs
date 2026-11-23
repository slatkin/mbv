//! Unit tests for the shared launch path rule, the terminal argv, and the
//! `pinwin` request runner.

use super::{PinSpawnError, launch_exe, request_result, terminal_prefix, toggle_argv};
use std::path::{Path, PathBuf};

/// The argv a test expects, as owned strings.
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// Regression for the Owner log `daemon.pin_swap.spawn_failed …
/// "… (deleted)"` (2026-10-10 and 2026-10-11): when `current_exe()` ends
/// in ` (deleted)` the started file is gone, so the launch falls back to
/// the launcher's `/proc/<pid>/exe`; a live path is returned unchanged.
#[rstest::rstest]
#[case::live_path_is_returned_unchanged("/usr/bin/mbv", 4242, "/usr/bin/mbv")]
#[case::deleted_suffix_falls_back_to_launcher_proc_exe(
    "/home/u/target/debug/mbv (deleted)",
    4242,
    "/proc/4242/exe"
)]
fn launch_exe_falls_back_to_the_launcher_binary_when_current_is_deleted(
    #[case] current: &str,
    #[case] launcher_pid: u32,
    #[case] expected: &str,
) {
    assert_eq!(
        launch_exe(Path::new(current), launcher_pid),
        PathBuf::from(expected)
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
