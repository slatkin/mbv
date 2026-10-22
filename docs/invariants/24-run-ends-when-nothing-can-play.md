# Invariant 24 — A run that cannot play ends, and ends exactly once

**Scope:** `RunLifecycle` (`crates/mbv-player/src/run/lifecycle.rs`),
`PlaybackRun::end_run` (`crates/mbv-player/src/run/state.rs`), and the
load-failure path `stop_for_unrecoverable_error`
(`crates/mbv-player/src/run/events/termination.rs`).

## The invariant

1. A run that cannot play anything ends, and reports one `Stopped` naming the
   failed slot and the error. It does not sit marked active while mpv is idle.
2. A load failure ends the run only when nothing can play after the failed
   entry: no entry follows it, or the run is in the active-file projection.
   With an entry after it, mpv advances on its own and the playlist-position
   drift machinery follows the move.
3. A failed jump is judged from the jump target, never from the entry played
   before it, and it leaves no jump in flight.
4. Each run identity reports exactly one `Stopped`, whichever end path fires.
   A second end path for the same identity reports nothing. A later accepted
   submission carries a new identity and can end again.

## Why it matters

Nothing enforces this in a type. `PlayerEvent::Stopped` is the only signal that
ends a run in the Owner, and every stop path must send it for its own case. mpv
has no obligation to say "I am idle and I have nothing to do": it emits `idle`,
mbv decodes that event to `Ignored`, and `playlist-pos` goes to -1, which
`divergent_entry` drops. So the run's own state is the only witness that
playback stopped. A run that misses that witness keeps `status.active = true`
forever: the now-playing panel freezes, the queue stays bound, and the user sees
no error, because the failure that caused it reached no error path.

The other half is the count. The Owner drops a `Stopped` whose run identity is
not current (`playback_run_identity_is_current` in `crates/mbv-daemon/src/run.rs`),
but it never removes two stops for one current identity: a duplicated `Stopped`
re-applies its stop observation to the same slot (`apply_stopped_observation` →
`apply_completion_progress`) and relays the event to Clients again.
Exactly-once is therefore a Player-side duty, not a filter the Owner can apply.
Before the `playback-run-lifecycle` change, the send was guarded by
`stopped_event_sent` plus the Service report state, so a fatal error after an
adoption sent no `Stopped` at all, while a failed active-file replacement let a
later quit send a second one.

## How the code maintains it today

- One module owns the state. `RunLifecycle` (`run/lifecycle.rs`) holds the
  "has playback begun" boundary (`began`), the projection (`Projection`, read
  through `is_active_file()` and seeded by `for_queue` from
  `ExecutionSequence::has_audiobookshelf_entries` in `PlaybackRun::init_from_queue`),
  the jump arm (`JumpArm`: `Idle`, `Pending(ForcedJump)`, `ResumeOnly { slot_id,
  resume_ticks }`), and the identity that already ended (`ended`). It is a pure
  state machine: no `Mpv`, no channel, no lock, and it borrows no queue — the
  run hands it plain values in `RunPosition`. `PlaybackRun` holds it as
  `run_lifecycle`.
