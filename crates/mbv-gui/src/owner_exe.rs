//! The `mbv` Owner program the GUI Client resolves and starts (design D9 of
//! `add-gui-front-end`). The Owner is never this GUI program, so every Owner
//! start and every attach that records the restart path names the `mbv`
//! program resolved from this executable's directory, then from `PATH`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The Owner program's file name, looked up beside this executable and in
/// every `PATH` entry.
const OWNER_PROGRAM: &str = "mbv";

/// Resolve the `mbv` program: the first executable `mbv` in `exe_dir` (this
/// executable's directory), then in each `PATH` entry in order. Pure over the
/// executable directory, the `PATH` value, and the executability predicate,
/// so the resolution-order test is hermetic.
pub(crate) fn resolve_owner_program(
    exe_dir: Option<&Path>,
    path_value: &str,
    is_executable: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push(dir.join(OWNER_PROGRAM));
    }
    candidates.extend(std::env::split_paths(path_value).map(|entry| entry.join(OWNER_PROGRAM)));
    candidates
        .into_iter()
        .find(|candidate| is_executable(candidate))
}

/// Resolve the `mbv` program for this running GUI executable: beside it,
/// then from `PATH`. `Err` is the notice the window shows.
pub(crate) fn resolve() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let path_value = std::env::var("PATH").unwrap_or_default();
    resolve_owner_program(exe_dir.as_deref(), &path_value, is_executable_file).ok_or_else(|| {
        format!(
            "mbv: could not find the `{OWNER_PROGRAM}` program beside mbv-gui or on PATH; \
             the GUI Client cannot start or attach to an Owner process."
        )
    })
}

/// Whether `path` is an executable regular file: the check a shell `PATH`
/// lookup applies. Symbolic links resolve through the metadata they point at.
fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// The executability predicate over a fixed set of paths, so no case
    /// touches the real filesystem.
    fn executable_set(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let executable: std::collections::HashSet<PathBuf> =
            paths.iter().map(PathBuf::from).collect();
        move |path: &Path| executable.contains(path)
    }

    /// The resolution-order contract (design D9): the executable directory
    /// wins over `PATH`, the first `PATH` entry wins over later ones, and no
    /// executable `mbv` anywhere resolves to nothing. The cases have
    /// different outcomes, so one `#[case]` table carries them.
    #[rstest]
    #[case::executable_directory_wins(
        Some("/opt/mbv-gui/bin"),
        "/usr/local/bin:/usr/bin",
        &["/usr/local/bin/mbv", "/opt/mbv-gui/bin/mbv"],
        Some("/opt/mbv-gui/bin/mbv"),
    )]
    #[case::first_path_entry_wins(
        Some("/opt/mbv-gui/bin"),
        "/usr/local/bin:/usr/bin",
        &["/usr/local/bin/mbv", "/usr/bin/mbv"],
        Some("/usr/local/bin/mbv"),
    )]
    #[case::not_found(
        Some("/opt/mbv-gui/bin"),
        "/usr/local/bin:/usr/bin",
        &[],
        None,
    )]
    fn resolution_prefers_the_executable_directory_then_path_order(
        #[case] exe_dir: Option<&str>,
        #[case] path_value: &str,
        #[case] executable: &[&str],
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            resolve_owner_program(
                exe_dir.map(Path::new),
                path_value,
                executable_set(executable)
            )
            .as_deref(),
            expected.map(Path::new)
        );
    }
}
