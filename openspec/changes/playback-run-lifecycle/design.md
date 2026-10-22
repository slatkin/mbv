# Design

## Context

See `proposal.md` for the motivation. The facts below come from a read-only recon of `crates/mbv-player/src/` (2026-10-21) and from mpv `master` source (`loadfile.c`, `command.c`).

**mpv event order for a jump.** A `playlist-pos` write sends `end-file reason=stop` for the outgoing entry, then `start-file` for the target, then `end-file reason=error` with `file_error` for the target, then mpv moves to the next entry by itself (`mp_next_file(+1)`). The decoder (`mpv_event/ipc.rs:100-120`) maps `end-file reason=error` to `MpvEvent::Failed(MpvFailure::Other(text))`. It drops `start-file` (`Ignored`) and drops `playlist_entry_id`.

**Where the lifecycle state lives today.** All of it is in fields of `PlaybackRun` (`run/types.rs`):

| Field | Written | Read |
|---|---|---|
| `playback_began` | `restart.rs:128`, `queue.rs:576` (init only) | `termination.rs:78,81` |
| `active_file` | `queue.rs:540`, `load.rs:296,368`, `commands/queue.rs:187` | 13 sites in `run/` |
| `forced_jump` | `commands/queue.rs:61,118,125`, `queue_advance.rs:130,141`, `restart.rs:12,20,81`, `queue.rs:283`, `queue_commands.rs:25`, `state.rs:93` | `queue_advance.rs:104,117`, `restart.rs:11,75`, `event_classifiers.rs:22`, `playlist_drift.rs:11`, `commands/queue.rs:117` |
| `stopped_event_sent` | `termination.rs:117,188`, `state.rs:95` | `termination.rs:108,179,233` |

**`Stopped` sites inside a run.** There are nine: `run_loop.rs:216` (`finalize_quit`), `queue_advance.rs:295,355,374`, `termination.rs:109,180,234,258`, and `commands/load.rs:397`. Four sit outside a live run and keep their own sends: `submit.rs:311,421,449` (before the run exists) and `run_loop.rs:120` (panic teardown).

**Defects the recon found** (each one is a spec scenario):

1. `stop_run_with_error` guards on `!stop_report_sent()`, which is the Emby report state. `report_stopped_and_adopt_mpv_entry` (`queue.rs:237`) sets it, so a fatal error after an adoption sends no `Stopped`.
2. `accept_stopped_replacement` (`load.rs:397`) sends `Stopped` and keeps the run alive without a flag. A later quit then sends a second `Stopped` through `finalize_quit` or `shutdown_queue`, and neither has a guard.
3. The `cmd_jump_to_active_file` error arm (`commands/queue.rs:81`) and the `promote_to_active_file` install failure (`commands/queue.rs:182`) send nothing. `install_active_projection` has already set `status.active = false` there.
4. A from-idle jump (`commands/queue.rs:113-123`) does not move `current_idx`. A target failure before the restart then reads the successor of the stale index (the Invariant 24 residual). This path runs only when `status.active` is false in a live playlist run. Recon did not prove that this state is reachable. The fix is cheap, and the lifecycle closes the case anyway.
5. After `settle_forced_jump` keeps a resume-only arm (`queue_advance.rs:141`), a failure of target `j` leaves the arm set. Drift stays muted (`playlist_drift.rs:11`), and the restart of `j+1` seeks to the resume position of `j` (`restart.rs:74-92`).
6. `playback_began` never resets. In a reused run, the #941 boundary is wrong for a new submission.

**Owner side.** `mbv-daemon` drops a `Stopped` whose `run_identity` is not current (`run.rs:111-138`). It does not remove duplicates, and the TUI does not either. A reused run gets a new `run_identity` on each non-empty `SubmitQueue` (`run/commands.rs:88`).

## Goals / Non-Goals

**Goals:**