- The verdict. `stop_for_unrecoverable_error` (`run/events/termination.rs`)
  turns a generic `MpvFailure::Other` into one question,
  `RunLifecycle::load_failed(RunPosition)`, and acts on the answer:
  `FailureVerdict::Continue` falls through so drift follows mpv's own move, and
  `FailureVerdict::EndRun { failed_slot, phase }` ends the run through
  `stop_run_with_error`, whose error text comes from `FailurePhase`
  (`BeforeStart` or `AfterStart`). `run_position()` resolves the armed target's
  ordinal (`queue.slot_index`) and the slot of the entry mpv is on before the
  call, because identity → ordinal is the run's resolution, not the machine's.
  The rule order is design D2: take the failed entry from the arm while one is
  set, clear the arm in every verdict, end before the first restart (#941), end
  in the active-file projection or when nothing follows (#942, #943), otherwise
  continue.
- The decode trap stays in one place. An mpv load error arrives as `end-file`
  with `reason=error`, which `end_file_from_ipc`
  (`crates/mbv-player/src/mpv_event/ipc.rs`) decodes to
  `MpvEvent::Failed(MpvFailure::Other(..))` — **not** `MpvEvent::EndFile`. So
  the end-of-file paths (`on_end_file`, `on_end_file_queue`,
  `stop_at_queue_end`, `on_end_file_standalone`) never see an error, and every
  error exit lives in `on_mpv_error`. The run loop turns the event into one
  lifecycle fact; the machine never sees an `MpvEvent`. Design D2 keeps the
  output-specific cases out of the machine: `on_mpv_error` still classifies
  `MpvFailure::ProcessExited` and `MpvFailure::AudioOutputInit` itself, and only
  `MpvFailure::Other` reaches `load_failed`.
- A jump-target failure reads the target (design D3). While `jump_arm()` is
  `Pending` or `ResumeOnly`, the failed entry is the armed target
  (`jump_target_slot`), and rule 2 of the verdict disarms it. Drift therefore
  stops being muted — `on_playlist_pos_changed` (`run/events/playlist_drift.rs`)
  ignores a position change only while the arm is not `Idle` — and no later
  entry inherits the failed target's resume seek, because `apply_forced_resume`
  takes it through `take_resume_ticks` (`run/events/restart.rs`) and the verdict
  already cleared it. The ordinary superseded-jump case stays with
  `is_superseded_jump_end_file` (`run/events/event_classifiers.rs`).
- The begun boundary belongs to each submission. `playback_started()` records
  the `PlaybackRestart` boundary (`run/events/restart.rs`), and
  `submission_accepted()` resets `began`, `jump`, and `ended` at every point a
  new submission is accepted into the live child: `PlayerCommand::SubmitQueue`
  (`run/commands.rs`), where `run_identity` refreshes, plus
  `replace_with_queue_items` and `cmd_load_new` (`run/commands/load.rs`).
  Design D5.
- The single exit. `PlaybackRun::end_run(StopOutcome)` (`run/state.rs`) calls
  `RunLifecycle::close(self.run_identity)`; only the first close for that
  identity sets `status.active = false` and sends `PlayerEvent::Stopped`, and a
  second one logs `player.run.end_suppressed` and sends nothing. The ten in-run
  end paths build their own outcome and call it: `finalize_quit`
  (`run/run_loop.rs`), `stop_for_missing_completed`, `stop_at_queue_end`, and
  `stop_for_failed_advance` (`run/events/queue_advance.rs`),
  `stop_run_with_error`, `on_end_file_standalone`, `shutdown_standalone`, and
  `shutdown_queue` (`run/events/termination.rs`), `accept_stopped_replacement`
  (`run/commands/load.rs`), and `stop_for_active_file_failure`
  (`run/commands/queue.rs`). No flag guards the send: the old
  `stopped_event_sent` field is gone, and `stop_run_with_error` does not read
  `stop_report_sent()` (`run/state.rs`), which tracks the Service report and not
  the run end.
- A failed active-file selection or install ends the run.
  `cmd_jump_to_active_file` (selection error) and `promote_to_active_file`
  (install error) both go through `stop_for_active_file_failure`, which reports
  one `Stopped` naming the slot the run tried to select and parks the run:
  `begin_item_lifecycle` arms the replacement drain, so the teardown `stop`
  command's own `end-file` is swallowed by `swallow_pending_load` instead of
  advancing or ending the parked run again. The mpv child stays alive for a
  later submission, as in `accept_stopped_replacement`.
- Sends outside a live run stay outside the machine: `submit.rs` (before the
  run exists) and the panic teardown (`run/run_loop.rs`) keep their own sends.
- Tests own the contracts, one per layer.
  `run/lifecycle/tests.rs` owns the machine:
  `close_reports_once_per_run_identity_and_reopens_on_a_new_submission` for the
  exit, and the named `#[case]` table `load_failed_judges_the_failure_verdict`
  for the verdict, whose regression cases cite #941, #942, #943 and design
  defects 4, 5, and 6. `crates/mbv-player/src/tests/session.rs` owns the wiring
  at run level: `quit_after_a_failed_active_file_replacement_sends_no_second_stopped`
  and `process_exit_after_a_reported_stop_sends_no_second_stopped`.
  `crates/mbv-player/src/tests/active_file.rs` owns the active-file arms:
  `failed_active_file_jump_reports_one_stopped_naming_the_target_slot` and
  `a_failed_active_file_jump_drains_its_teardown_end_file_and_parks_the_run`.

## Where it currently fails

The old residual is closed: a jump whose target failed to load no longer
leaves a jump armed, because the verdict both reads the arm for the failed
entry and disarms it in every answer. Two known limits remain, neither
observed:

1. The design D3 two-jump window. Two quick jumps, A then B, leave B armed,
   because the arm holds one jump and `arm_jump` overwrites it. If A's load
   fails before mpv applies B's `playlist-pos` write, the verdict blames the
   armed target: the stop names B's slot, and when B is the last entry the run
   ends although B never failed to load. The window is narrow — mpv normally
   stops A with `reason=stop` first, and `is_superseded_jump_end_file` handles
   that end-file — and the stop names a queued slot the user can replay. The
   upgrade path is the alternative design D3 rejected: decode
   `playlist_entry_id` on `start-file` and `end-file` and keep an
   entry-id-to-slot map. Take it up if a failure is ever seen with no jump
   armed.
2. A `JumpTo` into a parked run plays under the identity that already ended.
   `end_run` leaves a parked run alive on purpose, so a later submission can
   reuse the mpv child, but a jump is not a submission: it neither refreshes
   `run_identity` nor calls `submission_accepted()`. The entry it plays is
   therefore covered by the closed identity, and when that playback reaches its
   own end path `close` answers `false`, so no `Stopped` goes out and the Owner
   keeps the Bound queue and never retires the occurrence. In the active-file
   projection the panel also stays inactive: `arm_jump` discards the arm there,
   so `settle_idle_jump_on_restart` — the restart site that sets
   `status.active = true` — settles nothing, and no other active-file jump site
   reactivates the status. The fix would be to treat a `JumpTo` into a parked
   run as a new submission. That is a product call, and it is not part of this
   change.
