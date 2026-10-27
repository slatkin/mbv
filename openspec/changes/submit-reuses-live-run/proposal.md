# Proposal

## Why

Enter on a queue row while the panel shows the queue that is not playing re-submits the whole queue to the Owner. The Owner reuses the live mpv run only when its status flag reads active at that instant and the replacement's headless class matches; otherwise it stops the run, joins the thread, and spawns a fresh one. The user sees the playing mpv window close and a new one open. The intended local-playback behaviour is always to reuse an existing mpv window for a compatible replacement; a momentary status read must not decide whether the window survives.

## What Changes

- The Owner-side queue submission (`Controller::submit_queue_slots`) reuses a live run whenever a run thread exists and accepts the submission, instead of requiring `status.active == true` at decision time. The send itself is the liveness test; a failed send falls back to the existing cold start.
- A submission into a run that is idle or parked (playback ended or a failed load parked the run, mpv child still alive) now goes into that live run instead of tearing it down and starting a new child.
- A submission that arrives while the run is stopping cancels the stop and plays in the same run (the mechanism already exists for `LoadNew`; `SubmitQueue` gains the same treatment in behaviour, not code).
- The headless-class match stays the one legitimate teardown reason: a replacement whose computed headless class differs from the running run's still cold-starts.
- The reuse decision becomes observable: one structured log event names the outcome (reused live run, or cold start with the reason `no_live_run` or `headless_mismatch`).
- The daemon helper `submit_queue_slots_cold_start` is renamed; it routes through the reuse decision and its name claims otherwise.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `playback-run-lifecycle`: new requirement on which run a queue replacement enters — a replacement reuses a live run when one exists and the headless class matches, and cold-starts only when no run exists, the headless class differs, or the live run cannot take the submission.

## Impact

- `crates/mbv-player/src/submit.rs` — the reuse decision and cold-start fallback; `crates/mbv-player/src/controller.rs` — no signature changes expected.
- `crates/mbv-daemon/src/core.rs` — helper rename only.
- No ctrl-protocol, TUI, or config changes. The TUI's jump-vs-replace dispatch (`src/app/dispatch/action.rs`) is unchanged; the behaviour change is entirely Owner-side.
- Behaviour note for users: a Stop immediately followed by an enter press on a queue row now keeps the window and plays the selection, instead of closing the window and reopening it.
- Invariant 22 (`mpv-child-never-outlives-the-owner`) still holds: the same run takes the new submission and refreshes its identity; no child ever serves two runs. Invariant 24's parked-run provision ("the mpv child stays alive for a later submission") gains its intended consumer.
