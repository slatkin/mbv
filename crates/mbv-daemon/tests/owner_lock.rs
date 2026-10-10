//! `locked_owner_pid` contract (harden-owner-process-boundaries task 1.1,
//! decision D1): the probe reports a PID only while some other descriptor
//! holds a real `flock` on the file. `signal_owner` sends a real SIGTERM
//! (needs a live-kill target, so not hermetically assertable); its
//! stale-record refusal — the `mbv -q` destructive path from #915 — is
//! covered below (#920).

use mbv_daemon::{SignalOwnerError, lock_pid_file, locked_owner_pid, signal_owner};
use nix::fcntl::{Flock, FlockArg};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Temp dir removed on drop, so a panicking assertion cannot leak it.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-owner-lock-{}", uuid::Uuid::new_v4()));
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

/// Write a PID record and take the exclusive flock a live owner would hold;
/// the returned guard keeps the lock held for the caller.
fn seed_held_lock(path: &Path) -> Flock<File> {
    let mut file = File::create(path).expect("create pid file");
    write!(file, "4242").expect("write pid record");
    Flock::lock(file, FlockArg::LockExclusiveNonblock).expect("take the owner flock")
}

/// A live owner holds the flock: the probe recovers its PID even though both
/// descriptors live in this process (flock conflicts only between separate
/// open file descriptions).
#[test]
fn held_lock_reports_its_pid() {
    let dir = TempDir::new();
    let lock = dir.path().join("mbv.pid");

    let _guard = seed_held_lock(&lock);

    assert_eq!(locked_owner_pid(&lock), Some(4242));
}

/// A held owner lock refuses the packaged Owner's start with an error
/// naming the record (harden-owner-process-boundaries task 1.3): a second
/// `lock_pid_file` hits the already-held flock, fails with `WouldBlock`,
/// and the `OwnerLockError` carrying it names the PID record in its
/// `Display` just as the entry point's printed message must.
#[test]
fn held_lock_refuses_the_second_start_naming_the_path() {
    let dir = TempDir::new();
    let lock = dir.path().join("mbv.pid");
    let _guard = seed_held_lock(&lock);

    let refused = lock_pid_file(&lock).expect_err("a held lock must refuse the second acquisition");
    let message = refused.to_string();

    assert!(matches!(
        refused.error.kind(),
        std::io::ErrorKind::WouldBlock
    ));
    assert_eq!(refused.path, lock);
    assert!(
        message.contains(lock.to_string_lossy().as_ref()),
        "the refusal must name the PID record: {message}"
    );
}

/// An unlocked PID record is stale or empty: the probe takes its own flock
/// freely and reports no owner.
#[test]
fn unlocked_lock_reports_no_owner() {
    let dir = TempDir::new();
    let lock = dir.path().join("mbv.pid");
    std::fs::write(&lock, b"4242").expect("write pid record");

    assert_eq!(locked_owner_pid(&lock), None);
}

/// The probe opens read-only and never creates a missing file.
#[test]
fn missing_lock_reports_no_owner() {
    let dir = TempDir::new();
    let lock = dir.path().join("mbv.pid");

    assert_eq!(locked_owner_pid(&lock), None);
    assert!(!lock.exists(), "the probe must not create the PID file");
}

/// #920 regression for #915 item 1 (the stale-PID `mbv -q` signal): a PID
/// record nobody holds must name no owner even when the record contains a
/// live PID — here the test process itself — because liveness truth is the
/// flock, never the recorded number. `signal_owner` must refuse with
/// `NoOwner` instead of signalling; signalling the recorded PID would kill
/// this test process outright, so a regression is self-demonstrating.
#[test]
fn signal_owner_refuses_a_stale_record_instead_of_signalling() {
    let dir = TempDir::new();
    let lock = dir.path().join("mbv.pid");
    std::fs::write(&lock, std::process::id().to_string()).expect("write a live but unheld pid");

    let refused = signal_owner(&lock).expect_err("a record nobody holds names no owner");

    assert!(matches!(refused, SignalOwnerError::NoOwner));
}