- One module owns run-lifecycle state as a pure state machine with no `Mpv`, channel, or lock. Hermetic tests drive it with facts.
- One exit that sends `Stopped`, guarded per run identity.
- Fix defects 1-6.

**Non-Goals:**

- Do not change the `MpvEvent` decoder, `PlayerEvent`, ctrl, the Owner, or the TUI.
- The Emby stop-report state (`StopReport`, `LoadState`, `ItemLifecycleState` in `state.rs`) stays where it is. It tracks the Service report, not the run end. Defect 1 is fixed by not reading it as the event guard.
- The quit timer (`quit_at`), `stop_slot`, and `stop_runtime` stay in `PlaybackRun`. They feed the stop outcome, and the exit only guards its send.
- Sends from before the run and from panic teardown stay outside the module.
- Do not decode `playlist_entry_id` (see D3).
- Do not change the Owner playback-output seam (Candidate 1).

## Decisions

### D1. `RunLifecycle`: a pure state machine in `run/lifecycle.rs`

The new `crates/mbv-player/src/run/lifecycle.rs` holds `RunLifecycle`, and `PlaybackRun` holds one in place of `playback_began`, `active_file`, `forced_jump`, and `stopped_event_sent`. Its state:

- `began: bool`: whether playback began in the current submission.
- `projection: Projection { Playlist, ActiveFile }`: replaces `active_file`. Callers ask `is_active_file()`. Only `enter_active_file()` writes it, and the projection never goes back to `Playlist`, as today.
- `jump: JumpArm`: replaces `Option<ForcedJump>`. The variants are `Idle`, `Pending(ForcedJump)` (waiting for its end-file or idle restart), and `ResumeOnly { slot_id, resume_ticks }` (settled, waiting for the target to restart). Today one `Option` holds both meanings, and the reader infers which from `transition` and `from_idle`. That inference caused defect 5. With named states, each event has one transition.
- `ended: Option<PlaybackGeneration>`: the run identity that already sent its `Stopped`.

The facts it consumes are `submission_accepted()`, `enter_active_file()`, `arm_jump(ForcedJump)`, `playback_started()`, `jump_settled_on_end_file(...)`, `entry_adopted()`, `load_failed(RunPosition) -> FailureVerdict`, and `close(identity) -> bool`. `RunPosition` holds plain values (`current_idx`, `queue_len`, the slot at an index), so the machine does not borrow the queue.

*Alternative considered:* move the queue and the reporter into the module too. That makes the module the whole run again, and it would not be pure. Rejected.

### D2. The verdict on a load failure

`load_failed` returns `FailureVerdict::Continue` or `FailureVerdict::EndRun { failed_slot, phase }`. Here `phase` is `BeforeStart` or `AfterStart`, and it picks the error text that `termination.rs` already uses. The order of the rules:

1. Find the failed entry. If `jump` is `Pending` or `ResumeOnly`, the failed entry is the slot of the armed target. Otherwise it is `current_idx`.
2. Clear `jump` to `Idle` in all cases. A failed target leaves no jump in flight (defects 4 and 5).
3. If `!began`, return `EndRun(BeforeStart)`.
4. If the projection is `ActiveFile`, or no entry follows the failed entry, return `EndRun(AfterStart)`.
5. Otherwise return `Continue`. mpv moves to the next entry by itself, and drift adopts it now that no arm mutes it.

`on_mpv_error` keeps its classification of `ProcessExited` and `AudioOutputInit` as it is. Only `MpvFailure::Other` reaches `load_failed`. That is the "decode traps behind the interface" step: the run loop translates `MpvEvent` into a fact once, and the lifecycle never sees `MpvEvent`.

### D3. Find the failed entry from the armed jump, not from `playlist_entry_id`

From mpv's event order, an `end-file error` that arrives while a jump is armed belongs to the jump target, because the outgoing entry's `end-file stop` settled or was swallowed first. So the armed target is enough.

