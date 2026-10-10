// The audio-pipe FIFO contract (issue #918): ensure_pipe creates the pipe
// owner-only and never reuses an existing FIFO another account could read
// (foreign owner, or one granting group or other access). The fixtures below
// are the boundary mock: a real FIFO on a unique temp path, never a live mpv
// handle.

use crate::PlayerError;
use crate::runtime::{check_private_fifo, ensure_pipe};
use std::ffi::CString;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique temp path for one test fixture (process id + sequence), never
/// colliding with a parallel test.
fn unique_fifo_path(tag: &str) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("mbv-pipe-test-{}-{tag}-{seq}", std::process::id()))
}

/// Creates a FIFO at `path` with the given permission bits.
fn make_fifo(path: &Path, mode: u32) {
    let cpath = CString::new(path.to_str().expect("temp path has no NUL")).expect("CString");
    // SAFETY: `cpath` is NUL-terminated and remains alive for the call.
    let rc = unsafe { libc::mkfifo(cpath.as_ptr(), mode) };
    assert_eq!(rc, 0, "mkfifo fixture for {path:?} failed");
}

struct PipeFixture {
    path: PathBuf,
}

impl PipeFixture {
    fn fresh(tag: &str) -> Self {
        Self {
            path: unique_fifo_path(tag),
        }
    }
}

impl Drop for PipeFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// `ensure_pipe` must hand mpv only a FIFO it created itself, owner-only:
/// fresh creation succeeds and the mode on disk is exactly 0600 (#918).
#[test]
fn ensure_pipe_creates_owner_only_fifo() {
    let fixture = PipeFixture::fresh("create");

    ensure_pipe(fixture.path.to_str().expect("temp path has no NUL")).expect("creation succeeds");

    let meta = std::fs::metadata(&fixture.path).expect("created FIFO must exist");
    assert!(meta.file_type().is_fifo());
    assert_eq!(meta.mode() & 0o777, 0o600);
}

/// `ensure_pipe` must reuse own private FIFOs without recreating them (#918).
#[test]
fn ensure_pipe_reuses_own_owner_only_fifo() {
    let fixture = PipeFixture::fresh("reuse");
    make_fifo(&fixture.path, 0o600);

    let result = ensure_pipe(fixture.path.to_str().expect("temp path has no NUL"));

    result.expect("an own 0600 FIFO is reusable");
}

/// Runs `check_private_fifo` on a fresh FIFO built with `mode`, against the
/// file's own uid shifted by `uid_delta`. The foreign-owner case needs no
/// second account: the expected uid is a parameter (mirrors
/// `check_private_dir` in mbv-config).
fn check_fixture_fifo(mode: u32, uid_delta: u32) -> Result<(), PlayerError> {
    let fixture = PipeFixture::fresh("decision");
    make_fifo(&fixture.path, mode);
    let meta = std::fs::metadata(&fixture.path).expect("fixture FIFO must exist");
    check_private_fifo(
        fixture.path.to_str().expect("temp path has no NUL"),
        &meta,
        meta.uid().wrapping_add(uid_delta),
    )
}

/// An own, owner-only FIFO is reusable (#918).
#[test]
fn check_private_fifo_accepts_own_owner_only_fifo() {
    check_fixture_fifo(0o600, 0).expect("an own 0600 FIFO is accepted");
}

/// A FIFO another uid owns, or one granting group or other access, is refused,
/// and the refusal names its reason (#918).
#[rstest::rstest]
#[case::foreign_owner_fifo(0o600, 1, "owned by uid")]
#[case::group_or_other_accessible_fifo(0o666, 0, "grants group or other access")]
fn check_private_fifo_refusal_names_its_reason(
    #[case] mode: u32,
    #[case] uid_delta: u32,
    #[case] fragment: &str,
) {
    let refusal = check_fixture_fifo(mode, uid_delta).expect_err("the FIFO must be refused");
    assert!(
        refusal.to_string().contains(fragment),
        "refusal message '{refusal}' omits '{fragment}'"
    );
}
