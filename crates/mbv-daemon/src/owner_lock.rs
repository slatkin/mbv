//! Owner-lock resolution, probing, and signalling.
//!
//! A live Player owner holds an advisory `flock` on its PID record
//! (`mbv.lock` via the single-instance lock, `mbv.pid` when packaged, ADR
//! 0006; harden-owner-process-boundaries decision D1). The lock is the
//! liveness truth: a PID is only read out — and only signalled — while
//! another descriptor still holds the flock on the same file.
//!
//! Single-instance detection lives here too (`resolve`), so one function
//! serves the Owner's lock holder ([`lock_pid_file`]), the Client's Owner
//! discovery (`owner_link::find_owner`), and the Owner-restart and
//! session-switch probes. Stay-alive is never a factor: the flock is always
//! taken. The runtime directory's `mbv.lock` (`$XDG_RUNTIME_DIR/mbv.lock`,
//! `/run/mbv/mbv.lock` for a system instance, or the private
//! `/tmp/mbv-<uid>/mbv.lock` fallback) is held for the whole lifetime of the
//! Player-owning app. The kernel auto-releases it on any process death, so a
//! held lock always means a live app — there is no stale-lock case to reason
//! about. Resolution is a non-blocking flock: acquired -> fresh start;
//! would-block -> a live app exists, and socket connectability (never file
//! existence) disambiguates "a local daemon exists, attach as a client" from
//! "bare foreground TUI owns it, refuse".

use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;

/// The PID file the Owner processes share. Packaged `mbvd` and the Local
/// owner both use it; whenever either `mbv.pid` holds a PID, it points at
/// this process (harden-owner-process-boundaries D2).
#[must_use]
pub fn pid_file() -> PathBuf {
    let dir = mbv_config::data_dir_system_or_local();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("mbv.pid")
}

/// The PID of the process that currently holds the flock on `path`.
///
/// Opens the file read-only and never creates it. A missing file, an
/// unholdable lock, or a file whose contents are not a PID all yield
/// `None`: nobody is holding the lock, so the record is stale or empty.
#[must_use]
pub fn locked_owner_pid(path: &Path) -> Option<u32> {
    let file = File::open(path).ok()?;
    let probe = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock);
    // Only a would-block result means a live owner holds the lock; read the
    // PID out of the record through the descriptor that holds it. Any other
    // outcome — a freely acquired probe lock (dropped below) or another error
    // — names no owner.
    if let Err((mut file, nix::errno::Errno::EWOULDBLOCK)) = probe {
        let mut pid = String::new();
        file.read_to_string(&mut pid).ok()?;
        return pid.trim().parse().ok();
    }
    None
}

/// The exclusive `flock` guard on the PID record, held for the lifetime of
/// the packaged Owner's run (harden-owner-process-boundaries D2). The PID
/// is written only after the lock is acquired, so the file's PID is the
/// live lock holder the guard represents. Dropping it releases the flock
/// (which also happens automatically on any process death).
pub type PidFileLock = nix::fcntl::Flock<File>;
/// Why a packaged Owner process could not take its PID record and start.
///
/// The record's flock is already held by another Owner process (so a
/// daemon is already running), or the record itself could not be opened,
/// truncated, or written. The entry point that starts the daemon prints
/// this error and exits non-zero (harden-owner-process-boundaries 1.3).
#[derive(Debug)]
pub struct OwnerLockError {
    /// The PID record whose flock could not be taken.
    pub path: PathBuf,
    /// The acquisition error; a `WouldBlock` kind names a live lock holder.
    pub error: io::Error,
}

impl std::fmt::Display for OwnerLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "could not take the owner PID lock at {}: {}",
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for OwnerLockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// An `OwnerLockError` naming the record `error` occurred at.
fn held_error(path: &Path, error: io::Error) -> OwnerLockError {
    OwnerLockError {
        path: path.to_path_buf(),
        error,
    }
}

/// Acquire the exclusive non-blocking flock on the PID record at `path`,
/// creating it if missing. The contents are left in place. `Ok(None)` means
/// another descriptor already holds the lock.
pub fn try_lock(path: &Path) -> io::Result<Option<PidFileLock>> {
    // Intentionally not truncated: the file may hold a previous PID; it is
    // truncated by `write_pid`, once this process holds the lock.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    match nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock) {
        Ok(lock) => Ok(Some(lock)),
        Err((_, nix::errno::Errno::EWOULDBLOCK)) => Ok(None),
        Err((_, errno)) => Err(io::Error::from_raw_os_error(errno as i32)),
    }
}

/// Replace the PID record's contents with this process's PID. Only call it
/// under the flock, so the record names the live lock holder.
pub fn write_pid(file: &mut File) -> io::Result<()> {
    file.set_len(0)?;
    file.rewind()?;
    write!(file, "{}", std::process::id())
}

/// Acquire the exclusive non-blocking flock on `path` and write this
/// process's PID into it. Fails with `OwnerLockError` when another Owner
/// process already holds the lock or the record cannot be opened, truncated,
/// or written; the returned `PidFileLock` otherwise keeps the flock for the
/// run's lifetime.
pub fn lock_pid_file(path: &Path) -> Result<PidFileLock, OwnerLockError> {
    let mut lock = match try_lock(path) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            return Err(held_error(
                path,
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another Owner process holds the lock",
                ),
            ));
        }
        Err(error) => return Err(held_error(path, error)),
    };
    write_pid(&mut lock).map_err(|error| held_error(path, error))?;
    Ok(lock)
}

