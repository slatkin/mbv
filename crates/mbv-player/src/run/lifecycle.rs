//! Run-lifecycle facts for one playback run, held in place of loose flags on
//! `PlaybackRun` (design D1 of the `playback-run-lifecycle` change).
//!
//! The module owns the "has playback begun" boundary: a generic mpv failure
//! before the first `PlaybackRestart` is a start failure and stops the run
//! with an error (#941); after it, mpv owns skipping a failed entry and the
//! playlist-position drift machinery follows it — unless no entry follows
//! the failed one, where the run still ends with the error (#942, #943).
//! It also owns which projection feeds mpv: the active-file projection keeps
//! exactly one entry in mpv's playlist and never returns to the playlist
//! projection. The facts take plain values (`PlaybackRestart` observed, a
//! queue of entries), never a raw `MpvEvent`.
//!
//! A forced jump is a `JumpArm` with named states: a `Pending` arm waits for
//! its outgoing end-file — or, when jumped from idle, for its target's
//! `PlaybackRestart` — while a `ResumeOnly` arm has settled and waits for
//! the target entry so its resume seek can land. The old readers inferred
//! which of the two a single `Option<ForcedJump>` held from its `transition`
//! and `from_idle` fields; the named states give each event exactly one
//! transition (design D1).
//!
//! The run's single exit (design D4) is the machine's close: `close(identity)`
//! answers once per run identity whether the caller may send the run's one
//! `Stopped`, and an accepted later submission reopens the guard.

use super::{ExecutionSequence, ForcedJump, QueueSlotId};
use crate::transition::Transition;
use mbv_ctrl::PlaybackGeneration;

#[derive(Copy, Clone, Debug, Default)]
enum Projection {
    /// mpv's playlist carries the run's entries, and `playlist-pos` names
    /// one of them.
    #[default]
    Playlist,
    /// mpv holds only the entry playing (each item is reloaded directly),
    /// so `playlist-pos` never names the run's ordinal.
    ActiveFile,
}

/// What a forced `JumpTo` is waiting on (design D1). `Pending` waits for its
/// outgoing end-file — or, when jumped from idle, for its target
/// `PlaybackRestart`. `ResumeOnly` has settled already (transition marker
/// consumed) and waits for the target entry's restart so its resume seek can
/// land. `Idle` holds no jump.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) enum JumpArm {
    #[default]
    Idle,
    Pending(ForcedJump),
    ResumeOnly {
        slot_id: QueueSlotId,
        resume_ticks: i64,
    },
}

impl JumpArm {
    /// Whether no jump is in flight. A jump in either waiting state belongs
    /// to a transition mpv is still applying, so it mutes the
    /// playlist-position drift adoption.
    #[must_use]
    pub(crate) fn is_idle(self) -> bool {
        matches!(self, JumpArm::Idle)
    }
}

/// The lifecycle facts of one run: the "has playback begun" boundary, the
/// projection that decides whether anything follows a failed entry, and the
/// run identity that already ended. `PlaybackRun` holds this in place of the
/// `playback_began`, `active_file`, and `stopped_event_sent` flags it
/// produced (design D1).
#[derive(Debug, Default)]
pub(crate) struct RunLifecycle {
    /// Whether any `PlaybackRestart` has arrived in this run — the "has
    /// playback begun" boundary. A generic mpv failure before it is a start
    /// failure and stops the run with an error (#941); after it, mpv owns
    /// skipping a failed entry and the playlist-position drift machinery
    /// follows it — unless no entry follows the failed one, where the run
    /// still ends with the error (#942, #943).
    began: bool,
    projection: Projection,
    /// The forced-jump arm (design D1): what a `JumpTo` is waiting on, with
    /// the named states of `JumpArm`.
    jump: JumpArm,
    /// The run identity that already reported its `Stopped` (design D4): the
    /// one-shot guard behind `close`. A later accepted submission clears it
    /// (design D5), so that submission's run can end again.
    ended: Option<PlaybackGeneration>,
}

impl RunLifecycle {
    /// Records a `PlaybackRestart`: playback began in this run.
    pub(crate) fn playback_started(&mut self) {
        self.began = true;
    }

    /// Builds the initial state for a submitted queue: the active-file
    /// projection when the queue holds any Audiobookshelf entries (the
    /// predicate that selects that branch at `PlaybackRun::init_from_queue`),
    /// otherwise the playlist projection. `began` stays false: no
    /// `PlaybackRestart` arrived in the new run.
    #[must_use]
    pub(crate) fn for_queue(queue: &ExecutionSequence) -> Self {
        Self {
            began: false,
            projection: if queue.has_audiobookshelf_entries() {
                Projection::ActiveFile
            } else {
                Projection::Playlist
            },
            jump: JumpArm::Idle,
            ended: None,
        }
    }

    /// Switches the run to the active-file projection. Idempotent; the
    /// projection never returns to `Playlist`.
    pub(crate) fn enter_active_file(&mut self) {
        self.projection = Projection::ActiveFile;
    }

    /// Whether the run is in the active-file projection.
    #[must_use]
    pub(crate) fn is_active_file(&self) -> bool {
        matches!(self.projection, Projection::ActiveFile)
    }

    /// Whether playback began in this run (the #941 boundary).
    #[must_use]
    pub(crate) fn began(&self) -> bool {
        self.began
    }

    /// A new submission was accepted into the same mpv child (design D5):
    /// nothing rules the incoming submission's run identity out of ending,
    /// even if the previous submission already did.
    pub(crate) fn submission_accepted(&mut self) {
        self.ended = None;
    }

