# Design

## Context

See `proposal.md` for the motivation. Current state in `crates/mbv-player`:

- `lib.rs` holds `resume_start_pos` (per-kind gate on the canonical item),
  `resume_ticks_for_item`/`resume_ticks_for_slot` (public, the daemon calls
  them), and `refresh_emby_resume` (batched Emby fetch, three attempts, then
  position 0). Its four callers are `load_queue_sources` (`submit.rs`, thread
  start), `cmd_submit_queue` (`run/commands/load.rs`), `append_items_via_mpv`
  and `cmd_jump_to` (`run/commands/queue.rs`).
- `cmd_jump_to` branches on kind by hand: an Emby video fetches, every other
  kind keeps the owner's `resume_ticks`. `PreparedSource::override_jump_resume`
  (`sources.rs`) branches on kind again. `Session` (Audiobookshelf) keeps the
  session start. Every other kind takes `resume_ticks.map_or(0.0, ticks_to_seconds)`,
  so `select_active_slot` (which passes `None`) zeroes the start.
- Audiobookshelf start comes from the opened session
  (`prepare_episode_source`, `prepare_book_source`), not from the seam path.
  `arm_jump` returns `Idle` for an active-file queue, and every Audiobookshelf
  queue is active-file, so an Audiobookshelf jump arms no resume seek.
- Stop reports: within one run they are synchronous (`advance_to_next_track`,
  `report_jumped_item` call `reporter.report_stopped`). The helpers
  `report_stopped_background` and `transition_to_deferred` are reachable only
  from `cmd_load_new`, which no code path constructs (`PlayerCommand::LoadNew`
  has no wire form). The one stop that rides the worker is the run-exit stop,
  `report_stop_now_or_background` -> `ReportJob::ProgressJoinThenStopped`.
- The report worker is a detached FIFO thread made in `SessionReporter::new`.
  `Player::join` joins the player thread only. `submit_queue_slots` cold-starts
  with `self.stop(); self.join();`, and the new thread's `load_queue_sources`
  fetches before `make_reporter`. The previous run's worker can still hold the
  stop report then. This is the stale-read path.

## Goals / Non-Goals

**Goals:**

- One function chooses the resume source for a `QueueItem` kind, with one
  explicit arm per kind and no wildcard.
- The four fetch sites and the jump-time override ask one seam.
- A cold-start fetch reads a position that includes the previous run's stop
  report, within a bound.
- The source choice per kind is testable at the seam with the fetch mocked.

**Non-Goals:**

- Moving Settle (stop, complete, ack, mark, Feed persistence) to the Owner.
- The live progress tick.
- Changing the position the Cast and Emby Session outputs start from
  (`resume_ticks_for_slot` in `mbv-daemon`). They read the canonical item
  today. That is a separate decision.
- Changing `PlayerCommand::JumpTo { resume_ticks }`. The owner keeps sending
  its ticks. The seam decides whether the Player uses them.
- Applying the one-percent rule to the Audiobookshelf session position. The
  code does not apply it today. See Open Questions.

## Decisions

### D1: New `resume.rs` module owns the decision

Create `crates/mbv-player/src/resume.rs` (a module file, no `mod.rs`). Move
`resume_start_pos`, `resume_ticks_for_item`, `resume_ticks_for_slot`,
`refresh_emby_resume`, and `EMBY_RESUME_RETRY_DELAYS` into it. `lib.rs`
re-exports the two public functions so `mbv-daemon` is unchanged.

Alternative: keep the code in `lib.rs` and add a function. Rejected: `lib.rs`
is already the crate grab-bag, and the seam needs its own tests and docs.

### D2: `ResumeSource` is the per-kind decision

```rust
pub(crate) enum ResumeSource {
    /// Emby video: the position Emby reports when the start is decided.
    Server,
    /// Audiobookshelf: the position of the opened playback session.
    Session,
    /// Feed: the locally recorded position.
    Local,
    /// Emby audio: always the beginning.
    Beginning,
}

pub(crate) fn resume_source(item: &QueueItem) -> ResumeSource
```

`resume_source` matches every `QueueItem` variant explicitly. A new kind fails
to compile until it adds its arm. Every other resume decision matches on
`ResumeSource`, never on `QueueItem`. Matching on `ResumeSource` is also
exhaustive.

Alternative: a `QueueItem::resume_source` method in `mbv-queue`. Rejected: the
source depends on player concerns (server fetch, session), and `mbv-queue`
holds no player policy.

### D3: One interface, `ResumeSeam`

```rust
pub(crate) struct ResumeSeam<'a> {
    fetch: &'a mut dyn FnMut(&[String]) -> Result<Vec<EmbyItem>, EmbyError>,
    delays: &'a [Duration],
}

impl ResumeSeam<'_> {
    /// Items whose recorded progress is current: `Server` items are
    /// refreshed in one batch; every other arm passes through.
    pub(crate) fn refresh(&mut self, items: Vec<QueueItem>) -> Vec<QueueItem>;

    /// The re-visit seek for a jump target: ticks, or `None` for a start from
    /// the beginning or when the position is already baked into the load.
    pub(crate) fn jump_ticks(&mut self, item: &QueueItem, owner_ticks: Option<i64>) -> Option<i64>;
}
```

- `refresh` is today's `refresh_emby_resume` body, keyed on `ResumeSource::Server`.
  Thread start, queue load, and append call it.
