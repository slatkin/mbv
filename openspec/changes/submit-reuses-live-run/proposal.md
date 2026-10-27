# Proposal

## Why

A queue replacement re-submits the whole queue to the Owner. The Owner reuses the live mpv run only when its status flag reads active at that instant and the replacement's headless class matches; otherwise it stops the run, joins the thread, and spawns a fresh one. Two live runs exist where the flag reads inactive: a Standalone run that idles after its file ended (the mpv window stays up; `run/events/termination.rs`, "the run idles afterward"), and a run parked after a failed load (`accept_stopped_replacement`, `stop_for_active_file_failure`). Enter on a queue row in either state closes the mpv window and opens a new one. The intended local-playback behaviour is to reuse an existing mpv window for a compatible replacement; a status flag must not decide whether the window survives.

## What Changes

- The Owner-side queue submission (`Controller::submit_queue_slots`) reuses a live run whenever a run exists, its headless class matches, no stop was requested for it, and it has not exited. The send is the last test; a failed send falls back to the existing cold start.
- A submission into an idle or parked run (mpv child still alive) goes into that run instead of tearing it down.
- A run for which a stop was requested is never reused: the replacement cold-starts and plays once in a new run.
- A run whose loop has exited is never reused: the run thread publishes a `run_exited` flag when its loop ends, before the mpv child is dropped.
- The headless-class match stays a teardown reason: a replacement whose computed headless class differs from the running run's cold-starts.
- The reuse decision is observable: one structured log event names the outcome (reused live run, or cold start with the reason `no_run`, `headless_mismatch`, `stop_requested`, `run_exited`, or `send_failed`).
- The daemon helper `submit_queue_slots_cold_start` is renamed; it routes through the reuse decision and its name claims otherwise.
- Invariants 22 and 24 are updated to say that a live child may take a later submission.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `playback-run-lifecycle`: new requirement on which run a queue replacement enters — a replacement reuses a live run when one exists, its headless class matches, and it has neither a requested stop nor an exited loop; otherwise it cold-starts for a stated reason.

## Impact

- `crates/mbv-player/src/submit.rs` — the reuse decision and cold-start fallback; `crates/mbv-player/src/controller.rs` and `crates/mbv-player/src/run/run_loop.rs` — the `run_exited` flag (a field on `Player`, passed into the run thread, set when the loop returns).
- `crates/mbv-daemon/src/core.rs` and its call sites — helper rename only.
- `docs/invariants/22-mpv-child-never-outlives-the-owner.md`, `docs/invariants/24-run-ends-when-nothing-can-play.md` — wording.
- No ctrl-protocol, TUI, or config changes. The TUI's jump-vs-replace dispatch (`src/app/dispatch/action.rs`) is unchanged; the behaviour change is Owner-side.
- Invariant 22 (`mpv-child-never-outlives-the-owner`) still holds for the Owner-lifetime half: the child never outlives the Owner. Its item 1 ("a later run never reuses a live child") already disagrees with the existing fast path; the doc update makes it say what the code does. Invariant 24's parked-run provision ("the mpv child stays alive for a later submission") gains its intended consumer.
