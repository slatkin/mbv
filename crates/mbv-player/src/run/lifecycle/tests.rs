//! Private-item unit tests for the run-lifecycle machine: the
//! single exit's close per run identity, the load-failure verdict, and the
//! accepted-submission reset. No `Mpv`, no channel — a bare machine and
//! plain facts.

use super::RunLifecycle;
use crate::run::{FailurePhase, FailureVerdict, ForcedJump, JumpArm, QueueSlotId, RunPosition};
use rstest::rstest;

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

/// The contract "a new submission starts the counts again": the accepted
/// submission's own failure is a start failure (#941) even when an earlier
/// submission had begun, so `submission_accepted` clears the
/// "has playback begun" boundary.
#[test]
fn submission_accepted_resets_the_playback_begun_boundary() {
    let mut lifecycle = RunLifecycle::default();
    lifecycle.playback_started();

    lifecycle.submission_accepted();

    assert!(
        !lifecycle.began,
        "the accepted submission starts the counts again"
    );
}

/// The contract "an Audiobookshelf jump arms no resume seek": every
/// Audiobookshelf queue is active-file, and that projection never moves mpv
/// with a `playlist-pos` write, so `arm_jump` there discards the arm whatever
/// the jump carries. `take_resume_ticks` then yields `None`, so a resume
/// position baked into the prepared source is never applied again as a seek.
#[test]
fn arm_jump_in_the_active_file_projection_arms_no_resume_seek() {
    let mut lifecycle = RunLifecycle::default();
    lifecycle.enter_active_file();

    lifecycle.arm_jump(ForcedJump {
        slot_id: slot(7),
        transition: None,
        resume_ticks: Some(42),
        from_idle: true,
    });

    assert!(
        lifecycle.jump_arm().is_idle(),
        "the active-file projection arms no jump"
    );
    assert_eq!(lifecycle.take_resume_ticks(), None);
}

/// The plain position facts one row drives, with slots
/// distinct so a verdict built from a stale entry cannot pass the naming.
struct VerdictFacts {
    began: bool,
    active_file: bool,
    jump: JumpArm,
    position: RunPosition,
}

/// A fixture slot; distinct raw values separate the entry played before a
/// jump from the jump target.
fn slot(raw: u64) -> QueueSlotId {
    QueueSlotId::from_raw(raw)
}

/// The plain position facts of one row. `current_slot` names the entry mpv
/// reports (the stale one for the jump rows); `target_idx` names the armed
/// target's ordinal when it resolves.
fn playlist_position(
    current_idx: usize,
    queue_len: usize,
    target_idx: Option<usize>,
) -> RunPosition {
    RunPosition {
        current_idx,
        queue_len,
        current_slot: Some(slot(1)),
        jump_target_idx: target_idx,
    }
}

/// A pending arm that jumped from idle: no outgoing
/// end-file settles it, so the entry mpv reports stays stale.
fn pending_from_idle(target: QueueSlotId) -> JumpArm {
    JumpArm::Pending(ForcedJump {
        slot_id: target,
        transition: None,
        resume_ticks: None,
        from_idle: true,
    })
}

/// A settled resume-only arm: the jumped target's end
/// settled already and its seek still waits to land.
fn resume_only(target: QueueSlotId) -> JumpArm {
    JumpArm::ResumeOnly {
        slot_id: target,
        resume_ticks: 42,
    }
}

/// The contract "the load-failure verdict" in one table:
/// the failed entry comes from the armed jump when one is in flight —
/// from mpv's event order, an end-file error that arrives while a jump is
/// armed belongs to the jump target — and from the entry mpv is on
/// otherwise; the arm clears on
/// every verdict; and the run ends only when nothing can play after the
/// failed entry. `failed_slot` in an `EndRun` names that entry.
#[rstest]
#[case::before_start_ends_naming_the_entry_941(
    VerdictFacts {
        began: false,
        active_file: false,
        jump: JumpArm::Idle,
        position: playlist_position(0, 2, None),
    },
    FailureVerdict::EndRun {
        failed_slot: Some(slot(1)),
        phase: FailurePhase::BeforeStart,
    },
)]
#[case::after_start_failure_with_a_next_entry_continues(
    VerdictFacts {
        began: true,
        active_file: false,
        jump: JumpArm::Idle,
        position: playlist_position(0, 2, None),
    },
    FailureVerdict::Continue,
)]
#[case::after_start_last_entry_ends_942_943(
    VerdictFacts {
        began: true,
        active_file: false,
        jump: JumpArm::Idle,
        position: playlist_position(1, 2, None),
    },
    FailureVerdict::EndRun {
        failed_slot: Some(slot(1)),
        phase: FailurePhase::AfterStart,
    },
)]
#[case::active_file_ends_even_with_slots_after_it_942_943(
    VerdictFacts {
        began: true,
        active_file: true,
        jump: JumpArm::Idle,
        position: playlist_position(0, 3, None),
    },
    FailureVerdict::EndRun {
        failed_slot: Some(slot(1)),
        phase: FailurePhase::AfterStart,
    },
)]
#[case::from_idle_last_target_ends_naming_the_target(
    VerdictFacts {
        began: true,
        active_file: false,
        jump: pending_from_idle(slot(7)),
        position: playlist_position(0, 2, Some(1)),
    },
    FailureVerdict::EndRun {
        failed_slot: Some(slot(7)),
        phase: FailurePhase::AfterStart,
    },
)]
#[case::resume_only_target_continues_and_clears_the_arm(
    VerdictFacts {
        began: true,
        active_file: false,
        jump: resume_only(slot(7)),
        position: playlist_position(0, 3, Some(1)),
    },
    FailureVerdict::Continue,
)]
fn load_failed_judges_the_failure_verdict(
    #[case] facts: VerdictFacts,
    #[case] expected: FailureVerdict,
) {
    let mut lifecycle = RunLifecycle::default();
    if facts.began {
        lifecycle.playback_started();
    }
    if facts.active_file {
        lifecycle.enter_active_file();
    }
    // The arm is a fact input: no single setter reaches the `ResumeOnly`
    // state, so the row plants it as the table's fact.
    lifecycle.jump = facts.jump; // module-private field write

    let verdict = lifecycle.load_failed(facts.position);

    assert_eq!(verdict, expected);
    if !facts.jump.is_idle() {
        assert!(
            lifecycle.jump_arm().is_idle(),
            "every verdict clears an armed jump"
        );
    }
}
