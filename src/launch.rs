//! The mbv binary path a launcher starts.
//!
//! Both the local-daemon spawn and the `--pin` launcher start a fresh mbv from
//! the build the running process came from (spec pin-swap "Launches start the
//! Owner's own build after its binary file changes").

use std::path::{Path, PathBuf};

/// The binary path a launch starts from.
///
/// After the started file is unlinked, Linux `current_exe()` ends in
/// ` (deleted)` and spawn fails with ENOENT. Then start from the launcher's
/// `/proc/{launcher_pid}/exe`, which names the same build while the launcher
/// runs.
pub(crate) fn launch_exe(current: &Path, launcher_pid: u32) -> PathBuf {
    if current.to_string_lossy().ends_with(" (deleted)") {
        PathBuf::from(format!("/proc/{launcher_pid}/exe"))
    } else {
        current.to_path_buf()
    }
}

#[cfg(test)]
mod tests;
