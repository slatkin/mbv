// Path helper functions extracted from config_types_paths.rs.
// Types and config_dir/cache_dir/state_dir/is_system_instance come from config_types_paths.rs.

use super::{cache_dir, config_dir, is_system_instance, state_dir};
use std::env;
use std::io;
use std::path::{Path, PathBuf};

pub fn data_dir_system_or_local() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        },
        PathBuf::from,
    );
    base.join("mbv")
}

#[must_use]
pub fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

#[must_use]
pub fn stay_alive_queue_state_path() -> PathBuf {
    state_dir().join("stay_alive_queue_state.json")
}

#[must_use]
pub fn library_position_state_path() -> PathBuf {
    state_dir().join("library_position_state.json")
}

#[must_use]
pub fn home_latest_launch_path() -> PathBuf {
    state_dir().join("home_latest_launch.json")
}

/// Visibility/size of the now-playing panel, cycled with `h` and remembered across restarts.
pub(super) fn migrate_to_state(filename: &str) -> PathBuf {
    let dest = state_dir().join(filename);
    if dest.exists() {
        return dest;
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = cache_dir().join(filename);
    if cache.exists() {
        let _ = std::fs::rename(&cache, &dest);
        return dest;
    }
    let old = config_dir().join(filename);
    if old.exists() {
        let _ = std::fs::rename(&old, &dest);
    }
    dest
}

/// The outcome of resolving the mpv overlay script set (or its fonts):
/// the source handed to mpv, plus any ignored copy at the removed
/// installer's user-directory path (named in a startup warning, never used).
#[derive(Debug)]
pub struct ScriptSource {
    pub chosen: PathBuf,
    pub unused_legacy: Option<PathBuf>,
}

/// Pure resolution over injected candidates (B3): the checkout entry wins
/// when it exists, else the packaged path. `legacy` is never a candidate;
/// it is only reported when it exists so startup can warn that a
/// removed-installer copy is being ignored. No environment is read.
#[must_use]
pub fn resolve_script_source(checkout: PathBuf, package: PathBuf, legacy: PathBuf) -> ScriptSource {
    let chosen = if checkout.exists() { checkout } else { package };
    let unused_legacy = legacy.exists().then_some(legacy);
    ScriptSource {
        chosen,
        unused_legacy,
    }
}

/// Compile-time checkout root, derived from the manifest directory
/// (`<checkout>/crates/mbv-core`). Absent on installed systems, so the
/// packaged copy wins there.
pub(super) fn checkout_scripts_entry() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/mbv.lua"
    ))
}

pub(super) fn checkout_fonts_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts"))
}

#[must_use]
pub fn osc_script_source() -> ScriptSource {
    resolve_script_source(
        checkout_scripts_entry(),
        PathBuf::from("/usr/share/mbv/scripts/mbv.lua"),
        data_dir_system_or_local().join("scripts").join("mbv.lua"),
    )
}

#[must_use]
pub fn osc_fonts_source() -> ScriptSource {
    resolve_script_source(
        checkout_fonts_dir(),
        PathBuf::from("/usr/share/mbv/fonts"),
        data_dir_system_or_local().join("fonts"),
    )
}

#[must_use]
pub fn prefs_path() -> PathBuf {
    migrate_to_state("prefs.json")
}

#[must_use]
pub fn osc_fonts_dir() -> PathBuf {
    osc_fonts_source().chosen
}

pub(super) fn runtime_dir() -> String {
    if is_system_instance() {
        return "/run/mbv".to_string();
    }
    session_runtime_dir().unwrap_or_else(|| format!("/tmp/mbv-{}", current_uid()))
}

/// `$XDG_RUNTIME_DIR` when the session provides one. `runtime_dir` and the
/// boundary check in `ensure_runtime_dir` both read it through here, so they
/// agree on when the private `/tmp/mbv-<uid>` fallback is in use.
fn session_runtime_dir() -> Option<String> {
    env::var("XDG_RUNTIME_DIR").ok()
}

fn current_uid() -> u32 {
    nix::unistd::getuid().as_raw()
}

/// The owner-lock file in the runtime directory (harden-owner-process-
/// boundaries D4). One rule for where the Owner-process lock lives, shared
/// by single-instance resolution, daemon restart, and `mbv -q`.
#[must_use]
pub fn owner_lock_path() -> PathBuf {
    PathBuf::from(runtime_dir()).join("mbv.lock")
}

