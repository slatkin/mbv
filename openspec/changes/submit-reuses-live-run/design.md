# Design

## Context

The Owner-side submission entry point is `Controller::submit_queue_slots` (`crates/mbv-player/src/submit.rs`). Its current reuse decision:

- Fast path, taken when `status.active` is true and the submitted queue's headless class equals `current_is_headless`: seed the status from the start item and send `PlayerCommand::SubmitQueue` into the live run.
- Otherwise: `stop()`, `join()`, then spawn a fresh player thread (consuming the pre-warmed mpv child when one exists).

Facts that constrain the design:

- `thread_handle` is cleared only by `join()`/`join_or_timeout()`. A run that ended on its own keeps the handle set, so the handle is not a liveness signal.
- `cmd_tx` stays set after `stop()` on purpose (a `LoadNew` sent after a stop must reach the thread so it can cancel the quit). Once the run thread has exited, the receiver is dropped and a send fails. So the send itself is the only reliable "this run can take a command" test.
- Both arms of the run-side `cmd_submit_queue` begin with `cancel_pending_quit()`, so a submission that reaches a stopping run cancels the stop by existing mechanism.
- A parked run (`accept_stopped_replacement`, `stop_for_active_file_failure`) keeps its mpv child alive for a later submission; invariant 24 documents this and notes no consumer exists today, because `status.active = false` there routes submissions to the cold path.
- The daemon computes the replacement's headless class in `submit_queue_slots_cold_start` (`crates/mbv-daemon/src/core.rs`) via `Controller::headless_for(&client, all_audio)` and calls the same submission entry point; the helper's name claims "cold start" but the function inherits whatever the entry point decides.

## Goals / Non-Goals

Goals:

- A queue replacement never tears down a live, headless-compatible run because of a status-flag read.
- The reuse decision is observable in the log with its reason.

Non-Goals:

- No change to the Client-side dispatch (`play_queue_cursor_locally`): jump-vs-replace stays as built.
- No change to the headless class definition or `headless_for`.
- No new ctrl-protocol vocabulary; no wire changes.
- No rework of the run-side failure stops (`stop_for_missing_completed` and friends); the parked-run reuse merely gives `accept_stopped_replacement`'s parked child its consumer.

## Decisions

- **D1 — Try the live run, fall back on a failed send.** The reuse attempt sends `SubmitQueue` into the live run when `current_is_headless` matches; a failed send (thread gone, channel disconnected) falls through to the existing cold start. The cold start re-seeds the status and mints a fresh generation, so the first attempt's seeding and generation advance are harmless. Alternative considered: pre-check liveness via `thread_handle` — rejected, because the handle stays set after a natural run end and a pre-check still races the send.

- **D2 — Status seeding moves after the send succeeds, not before.** Today the fast path seeds the status before sending; with a fall-back path this would briefly publish a seeded status for a submission that then cold-starts. Seed after the send is accepted. The status stays a projection; a cold start re-seeds it from the start item as it does today.

- **D3 — A submission into a not-yet-applied stop cancels the stop.** No new mechanism: both `cmd_submit_queue` arms already begin with `cancel_pending_quit()`, and the run loop drains commands before it applies a pending stop. The behavioural change is that submissions now reach a stopping run instead of cold-starting beside it. The guarantee is bounded: once `handle_stop_request` has told mpv to quit, the child dies regardless, the send-failure or process-exit path cold-starts, and the replacement plays exactly once in a new run. The observable contract (play exactly once, same run when possible) is what the spec delta states. Accepted product call from planning (2026-10-27): Stop followed by an enter press keeps the window and plays the selection whenever the stop has not yet been applied. If this ever proves unwanted, the alternative needs stop provenance on the wire, which is out of scope.

- **D4 — The headless-class guard stays.** A replacement whose `headless_for` result differs from `current_is_headless` still cold-starts, because the child's output configuration must change (headless audio runs with `show_audio_window = false` must not hold a video-window child). This is the one legitimate teardown reason while a run exists.

- **D5 — Name the decision.** Two structured events at the decision point, following the existing `name:` convention: reuse emits one event naming the reused run's identity; a cold start emits one with a `reason` field (`no_run`, `headless_mismatch`, or `send_failed`). The events live in `submit.rs` so every caller (Emby play, play queue, daemon replace) reports the same vocabulary.

- **D6 — Rename the daemon helper.** `submit_queue_slots_cold_start` becomes `submit_queue_slots_for_replace` (it is the ctrl-replace entry, and the reuse/cold decision belongs to the Controller, not the name). Call-site-only rename in `crates/mbv-daemon`.

- **D7 — Invariant 22 boundary.** The same run takes the new submission and refreshes its run identity via the existing `submission_accepted()` path; no mpv child ever serves two runs, and the child never outlives the Owner. The change consumes invariant 24's parked-run provision rather than altering either invariant.

## Risks / Trade-offs

- [The send succeeds but the run exits before processing the command] → The replacement would be lost while the Owner has already installed the queue. Mitigation: verified during planning — the run loop drains commands before applying a pending stop, so a submission that arrives before `handle_stop_request` runs cancels the stop; after mpv's `quit` is applied the child dies and the Owner's stop-observation path ends the run, after which a retry cold-starts. The residual window (submit lands between quit and run exit) is the same one `LoadNew` lives with today; record the finding in the invariant notes if it ever needs a wider fix.
- [A submission revives a parked run whose child is in an unknown state] → The run-side install failure path ends the run with exactly one stop naming the slot (existing spec requirement), so the worst case is the behaviour users see today on a failed load, without the extra teardown.
- [Status briefly disagrees with the run during the send-fallback window] → The cold start re-seeds everything it publishes; the gap is microseconds inside one mutex scope.

## Migration Plan

Single binary, no persistence or protocol change: ship and revert like any code change. The behaviour change users can notice (Stop-then-enter keeps the window) is listed in the proposal.

## Open Questions

None.
