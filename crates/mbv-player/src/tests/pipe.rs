// The audio-pipe FIFO contract (issue #918): ensure_pipe creates the pipe
// owner-only and never reuses an existing FIFO another account could read
// (foreign owner, or one granting group or other access). The fixtures below
// are the boundary mock: a real FIFO on a unique temp path, never a live mpv
// handle.

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

/// Decision table for reusing an existing FIFO (#918): an own exclusive FIFO
/// is accepted; a foreign owner and a group/other-accessible FIFO are
/// refused, and the refusal names the reason. The foreign-owner row needs no
/// second account: `check_private_fifo` takes the expected uid as a
/// parameter (mirrors `check_private_dir` in mbv-config).
#[rstest::rstest]
#[case::own_private_fifo(0o600, 0, None)]
#[case::foreign_owner_fifo(0o600, 1, Some("owned by uid"))]
#[case::group_or_other_accessible_fifo(0o666, 0, Some("grants group or other access"))]
fn check_private_fifo_refusal_table(
    #[case] mode: u32,
    #[case] uid_delta: u32,
    #[case] expected_detail: Option<&str>,
) {
    let fixture = PipeFixture::fresh("decision");
    make_fifo(&fixture.path, mode);
    let meta = std::fs::metadata(&fixture.path).expect("fixture FIFO must exist");

    let result = check_private_fifo(
        fixture.path.to_str().expect("temp path has no NUL"),
        &meta,
        meta.uid().wrapping_add(uid_delta),
    );

    match (result.err(), expected_detail) {
        (None, None) => {}
        (Some(refusal), Some(fragment)) => {
            let message = refusal.to_string();
            assert!(
                message.contains(fragment),
                "refusal message '{message}' omits '{fragment}'"
            );
        }
        (refusal, expected) => {
            panic!("refusal mismatch: got {refusal:?}, expected detail {expected:?}")
        }
    }
}
