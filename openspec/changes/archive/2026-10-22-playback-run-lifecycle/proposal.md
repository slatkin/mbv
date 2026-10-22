# Proposal

## Why

The decision "this Playback run ends" is spread across mpv event-decode traps and loose flags in `crates/mbv-player/src/run/`. Five recent fixes landed here (`177e08d5a`, `7971bf149`, `300cc74fb`, `582082a57`, `e1dc8a021`), and Invariant 24 still names an open residual. A read-only recon for this change found more defects of the same kind: stop paths that send no `Stopped`, stop paths that can send two, and a jump-target failure that reads the wrong successor. Issue #950 (Candidate 2) proposes one deep run-lifecycle module, so the next mpv event-shape surprise has one place to land.

## What Changes

- One run-lifecycle module in `mbv-player` owns the "has playback begun" boundary, jump arming, the active-file projection flag, and the single exit that sends `PlayerEvent::Stopped`. It takes lifecycle facts (load failed, playback started, entry ended, jump armed, submission accepted) instead of raw `MpvEvent` values. ADR 0032 and the mpv event decoder stay as they are.
- Every run end sends exactly one `Stopped` for its run identity. Today these paths send none: a fatal error after mpv moved to another entry by itself; a failed active-file jump; and a failed promotion to the active-file projection. A failed active-file replacement can send two.
- A jump whose target fails to load is decided from the target, not from the entry played before. The armed jump settles or clears on that failure, so playlist-position drift is not left muted and a later restart does not get the failed target's resume seek.
- The "has playback begun" boundary resets for each new submission into a reused run. A failure before the first restart of a new submission is then a start failure again (#941).
- Invariant 24 is rewritten to the new mechanism. Its "Where it currently fails" entry is closed.

No **BREAKING** changes. The `PlayerEvent` wire shape does not change.

## Capabilities

### New Capabilities

- `playback-run-lifecycle`: when a local Playback run ends because nothing can play, which entry the failure belongs to, and the rule that each run identity ends with exactly one `Stopped`.

### Modified Capabilities

None. `mpv-process-isolation` already covers a crash of the mpv child, and this change keeps that behavior.

## Impact

- Code: `crates/mbv-player/src/run/` (`types.rs`, `state.rs`, `events/termination.rs`, `events/restart.rs`, `events/queue_advance.rs`, `events/playlist_drift.rs`, `commands/queue.rs`, `commands/load.rs`, `run_loop.rs`, `queue.rs`) and a new lifecycle module beside them.
- Tests: the `mbv-player` tests that set `playback_began`, `active_file`, `forced_jump`, `quit_at`, or `stop_slot` directly (`src/tests/submit.rs`, `src/tests/session.rs`, `src/tests/session_feed.rs`) move to the lifecycle interface.
- Docs: `docs/invariants/24-run-ends-when-nothing-can-play.md`.
- No change to `mbv-daemon`, `mbv-ctrl`, or the TUI. The Owner already drops a `Stopped` whose run identity is not current. It does not remove duplicates, so a single send per run is a Player-side duty.