/// The control socket this machine's Owner binds and its Clients connect to.
#[must_use]
pub fn socket_path() -> PathBuf {
    PathBuf::from(mbv_config::control_socket_path())
}

/// The result of [`resolve`]: who may be the Player-owning app now.
pub enum Resolution {
    /// No live app holds the lock: acquired it fresh. Caller owns the
    /// guard for as long as it wants to be the Player-owning app.
    Fresh(LockGuard),
    /// A live app holds the lock AND its control socket is connectable: a
    /// live local daemon exists. Caller should attach as a client alongside
    /// any others already attached.
    Attach,
    /// A live app holds the lock but the control socket is refused/absent:
    /// the Owner process is not accepting connections. Caller should
    /// refuse.
    Refuse,
}

/// Raised when a message does not fit a consumer's expectations.
///
/// The lock file is held for the process lifetime of the Player-owning app:
/// dropping it releases the flock (which also happens automatically on any
/// process death).
pub struct LockGuard {
    file: PidFileLock,
}

impl std::fmt::Debug for LockGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockGuard").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fresh(guard) => f.debug_tuple("Fresh").field(guard).finish(),
            Self::Attach => f.write_str("Attach"),
            Self::Refuse => f.write_str("Refuse"),
        }
    }
}

impl LockGuard {
    /// Overwrite the lock file with this process's PID, so `mbv -q` /
    /// tray-Quit can find it to send SIGTERM.
    pub fn write_pid(&mut self) -> io::Result<()> {
        write_pid(&mut self.file)
    }
}

/// Non-blocking flock probe + (if held) socket-connectability check.
pub fn resolve(socket: &Path, lock: &Path) -> io::Result<Resolution> {
    let Some(file) = try_lock(lock)? else {
        // A live app holds the lock. Never trust socket-file existence — only
        // a successful connect counts (ADR 0006).
        return match UnixStream::connect(socket) {
            Ok(_) => Ok(Resolution::Attach),
            Err(_) => Ok(Resolution::Refuse),
        };
    };
    let _ = std::fs::set_permissions(lock, std::fs::Permissions::from_mode(0o600));
    Ok(Resolution::Fresh(LockGuard { file }))
}

/// Why an Owner process could not be signalled.
#[derive(Debug)]
pub enum SignalOwnerError {
    /// The lock file held no held-lock PID, so there is no Owner to signal.
    NoOwner,
    /// `SIGTERM` to the lock file's PID failed.
    Signal { pid: u32, error: io::Error },
}

impl std::fmt::Display for SignalOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOwner => {
                write!(
                    f,
                    "no running instance found; if one just started, try again in a moment"
                )
            }
            Self::Signal { pid, error } => write!(f, "failed to signal pid {pid}: {error}"),
        }
    }
}

impl std::error::Error for SignalOwnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoOwner => None,
            Self::Signal { error, .. } => Some(error),
        }
    }
}

/// Send `SIGTERM` to the PID that currently holds the flock on `path`.
pub fn signal_owner(path: &Path) -> Result<u32, SignalOwnerError> {
    let pid = locked_owner_pid(path).ok_or(SignalOwnerError::NoOwner)?;
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid.cast_signed()),
        nix::sys::signal::Signal::SIGTERM,
    )
    .map(|()| pid)
    .map_err(|errno| SignalOwnerError::Signal {
        pid,
        error: io::Error::from_raw_os_error(errno as i32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// Temp dir removed on drop, so a panicking assertion cannot leak it.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("mbv-si-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `Fresh` + PID round-trip: a free lock is acquired and, while that guard
    /// still holds the flock, `locked_owner_pid` (the `mbv -q` probe) must
    /// recover this process's id.
    #[test]
    fn fresh_lock_round_trips_the_pid() {
        let dir = TempDir::new();
        let lock = dir.path().join("mbv.lock");
        let socket = dir.path().join("mbv.sock");

        let Ok(Resolution::Fresh(mut guard)) = resolve(&socket, &lock) else {
            panic!("expected a fresh lock on an unused path");
        };
        guard.write_pid().expect("write pid under the held lock");
        assert_eq!(locked_owner_pid(&lock), Some(std::process::id()));
    }

    /// Held lock: `Attach` iff the socket actually connects. A plain file at the
    /// socket path must not count (ADR 0006). The held lock is reproducible
    /// because flocks attach to the open file description.
    #[test]
    fn held_lock_attaches_only_to_a_connectable_socket() {
        let dir = TempDir::new();
        let lock = dir.path().join("mbv.lock");
        let socket = dir.path().join("mbv.sock");

        let Ok(Resolution::Fresh(_guard)) = resolve(&socket, &lock) else {
            panic!("expected the first resolve to take a fresh lock");
        };

        // No socket file at all: a bare foreground TUI owns the lock.
        assert!(matches!(resolve(&socket, &lock), Ok(Resolution::Refuse)));

        // A non-listening file at the socket path must not be mistaken for a
        // daemon.
        std::fs::write(&socket, b"").expect("seed socket path with a plain file");
        assert!(matches!(resolve(&socket, &lock), Ok(Resolution::Refuse)));

        // A real listener: attach as a client.
        std::fs::remove_file(&socket).expect("clear path for bind");
        let listener = UnixListener::bind(&socket).expect("bind test listener");
        assert!(matches!(resolve(&socket, &lock), Ok(Resolution::Attach)));
        drop(listener);
    }
}