*Alternative considered:* decode `playlist_entry_id` on `start-file` and `end-file`, and keep an entry-id-to-slot map. It is exact, but it adds decoder changes, a new map that must stay in step with every playlist rewrite, and fixture churn, all for a case that the arm already settles. Rejected for now. It is the upgrade path if a failure is ever seen without an arm.

### D4. The one exit: `PlaybackRun::end_run(StopOutcome)`

Every in-run send goes through one method: `end_run(StopOutcome { slot_id, position_ticks, played, consume, error })`. The method calls `lifecycle.close(self.run_identity)`. Only when that returns `true` does it set `status.active = false` and send `PlayerEvent::Stopped`. The nine call sites keep their own outcome math (`advance_decision`, `quit_timeout_stop_flags`, near-end) and build a `StopOutcome`. `stopped_event_sent` and the `stop_report_sent()` guard go away (defects 1 and 2). `submission_accepted()` clears `ended`, because a new submission carries a new identity.

The three active-file failure arms (defect 3) call `end_run` with the error and the slot that the run tried to select. Like `accept_stopped_replacement`, the run stays alive and parked, so a later submission can reuse the mpv child.

*Alternative considered:* remove duplicates in the Owner by `run_identity`. The duty belongs to the Player, and `AGENTS.md` keeps the Owner authoritative only over queue state. Rejected.

### D5. `submission_accepted` resets `began`

`PlayerCommand::SubmitQueue` with items refreshes `run_identity` and calls `submission_accepted()`. The active-file replacement path is reached through that same command, so it inherits the refreshed identity and calls `submission_accepted()` again. That resets `began`, `jump`, and `ended` (defect 6). `LoadNew` does not get a new identity today: it keeps the current one and calls `submission_accepted()` too, so a new file that fails before its first restart is a start failure. Its single-entry queue already ends the run, so only the error text changes.

### D6. Tests

Tests follow `docs/invariants/14-test-ownership.md`. Each contract has one owner:

- **The lifecycle verdict** (D2, D5) is owned by `run/lifecycle/tests.rs`. These are private-item unit tests: a named `#[case]` table of facts in and verdict out, where each case expects a different outcome. One table holds the rules, and a regression case per defect cites it (#941, #942, #943, defects 4 and 5).
- **The single exit** (D4) is owned by one unit test on `close` (second close → false, close after `submission_accepted` → true), and one run-level test in `src/tests/session.rs` that drives a failed replacement and then a quit, and counts `Stopped` on the channel (defect 2).
- **The active-file failure arms** (defect 3) get one run-level test of the jump error arm, which asserts one `Stopped` with an error.
- The existing tests that set fields directly (`tests/submit.rs:412-476`, `tests/session.rs:24-140,217`, `tests/session_feed.rs:144`) move to the lifecycle facts. Their `playback_began` and `active_file` cases fold into the verdict table and are deleted where the table covers them.

## Risks / Trade-offs

- [Two quick jumps to A then B arm B. If A fails to load before mpv applies B's write, D2 blames B. When B is the last entry, the run then ends while mpv still loads B] → This window is narrow. mpv usually stops A with `reason=stop`, and the superseded-jump guard already handles that. The `playlist_entry_id` path of D3 closes the window if it is ever seen. Until then, the stop names B and the user can play again.
- [A start failure in a new submission now ends the run where it used to skip ahead (D5)] → This is the #941 rule applied to every submission. It is a visible change, and the spec names it.
- [`end_run` keeps the run alive in the parked cases, so a parked run with `status.active = false` keeps waiting for commands] → This is the same as `accept_stopped_replacement` today. Shutdown and quit still reach `end_run`, which now sends nothing for the closed identity.
- [The refactor touches 9 send sites and about 30 flag sites in one crate] → The tasks move state first, with no behavior change and the tests green, and then fix one defect per task.

## Migration Plan

Internal to `mbv-player`. No wire, config, or persistence change. Rollback is a revert.