/// Why a per-user runtime directory is refused.
#[derive(Debug)]
pub enum RuntimeDirErrorReason {
    /// The path is a symlink: never follow a planted symlink.
    Symlink,
    /// The path exists but is not a directory.
    NotADirectory,
    /// The directory is owned by another user.
    ForeignOwner { owner_uid: u32 },
    /// The directory grants group or other access (`mode & 0o077` nonzero).
    Permissive,
    /// The directory could not be created or inspected.
    Io(io::Error),
}

/// Why a fallback runtime directory could not be made private to this user.
/// It names the directory and the reason (harden-owner-process-boundaries
/// D4); the entry point that calls `ensure_runtime_dir` prints it and exits
/// non-zero.
#[derive(Debug)]
pub struct RuntimeDirError {
    /// The runtime directory that was refused or failed.
    pub path: PathBuf,
    /// Why it cannot be used.
    pub reason: RuntimeDirErrorReason,
}

impl std::fmt::Display for RuntimeDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime directory {}: ", self.path.display())?;
        match &self.reason {
            RuntimeDirErrorReason::Symlink => f.write_str("it is a symlink"),
            RuntimeDirErrorReason::NotADirectory => f.write_str("it is not a directory"),
            RuntimeDirErrorReason::ForeignOwner { owner_uid } => {
                write!(f, "it is owned by uid {owner_uid}")
            }
            RuntimeDirErrorReason::Permissive => f.write_str("it grants group or other access"),
            RuntimeDirErrorReason::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RuntimeDirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.reason {
            RuntimeDirErrorReason::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Guarantee that the fallback runtime directory is private to this user
/// (harden-owner-process-boundaries D4). Nothing to do when the session
/// provides `$XDG_RUNTIME_DIR` or the instance is a system instance. The
/// fallback `/tmp/mbv-<uid>` is created owner-only when missing. An existing
/// directory is checked, never repaired: a symlink, a plain file, another
/// user's directory, or one granting group or other access is refused
/// before any lock or socket is taken.
pub fn ensure_runtime_dir() -> Result<(), RuntimeDirError> {
    if is_system_instance() || session_runtime_dir().is_some() {
        return Ok(());
    }
    let path = PathBuf::from(runtime_dir());
    match std::fs::create_dir(&path) {
        Ok(()) => {
            use std::os::unix::fs::PermissionsExt;
            // A fresh directory carries the process umask; the created
            // directory must be owner-only regardless of the umask's
            // group/other bits. A failed chmod is caught by the check.
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
        }
        // An existing directory is fine; its shape is checked below.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(RuntimeDirError {
                path,
                reason: RuntimeDirErrorReason::Io(error),
            });
        }
    }
    check_private_dir(&path, current_uid())
}

/// Check (via `lstat`, which never follows a planted symlink) that `path`
/// is a plain directory owned by `uid` with no group or other access.
/// `uid` is a parameter so a test can cover the foreign-owner case without
/// a second account (harden-owner-process-boundaries D4).
fn check_private_dir(path: &Path, uid: u32) -> Result<(), RuntimeDirError> {
    use std::os::unix::fs::MetadataExt;
    let refused = |reason| RuntimeDirError {
        path: path.to_path_buf(),
        reason,
    };
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| refused(RuntimeDirErrorReason::Io(error)))?;
    if metadata.file_type().is_symlink() {
        return Err(refused(RuntimeDirErrorReason::Symlink));
    }
    if !metadata.is_dir() {
        return Err(refused(RuntimeDirErrorReason::NotADirectory));
    }
    if metadata.uid() != uid {
        return Err(refused(RuntimeDirErrorReason::ForeignOwner {
            owner_uid: metadata.uid(),
        }));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(refused(RuntimeDirErrorReason::Permissive));
    }
    Ok(())
}

/// The default audio-pipe FIFO path: a private name in the per-user runtime
/// directory (issue #918), beside the mpv IPC socket and the control socket.
/// An explicitly configured `audio_pipe_path` in config.toml overrides it.
#[must_use]
pub fn default_audio_pipe_path() -> String {
    format!("{}/mbv-pipe", runtime_dir())
}

