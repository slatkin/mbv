# Proposal

## Why

The `playback-resume` spec says each medium has one source of resume progress.
The code spreads that rule over many sites in `mbv-player`: `refresh_emby_resume`
has four call sites (thread start, queue load, append, jump), Audiobookshelf
takes its position from the opened session in `sources.rs` while
`override_jump_resume` skips it by kind, `resume_start_pos` reads the canonical
item, and `apply_forced_resume` seeks on a re-visit (Invariant 6). A new
`QueueItem` kind must repeat the source choice at each site. Issue #963 asks
for one per-kind decision.

Code reading also shows a likely stale-read race, on the cold-start path.
Within one run, stop reports are synchronous (`advance_to_next_track`,
`report_jumped_item`), so a jump back is ordered. At run exit the stop report
rides the run's detached FIFO report worker (`ReportJob::ProgressJoinThenStopped`).
`Player::join` joins the player thread but not that worker. The next submission
cold-starts and `load_queue_sources` fetches the position while the stop report
can still be queued. Stopping an item and replaying it right away can read the
server position from before the stop report landed. A finished item is the worst
case: the stale read gives a position near the end instead of the reset position.

## What Changes

- Add one resume seam in `mbv-player`: a per-kind `ResumeSource` decision with
  an explicit arm for each `QueueItem` kind (no wildcard), and one interface
  for the resume position of a slot now. The Emby fetch, retry, and one-percent
  rule sit behind it.
- Route the four `refresh_emby_resume` call sites through the seam.
  `cmd_jump_to` and `PreparedSource::override_jump_resume` stop branching on
  kind. They ask the seam.
- Order the cold-start fetch after the previous run's stop report: a run drains
  its report worker, for a bounded time, before its thread ends. The existing
  `stop(); join()` in `submit_queue_slots` then covers the next fetch. The seam
  needs no ordering hook.
- Update Invariant 6 to name the seam as the one site that chooses the source,
  and to record the ordering rule.
- Out of scope: the Settle half of #950 (stop, complete, ack, mark, Feed
  persistence), the live progress tick (needs #950 candidate 1), and the Cast
  and Emby Session output start positions in `mbv-daemon`
  (`resume_ticks_for_slot`). Those keep reading the canonical item.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `playback-resume`: adds a requirement that a stopped Emby video's report is
  sent, with a bounded wait, before a later start fetches an Emby position.

## Impact

- `crates/mbv-player/src/lib.rs`: `resume_start_pos`, `resume_ticks_for_item`,
  `refresh_emby_resume`, and `EMBY_RESUME_RETRY_DELAYS` move into a new
  `resume.rs` module. `resume_ticks_for_item` and `resume_ticks_for_slot` stay
  public (the daemon calls them).
- `crates/mbv-player/src/sources.rs`, `submit.rs`, `run/commands/load.rs`,
  `run/commands/queue.rs`, `run/queue.rs`: call sites move to the seam.
- `crates/mbv-player/src/report_worker.rs`: a barrier job on the FIFO worker.
- `crates/mbv-player/src/run/run_loop.rs`: the run drains its worker at exit.
- `crates/mbv-player/src/tests/resume_refresh.rs`: becomes the seam test file.
- `docs/invariants/06-queue-progress-application-sites.md`: updated.
- No ctrl protocol change. `PlayerCommand::JumpTo { resume_ticks }` stays.
- No new term for `CONTEXT.md` beyond *Resume source* (see design).