    /// Answers the run's single exit (design D4): `true` when this identity
    /// has not reported its `Stopped` yet — the caller may deactivate and
    /// send — and `false` for a second end path over the same identity. The
    /// first close records `identity` as ended.
    pub(crate) fn close(&mut self, identity: PlaybackGeneration) -> bool {
        if self.ended == Some(identity) {
            return false;
        }
        self.ended = Some(identity);
        true
    }

    /// Arms a pending `JumpTo` (design D1): the run is `Pending`, waiting for
    /// the target's outgoing end-file — or, when jumped from idle, for its
    /// target's `PlaybackRestart`. The active-file projection never moves mpv
    /// with a `playlist-pos` write, so there an arm always discards: `Idle`.
    pub(crate) fn arm_jump(&mut self, jump: ForcedJump) {
        self.jump = if self.is_active_file() {
            JumpArm::Idle
        } else {
            JumpArm::Pending(jump)
        };
    }

    /// Disarms the jump arm: nothing is in flight. Every site that aborts an
    /// in-flight jump without it reaching a settle lands here — a failed
    /// `playlist-pos` write, a `QueueRemove` taking the jump's target, and
    /// the item-lifecycle handoff (`begin_item_lifecycle`) where a new item
    /// starting means no jump is waiting for it.
    pub(crate) fn disarm_jump(&mut self) {
        self.jump = JumpArm::Idle;
    }

    /// Records that the run adopted the entry mpv moved to on its own: any
    /// armed resume value belonged to the jumped entry, which is gone, so the
    /// arm goes `Idle`.
    pub(crate) fn entry_adopted(&mut self) {
        self.jump = JumpArm::Idle;
    }

    /// The current arm, for reads that decide against it whole: the drift
    /// mute (`is_idle`), the superseded-jump guard, and the pending
    /// transition marker a playlist jump re-arms with.
    #[must_use]
    pub(crate) fn jump_arm(&self) -> JumpArm {
        self.jump
    }

    /// The queue slot the arm is waiting on: a pending target while
    /// `Pending`, the resume-only target while `ResumeOnly`. The run resolves
    /// it against its queue (identity -> ordinal, design D2) before handing
    /// a settle to the machine, which takes plain values only.
    #[must_use]
    pub(crate) fn jump_target_slot(&self) -> Option<QueueSlotId> {
        match self.jump {
            JumpArm::Pending(jump) => Some(jump.slot_id),
            JumpArm::ResumeOnly { slot_id, .. } => Some(slot_id),
            JumpArm::Idle => None,
        }
    }

    /// Settles the arm on an end-file (design D1), returning `(next_idx,
    /// transition)`: the ordinal of the settled target, and the in-flight
    /// transition to announce, if any. `target_index` is the armed slot
    /// resolved against the run's queue; a removed slot falls back to the
    /// queue neighbor (`current_idx + 1`). When an entry follows the settled
    /// target, a resume-bearing arm re-arms as `ResumeOnly` — waiting for the
    /// target's restart to land its seek — and everything else goes `Idle`;
    /// when the settled target was the last entry the run ends, and the arm
    /// stays `Idle`.
    pub(crate) fn jump_settled_on_end_file(
        &mut self,
        target_index: Option<usize>,
        current_idx: usize,
        queue_len: usize,
    ) -> (usize, Option<Transition>) {
        let next_idx = target_index.unwrap_or(current_idx + 1);
        let (transition, resume) = match std::mem::replace(&mut self.jump, JumpArm::Idle) {
            JumpArm::Idle => (None, None),
            JumpArm::Pending(jump) => (
                jump.transition,
                jump.resume_ticks.map(|t| (jump.slot_id, t)),
            ),
            JumpArm::ResumeOnly {
                slot_id,
                resume_ticks,
            } => (None, Some((slot_id, resume_ticks))),
        };
        if next_idx < queue_len
            && let Some((slot_id, resume_ticks)) = resume
        {
            self.jump = JumpArm::ResumeOnly {
                slot_id,
                resume_ticks,
            };
        }
        (next_idx, transition)
    }

    /// Settles the arm when a `PlaybackRestart` starts its target (design
    /// D1): only a `Pending` jump that armed from idle settles here — an idle
    /// jump has no outgoing end-file to wait on — and the arm is untouched
    /// otherwise. The transition is returned for the announce; a
    /// resume-bearing arm re-arms as `ResumeOnly` (waiting for the target's
    /// restart to land its seek) and a no-resume arm goes `Idle`.
    pub(crate) fn jump_settled_on_restart(&mut self) -> Option<(QueueSlotId, Option<Transition>)> {
        let JumpArm::Pending(jump) = self.jump else {
            return None;
        };
        if !jump.from_idle {
            return None;
        }
        self.jump = match jump.resume_ticks {
            Some(resume_ticks) => JumpArm::ResumeOnly {
                slot_id: jump.slot_id,
                resume_ticks,
            },
            None => JumpArm::Idle,
        };
        Some((jump.slot_id, jump.transition))
    }

    /// Takes the arm's resume ticks for an entry that just loaded so the run
    /// can seek to it, resolving the arm to `Idle` — the resume it was
    /// waiting for is now served. `None` when no arm carries ticks: only an
    /// arm carrying ticks is resolved, while a `Pending` arm without ticks
    /// stays in flight, still waiting for its end-file or restart.
    pub(crate) fn take_resume_ticks(&mut self) -> Option<i64> {
        let resume_ticks = match self.jump {
            JumpArm::Pending(jump) => jump.resume_ticks,
            JumpArm::ResumeOnly { resume_ticks, .. } => Some(resume_ticks),
            JumpArm::Idle => None,
        }?;
        self.jump = JumpArm::Idle;
        Some(resume_ticks)
    }
}

#[cfg(test)]
mod tests;
