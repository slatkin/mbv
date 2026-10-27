# Design

## Context

The Owner-side submission entry point is `Controller::submit_queue_slots` (`crates/mbv-player/src/submit.rs`). Its current reuse decision:

- Fast path, taken when `status.active` is true and the submitted queue's headless class equals `current_is_headless`: seed the status from the start item and send `PlayerCommand::SubmitQueue` into the live run.
- Otherwise: `stop()`, `join()`, then spawn a fresh player thread (consuming the pre-warmed mpv child when one exists).

Facts that constrain the design:

- `thread_handle` is cleared only by `join()`/`join_or_timeout()`. A run that ended on its own keeps the handle set, so the handle is not a liveness signal.
- `cmd_tx` stays set after `stop()` on purpose. Once the run thread has exited, the receiver is dropped and a send fails. A send can still succeed while the run thread is tearing down: `end_run` sets `active=false`, the loop returns, `run()` drops the mpv child, and `cmd_rx` drops only when `run_player_thread` returns.
- `Player::stop()` takes `stop_tx` (`controller.rs`); only a cold start installs a new one (`submit.rs`). The queued `()` stays in the run's `stop_rx`.
- The run loop does not cancel a requested stop. `cancel_stop` from a drained `SubmitQueue` only skips the `stop_rx` read of that one tick (`run/run_loop.rs`, `run_tick`); the next tick reads the queued `()` and applies the stop, which quits mpv and kills the replacement. `cancel_pending_quit()` clears `quit_at` and the stop context, not the queued `()`. So a stop that was requested is not reversible by a submission, and a reused run would have no `stop_tx` left for a later `stop()`.
- A parked run (`accept_stopped_replacement`, `stop_for_active_file_failure`) keeps its mpv child alive for a later submission; invariant 24 documents this and notes no consumer exists today, because `status.active = false` there routes submissions to the cold path.
- A Standalone run idles after its file ends (`run/events/termination.rs`): `status.active = false`, mpv stays up. This is the second live-but-inactive case.
- The daemon computes the replacement's headless class in `submit_queue_slots_cold_start` (`crates/mbv-daemon/src/core.rs`) via `Controller::headless_for(&client, all_audio)` and calls the same submission entry point.
- `current_is_headless` starts `false` (`controller.rs`) and is written only by a cold start. Before any run exists it carries no information.

## Goals / Non-Goals

Goals:

- A queue replacement never tears down a live, headless-compatible run that no stop was requested for, because of a status-flag read.
- The reuse decision is observable in the log with its reason.

Non-Goals:

- No change to the Client-side dispatch (`play_queue_cursor_locally`): jump-vs-replace stays as built.
- No change to the headless class definition or `headless_for`.
- No new ctrl-protocol vocabulary; no wire changes.
- No change to the run's stop handling (`stop_rx`, `cancel_pending_quit`, `LoadNew`), and no rework of the run-side failure stops.
- No "Stop then Enter keeps the window" behaviour: a requested stop always cold-starts (D3).

## Decisions

- **D1 — Decision order, then send, fall back on a failed send.** Test in this order, each failure being a cold start with its reason:
  1. a run exists (`cmd_tx` is `Some`), else `no_run`;
  2. `current_is_headless == headless`, else `headless_mismatch`;
  3. no stop was requested (`stop_tx` is `Some`), else `stop_requested`;
  4. `run_exited` is not set, else `run_exited`;
  5. the send of `PlayerCommand::SubmitQueue` succeeds, else `send_failed`.

  Checks 3 to 5 run with the `stop_tx` lock held and the `cmd_tx` lock taken inside it (`stop()` takes `stop_tx` and then `wakeup_fd`; `send_command` takes `cmd_tx` and then `wakeup_fd`; no code takes `cmd_tx` and then `stop_tx`). A `stop()` therefore cannot slip between the check and the send. The generation advances once at the top of `submit_queue_slots` (`advance_sequence_generation`); the cold start reuses that `run_identity`, so a failed first attempt leaves nothing to undo. Alternative considered: pre-check liveness via `thread_handle` — rejected, because the handle stays set after a natural run end.

- **D2 — Status seeding moves after the send succeeds, not before.** Today the fast path seeds the status before sending; with a fall-back path this would briefly publish a seeded status for a submission that then cold-starts. Seed after the send is accepted. The status stays a projection; a cold start re-seeds it from the start item as it does today.

- **D3 — A requested stop is never reused.** Once `Player::stop()` ran, `stop_tx` is `None` (D1 check 3) and the replacement cold-starts: `stop()` is a no-op, `join()` waits for the run to end, and a new run plays the replacement once. This replaces the earlier plan that a submission cancels a pending stop: it does not (see Context), and a reused run would also have no `stop_tx`. The product call "Stop then Enter keeps the window" (2026-10-27) is withdrawn; it held only for about 250 ms (the quit timeout) in any case. If the user wants it later, it needs a run-side `stop_rx` drain and a re-armed stop channel, out of scope here.

- **D4 — The headless-class guard stays.** A replacement whose `headless_for` result differs from `current_is_headless` still cold-starts, because the child's output configuration must change (headless audio runs with `show_audio_window = false` must not hold a video-window child).

- **D5 — Name the decision.** Two structured events at the decision point, following the existing `name:` convention: reuse emits one event naming the reused run's identity; a cold start emits one with a `reason` field from the one set `no_run`, `headless_mismatch`, `stop_requested`, `run_exited`, `send_failed`. The events live in `submit.rs` so every caller (Emby play, play queue, daemon replace) reports the same vocabulary.

- **D6 — Rename the daemon helper.** `submit_queue_slots_cold_start` becomes `submit_queue_slots_for_replace` (it is the ctrl-replace entry, and the reuse/cold decision belongs to the Controller, not the name). Rename the definition, the `lib.rs` re-export, the call sites (`control.rs`, `reconciliation.rs`, `output/handoff.rs`, `core/transition_dispatch.rs`), and the doc comments that call it a cold start.

- **D7 — `run_exited` closes the teardown window.** The run thread sets a per-run `run_exited` flag (an `Arc<AtomicBool>` on `Player`, replaced at each cold start and passed into the run thread) when the loop returns, before `drop(mpv)`. D1 check 4 reads it. Without it, `active=false` after `end_run` no longer routes to the cold path, and a submission sent during mpv teardown would be accepted by a dying run and lost. The same run takes an accepted submission and refreshes its run identity via the existing `submission_accepted()` path; the child never outlives the Owner. Invariants 22 and 24 are updated in the same change (task 4.1).

## Risks / Trade-offs

- [The send succeeds but the run exits in the same loop pass] → The loop drained its commands, then returned, without reading the new one. The replacement is lost while the Owner installed the queue; the user repeats the action. The window is one loop pass (at most the 50 ms poll ceiling), and `run_exited` removes the larger teardown window. Stated in the spec delta as the residual window. If it ever needs closing, the run must hand unread commands back at exit.
- [A submission revives a parked run whose child is in an unknown state] → The run-side install failure path ends the run with exactly one stop naming the slot (existing spec requirement), so the worst case is the behaviour users see today on a failed load, without the extra teardown.
- [Status briefly disagrees with the run during the send-fallback window] → The cold start re-seeds everything it publishes; the gap is microseconds inside one mutex scope.

## Migration Plan

Single binary, no persistence or protocol change: ship and revert like any code change.

## Open Questions

None.
