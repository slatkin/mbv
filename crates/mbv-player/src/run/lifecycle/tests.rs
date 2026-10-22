//! Private-item unit tests for the run's single exit (design D4/D6): the
//! close per run identity, answered by the lifecycle machine. No `Mpv`, no
//! channel — a default machine and plain identities.

use super::RunLifecycle;

/// The contract "one close per run identity": the machine answers `true`
/// once, so the single exit sends its `Stopped`; a second end path over the
/// same identity answers `false`; and a later accepted submission in the
/// same mpv child reopens the exit, so that submission's run can end again.
#[test]
fn close_reports_once_per_run_identity_and_reopens_on_a_new_submission() {
    let mut lifecycle = RunLifecycle::default();
    let identity = 7;

    assert!(lifecycle.close(identity), "the first close may send");
    assert!(
        !lifecycle.close(identity),
        "a second end path for the same run identity must not send again"
    );

    // An accepted later submission carries its fresh identity (a SubmitQueue
    // refreshes it; a LoadNew reuses it) and ends again.
    lifecycle.submission_accepted();
    assert!(
        lifecycle.close(identity),
        "the accepted submission's run may end again"
    );
}
