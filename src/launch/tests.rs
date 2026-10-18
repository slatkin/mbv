//! Unit tests for the shared launch path rule.

use super::launch_exe;
use std::path::{Path, PathBuf};

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