#[must_use]
pub fn mpv_ipc_path() -> String {
    format!("{}/mbv-mpv.sock", runtime_dir())
}

#[must_use]
pub fn mpv_config_dir() -> PathBuf {
    PathBuf::from(runtime_dir()).join("mpv-config")
}

#[must_use]
pub fn control_socket_path() -> String {
    format!("{}/mbv-ctrl.sock", runtime_dir())
}

#[must_use]
pub fn token_cache_path() -> PathBuf {
    migrate_to_state("token.json")
}

#[must_use]
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

#[cfg(test)]
mod check_private_dir_tests {
    use super::{RuntimeDirErrorReason, check_private_dir, current_uid};
    use rstest::rstest;
    use std::mem::discriminant;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// The on-disk fixture one table row builds, materialized inside a
    /// unique temp root so parallel tests never collide.
    enum Kind {
        /// Our own 0700 directory.
        PrivateDir,
        /// Our own directory left group/other-accessible.
        PermissiveDir,
        /// A symlink planted at the check path that points at our own
        /// 0700 directory: the symlink is refused, the target is fine.
        SymlinkToPrivateDir,
        /// A plain file at the check path.
        RegularFile,
        /// Our own 0700 directory re-checked as if by another user: the
        /// expected uid is a `check_private_dir` parameter, so the
        /// foreign-owner row needs no second account.
        PrivateDirAnotherUid,
    }

    struct Fixture {
        path: PathBuf,
        uid: u32,
        root: PathBuf,
    }

    fn make_dir_with_mode(root: &std::path::Path, name: &str, mode: u32) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
        dir
    }

    impl Fixture {
        fn materialize(kind: &Kind) -> Self {
            let root =
                std::env::temp_dir().join(format!("mbv-runtime-dir-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&root).unwrap();
            let (path, uid) = match kind {
                Kind::PrivateDir => (make_dir_with_mode(&root, "dir", 0o700), current_uid()),
                Kind::PermissiveDir => (make_dir_with_mode(&root, "dir", 0o755), current_uid()),
                Kind::SymlinkToPrivateDir => {
                    let dir = make_dir_with_mode(&root, "dir", 0o700);
                    let link = root.join("entry");
                    std::os::unix::fs::symlink(&dir, &link).unwrap();
                    (link, current_uid())
                }
                Kind::RegularFile => {
                    let file = root.join("file");
                    std::fs::write(&file, b"payload").unwrap();
                    (file, current_uid())
                }
                Kind::PrivateDirAnotherUid => {
                    (make_dir_with_mode(&root, "dir", 0o700), current_uid() + 1)
                }
            };
            Self { path, uid, root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[rstest]
    #[case::own_private_dir(Kind::PrivateDir, None)]
    #[case::group_or_other_access(Kind::PermissiveDir, Some(RuntimeDirErrorReason::Permissive))]
    #[case::planted_symlink(Kind::SymlinkToPrivateDir, Some(RuntimeDirErrorReason::Symlink))]
    #[case::regular_file(Kind::RegularFile, Some(RuntimeDirErrorReason::NotADirectory))]
    #[case::foreign_owner(
        Kind::PrivateDirAnotherUid,
        Some(RuntimeDirErrorReason::ForeignOwner { owner_uid: 0 })
    )]
    fn check_private_dir_refusal_reasons(
        #[case] kind: Kind,
        #[case] expected: Option<RuntimeDirErrorReason>,
    ) {
        let fixture = Fixture::materialize(&kind);
        let refusal = check_private_dir(&fixture.path, fixture.uid).err();

        // The same discriminant is not enough for the foreign-owner case:
        // the reported uid must name the file's real on-disk owner, not the
        // uid the check was imposed against.
        if let Some(RuntimeDirErrorReason::ForeignOwner { owner_uid }) =
            refusal.as_ref().map(|e| &e.reason)
        {
            use std::os::unix::fs::MetadataExt;
            let on_disk_owner = std::fs::symlink_metadata(&fixture.path)
                .expect("a refused fixture must still exist")
                .uid();
            assert_eq!(*owner_uid, on_disk_owner);
        }

        assert_eq!(
            refusal.as_ref().map(|error| discriminant(&error.reason)),
            expected.as_ref().map(discriminant)
        );
    }
}
