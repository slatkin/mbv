# Proposal

## Why

`PlayerEvent::Stopped::run_identity` and `PlayerEvent::TrackCompleted::run_identity`
are typed as `(PlaybackRequestId, PlaybackGeneration)`, but every construction
site hardcodes the `PlaybackRequestId` half to `0` — no caller ever supplies a
non-zero value. The same dead `0` rides through `PlaybackRunIdentity`
(`crates/mbv-core/src/daemon/run.rs`), `PendingIdleQueueLoad.stopped_run`
(`crates/mbv-core/src/daemon/core.rs`), and every `.into()`/tuple-literal call
site built around it. Collapsing the type to a plain `PlaybackGeneration`
(`u64`) removes that dead weight without changing any observed behavior.

## What Changes

- Change `PlaybackRunIdentity` (`crates/mbv-core/src/daemon/run.rs`) from a
  `{ request_id, generation }` struct to a newtype/plain `PlaybackGeneration`.
- Change `PlayerEvent::Stopped::run_identity` and
  `PlayerEvent::TrackCompleted::run_identity`
  (`crates/mbv-core/src/player/types.rs`) from
  `(PlaybackRequestId, PlaybackGeneration)` to `PlaybackGeneration`.
- Change `PendingIdleQueueLoad.stopped_run`
  (`crates/mbv-core/src/daemon/core.rs`) to match.
- Update every construction/comparison site to drop the `0` request-id slot:
  `player/run/commands.rs`, `player/run/queue.rs`, `player/run/types.rs`,
  `player/submit.rs`, `daemon/run.rs`, `daemon/control/queue_load.rs`,
  `daemon/event_loop/player_events.rs`, and the affected tests under
  `daemon/tests/` and `src/app/tests/`.
- **BREAKING**: `PlayerEvent::Stopped` and `PlayerEvent::TrackCompleted` cross
  the daemon/client ctrl-socket wire protocol and change shape (a tuple field
  becomes a scalar). Bump `CTRL_PROTOCOL_VERSION` (`crates/mbv-core/src/ctrl.rs`,
  currently `10`) to `11` so a daemon/client pair running mismatched versions
  is rejected at handshake (`CtrlCompatibility::for_peer`) instead of
  failing to deserialize an event later.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ctrl-protocol`: the protocol version requirement changes to require version
  11 (a non-additive bump), per the spec's own rule that a non-additive
  wire-shape change requires a version bump.

## Impact

- Affected code: `crates/mbv-core/src/daemon/run.rs`,
  `crates/mbv-core/src/daemon/core.rs`,
  `crates/mbv-core/src/daemon/control/queue_load.rs`,
  `crates/mbv-core/src/daemon/event_loop/player_events.rs`,
  `crates/mbv-core/src/player/types.rs`, `crates/mbv-core/src/player/submit.rs`,
  `crates/mbv-core/src/player/run/{commands,queue,types,run_loop}.rs`,
  `crates/mbv-core/src/player/run/events/{queue_advance,termination}.rs`,
  `crates/mbv-core/src/ctrl.rs`, and tests referencing `run_identity`/
  `stopped_run` in `daemon/tests/` and `src/app/tests/`.
- No user-visible behavior change: the always-zero request-id slot carried no
  information, so playback-run-identity comparisons are unaffected.
- Wire compatibility: any daemon and client must be rebuilt together (already
  true for every protocol-version bump today).