- `jump_ticks` replaces the hand-written branch in `cmd_jump_to`. `Server`
  fetches one item through `refresh` and returns `resume_ticks_for_item`.
  `Local` returns `owner_ticks`. `Session` returns `None`, because the session
  position is already baked into the prepared source. `Beginning` returns
  `None`.
- `PreparedSource::override_jump_resume` keeps its apply-or-zero semantics and
  matches on `resume_source` instead of `is_audiobookshelf`: `Session` is the
  pass-through arm, every other arm applies `resume_ticks` or zero.
  `jump_ticks` is for `cmd_jump_to` only. `select_active_slot` and the other
  prepare sites never fetch.
- The retry schedule, the 1% rule, and the "missing id counts as failed" rule
  stay exactly as the `playback-resume` spec states them.

The fetch stays a closure so tests mock it. This matches the existing
`resume_refresh.rs` tests, which move to the seam unchanged in contract.

Alternative: a `ResumeProvider` trait object per kind. Rejected: four arms
with no shared state do not need dynamic dispatch, and the issue asks for
explicit arms.

### D4: A run drains its report worker before its thread ends

`ReportJob` gains `Barrier(mpsc::Sender<()>)`. `SessionReporter::settle(bound)`
sends it and waits on `recv_timeout(bound)`. The worker is FIFO, so the ack
means every report queued before the barrier was sent. `run()` in
`run/run_loop.rs` calls `self.reporter.settle(REPORT_SETTLE_BOUND)` after the
loop returns and the mpv child is dropped. `Player::join` then returns only
after the run's stop report was sent, or the bound passed. The existing
`stop(); join()` in `submit_queue_slots` orders the next cold-start fetch with
no new handle across runs, and the seam carries no hook.

On timeout `settle` logs `player.resume.settle_timed_out` itself and returns.
The fetch then runs anyway, as the existing fetch rule requires: no failure and
no user wait. The bound is a named constant, 2 seconds. Tests inject zero.

Same-run fetches (jump, live-run queue load, append) need no barrier: their
stop reports are synchronous.

Alternatives:

- Carry the last run's `job_tx` in `Player` and barrier it in
  `load_queue_sources`. Rejected: new cross-run state, and the drain at exit
  gives the same order through `join`.
- Scope the requirement to the current run's reporter and record the cross-run
  gap. Rejected: the cold start is the dominant path for the stale read.
- A hook on the seam. Rejected: the seam fetches on the new run's thread, which
  has no reporter yet, so only the old run can order it.
- A pending-report counter. Rejected: a barrier needs no shared counter and no
  decrement discipline.

### D5: What is verified, and how

Verified by reading the code (the plan review cites each site): the stop paths
above, the detached worker, and that `join` does not cover it. A hermetic test
cannot reproduce the stale read: it needs a real run with mpv. The worker-level
test proves the barrier contract only. Whether a real Emby server serves a stale
position after a quick replay needs a live test, which the user runs. No live
probe of the user's Emby server (project rules).

Out of this change: `cmd_load_new` and the background helpers it alone uses are
dead. They are named here and left in place.

Known same-run gap, not fixed here: a live-run queue load that replaces the
active item with the same Emby video fetches before the old item's stop report
(`cmd_submit_queue`), so it reads the last progress report. That is a different
race (progress interval, not the worker) and needs its own decision.

### D6: Invariant 6 and CONTEXT.md

Update `docs/invariants/06-queue-progress-application-sites.md`: the seek
target is chosen by `resume_source`, the list of kinds moves to the seam, and
the run-exit drain is stated as the ordering rule, with its test name. Add a *Resume source* term to
`CONTEXT.md` with *Avoid*: `progress source`, `position source`. The
*Playback resume* term (the 1% rule) stays as is.

## Risks / Trade-offs

- [The drain delays run-thread exit when the worker is busy] → The wait is
  bounded at 2 seconds. A busy worker is rare: it holds small HTTP reports. The
  stop job joins a progress thread with a 30 second budget
  (`ProgressJoinThenStopped`). If that join blocks the worker, the barrier times
  out and the cold start proceeds. Task 3.3 checks every `Player::join` and
  `join_or_timeout` caller: the daemon callers (`reconciliation.rs`,
  `control_events.rs`) already bound the join with a deadline.
- [`Session` returning `None` changes behavior] → No. `arm_jump` already
  returns `Idle` for active-file queues, so the arm is a no-op change. A pinning
  test (task 1.1) records it.
- [A moved function breaks daemon imports] → `lib.rs` re-exports
  `resume_ticks_for_item` and `resume_ticks_for_slot`. `mbv-daemon` calls
  `resume_ticks_for_slot` at five sites. `cargo check -p mbv-daemon` is a task.
- [The seam grows into a second queue authority] → It reads items and returns
  ticks. It never writes the canonical queue. The Owner stays authoritative
  (AGENTS.md, Playback and queue authority).
- [The run-exit drain has no hermetic test of its wiring] → The barrier
  contract is tested at the worker. The wiring is one call in `run()`. A live
  test by the user covers the replay case.

## Migration Plan

Internal refactor plus one ordering step. No data, config, or protocol
migration. Roll back by reverting the PR. Archive the change and sync the
`playback-resume` delta only when the user approves the PR for merge.

## Open Questions

- The `playback-resume` spec says the one-percent rule applies to the position
  from each source. The Audiobookshelf path uses `session.current_time_seconds`
  with no gate. This change keeps that behavior. Whether the spec or the code
  should change is a separate decision, and the answer does not change this
  seam's shape (the `Session` arm would apply the rule in one place).
