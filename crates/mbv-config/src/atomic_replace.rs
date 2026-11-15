// Shared atomic file replace: the create-dir, temp-file, rename rule every
// small persisted file in this crate writes by. Serialisation stays with
// each caller — JSON for the launch-state snapshots (`launch_state.rs`),
// TOML for the GUI panel widths (`gui_panel_widths.rs`) — so this module
// owns only the directory creation, the process-unique temporary path, and
// the rename.

use std::path::{Path, PathBuf};

/// Which step of the shared atomic replace failed. Callers turn it into
/// their own error vocabulary without re-deriving the phase from a bare
/// [`std::io::Error`].
#[derive(Debug)]
pub(super) enum AtomicReplaceStep {
    /// Creating the target's parent directory failed.
    CreateDirectory { dir: PathBuf },
    /// Writing the temporary sibling file failed.
    Write { tmp: PathBuf },
    /// Renaming the temporary over the target failed; the temporary was
    /// removed best-effort before this reaches the caller.
    Replace { tmp: PathBuf },
}

impl std::fmt::Display for AtomicReplaceStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDirectory { dir } => write!(f, "create directory {}", dir.display()),
            Self::Write { tmp } => write!(f, "write {}", tmp.display()),
            Self::Replace { tmp } => write!(f, "replace from {}", tmp.display()),
        }
    }
}

/// A failure from [`write_bytes_atomically`]: the step that failed and its
/// IO source.
#[derive(Debug)]
pub(super) struct AtomicReplaceError {
    pub step: AtomicReplaceStep,
    pub source: std::io::Error,
}

impl std::fmt::Display for AtomicReplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.step, self.source)
    }
}

impl std::error::Error for AtomicReplaceError {}

/// Create `path`'s parent directory, write `bytes` to a process-unique
/// temporary file beside `path` ([`atomic_replace_tmp_path`]), and
/// atomically rename it over `path`: a killed or crashed writer leaves the
/// previous completed file intact, and only a finished rename is ever
/// observed by the next reader. Two concurrently exiting Clients never
/// share a temp path and cannot corrupt each other's replace.
pub(super) fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Result<(), AtomicReplaceError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| AtomicReplaceError {
            step: AtomicReplaceStep::CreateDirectory {
                dir: dir.to_path_buf(),
            },
            source,
        })?;
    }
    let tmp = atomic_replace_tmp_path(path);
    std::fs::write(&tmp, bytes).map_err(|source| AtomicReplaceError {
        step: AtomicReplaceStep::Write { tmp: tmp.clone() },
        source,
    })?;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AtomicReplaceError {
            step: AtomicReplaceStep::Replace { tmp },
            source: error,
        });
    }
    Ok(())
}

/// Process-unique sibling path for the atomic replace: same directory (so
/// the rename stays atomic), qualified by pid and a fresh uuid so two
/// Clients writing concurrently never share a pathname. The name is derived
/// from the target's file name, so callers filter debris by the target's
/// own `.tmp-` prefix.
pub(super) fn atomic_replace_tmp_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().map_or_else(
        || "atomic-replace-target".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!(
        "{file_name}.tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}
