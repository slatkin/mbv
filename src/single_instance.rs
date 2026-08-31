//! Single-instance detection via advisory flock + socket connectability
//! (ADR 0006). Independent of stay-alive: always on.
//!
//! The Player-owning app (a bare `mbv`, or a local daemon in stay-alive
//! mode) holds an advisory `flock` at `$XDG_RUNTIME_DIR/mbv.lock` for its
//! entire lifetime. The kernel auto-releases it on any process death, so a
//! held lock always means a live app — there is no stale-lock case to
//! reason about. Startup does a non-blocking flock: acquired -> fresh
//! start; would-block -> a live app exists, and socket connectability
//! (never file existence) disambiguates "a local daemon exists, attach as a
//! client" from "bare foreground TUI owns it, refuse".

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn lock_path() -> PathBuf {
    runtime_dir().join("mbv.lock")
}

pub fn socket_path() -> PathBuf {
    PathBuf::from(mbv_core::config::control_socket_path())
}

/// Held for the process lifetime of the Player-owning app. Dropping it
/// releases the flock (also happens automatically on any process death).
pub struct LockGuard {
    file: nix::fcntl::Flock<File>,
}

impl LockGuard {
    /// Overwrite the lock file with this process's PID, so `mbv -q` /
    /// tray-Quit can find it to send SIGTERM.
    pub fn write_pid(&mut self) -> io::Result<()> {
        use std::io::Seek;
        self.file.set_len(0)?;
        self.file.seek(io::SeekFrom::Start(0))?;
        write!(self.file, "{}", std::process::id())?;
        self.file.flush()
    }
}

pub enum Resolution {
    /// No live app holds the lock: acquired it fresh. Caller owns the
    /// guard for as long as it wants to be the Player-owning app.
    Fresh(LockGuard),
    /// A live app holds the lock AND its control socket is connectable: a
    /// live local daemon exists. Caller should attach as a client alongside
    /// any others already attached.
    Attach,
    /// A live app holds the lock but the control socket is refused/absent:
    /// a bare foreground TUI owns it (no local daemon). Caller should
    /// refuse.
    Refuse,
}

/// Non-blocking flock probe + (if held) socket-connectability check.
pub fn resolve(socket: &Path, lock: &Path) -> io::Result<Resolution> {
    // Intentionally not truncated: the file may already hold a previous
    // PID we're about to re-lock over; `write_pid` explicitly truncates
    // once the lock is actually held.
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock)?;
    let file = match nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock) {
        Ok(file) => file,
        Err((_, nix::errno::Errno::EWOULDBLOCK)) => {
            // A live app holds the lock. Never trust socket-file existence — only
            // a successful connect counts (ADR 0006).
            return match UnixStream::connect(socket) {
                Ok(_) => Ok(Resolution::Attach),
                Err(_) => Ok(Resolution::Refuse),
            };
        }
        Err((_, errno)) => return Err(io::Error::from_raw_os_error(errno as i32)),
    };
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(lock, std::fs::Permissions::from_mode(0o600));
    Ok(Resolution::Fresh(LockGuard { file }))
}

/// Read the PID out of the lock file (best-effort, used by `mbv -q`).
pub fn read_pid(lock: &Path) -> Option<u32> {
    std::fs::read_to_string(lock).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_acquires_fresh_when_unlocked() {
        let dir =
            std::env::temp_dir().join(format!("mbv-single-instance-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let lock = dir.join("fresh.lock");
        let sock = dir.join("fresh.sock");
        let _ = std::fs::remove_file(&lock);
        match resolve(&sock, &lock).unwrap() {
            Resolution::Fresh(_) => {}
            _ => panic!("expected Fresh"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_refuses_when_locked_without_socket() {
        let dir =
            std::env::temp_dir().join(format!("mbv-single-instance-test2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let lock = dir.join("held.lock");
        let sock = dir.join("held.sock"); // deliberately never bound
        let _ = std::fs::remove_file(&lock);
        let held = match resolve(&sock, &lock).unwrap() {
            Resolution::Fresh(g) => g,
            _ => panic!("expected Fresh on first probe"),
        };
        // Second probe, lock still held by `held` in this same process:
        // flock is per-open-file-description, so a second independent open
        // does contend.
        match resolve(&sock, &lock).unwrap() {
            Resolution::Refuse => {}
            _ => panic!("expected Refuse (locked, no live socket)"),
        }
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_attaches_when_locked_with_bound_socket() {
        let dir =
            std::env::temp_dir().join(format!("mbv-single-instance-test3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let lock = dir.join("attach.lock");
        let sock = dir.join("attach.sock");
        let _ = std::fs::remove_file(&lock);
        let _ = std::fs::remove_file(&sock);
        let held = match resolve(&sock, &lock).unwrap() {
            Resolution::Fresh(g) => g,
            _ => panic!("expected Fresh on first probe"),
        };
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        match resolve(&sock, &lock).unwrap() {
            Resolution::Attach => {}
            _ => panic!("expected Attach (locked, live listening socket)"),
        }
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
