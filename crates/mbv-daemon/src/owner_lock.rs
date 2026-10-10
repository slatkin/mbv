//! Owner-lock probing and signalling.
//!
//! A live Player owner holds an advisory `flock` on its PID record
//! (`mbv.lock` via the single-instance lock, `mbv.pid` when packaged, ADR
//! 0006; harden-owner-process-boundaries decision D1). The lock is the
//! liveness truth: a PID is only read out — and only signalled — while
//! another descriptor still holds the flock on the same file.

use std::fs::File;
use std::io::{self, Seek, Write};
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
    // PID out of the record. Any other outcome — a freely acquired probe
    // lock (dropped below) or another error — names no owner.
    if let Err((_, nix::errno::Errno::EWOULDBLOCK)) = probe {
        return std::fs::read_to_string(path).ok()?.trim().parse().ok();
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

/// Acquire the exclusive non-blocking flock on `path` and write this
/// process's PID into it. Fails with `OwnerLockError` when another Owner
/// process already holds the lock or the record cannot be opened, truncated,
/// or written; the returned `PidFileLock` otherwise keeps the flock for the
/// run's lifetime.
pub fn lock_pid_file(path: &Path) -> Result<PidFileLock, OwnerLockError> {
    // Intentionally not truncated: the file may hold a previous PID; it
    // is truncated below, once this process holds the lock.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| held_error(path, error))?;
    let mut file = match nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock)
    {
        Ok(file) => file,
        Err((_, nix::errno::Errno::EWOULDBLOCK)) => {
            return Err(held_error(
                path,
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another Owner process holds the lock",
                ),
            ));
        }
        Err((_, errno)) => {
            return Err(held_error(path, io::Error::from_raw_os_error(errno as i32)));
        }
    };
    file.set_len(0).map_err(|error| held_error(path, error))?;
    file.seek(io::SeekFrom::Start(0))
        .map_err(|error| held_error(path, error))?;
    write!(file, "{}", std::process::id()).map_err(|error| held_error(path, error))?;
    file.flush().map_err(|error| held_error(path, error))?;
    Ok(file)
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
    // SAFETY: signalling the PID read from the held owner lock is intentional.
    let ok = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0;
    if ok {
        Ok(pid)
    } else {
        Err(SignalOwnerError::Signal {
            pid,
            error: io::Error::last_os_error(),
        })
    }
}
