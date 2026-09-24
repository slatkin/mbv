# Proposal

## Why

`run_with_options` in `crates/mbv-core/src/daemon_run.rs` mixes process setup (writing the pid file, loading credentials, binding sockets, spawning threads, `process::exit`) with the whole Stay-alive owner event loop. Because of that, the per-event queue logic cannot be unit tested. In particular, the batched owner-queue persistence (`owner_queue_dirty`) has no tests for the TrackCompleted, Stopped/pending idle-load, Ws and PlaybackResolved arms. The file is also 1032 lines, over the 800-line cap. (#768)

## What Changes

- Move the event loop into a new `daemon_loop.rs` as a `DaemonLoop` state struct with `handle_event(&mut self, ev: DaemonEvent) -> LoopFlow` (`Continue` / `Shutdown`). `run_with_options` keeps only setup, the receive loop, and the pid removal plus `process::exit` after `LoopFlow::Shutdown`.
- Inject the owner-queue store: `DaemonLoop` holds a save function instead of calling `save_stay_alive_queue_state()` directly, so tests can record snapshots.
- Destructure `PlayerEvent::Stopped` once per arm instead of re-matching `&pe` several times.
- Add unit tests for the four arms with synthetic events: no libmpv, no real state or config directories, no sockets.

No behaviour changes.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a pure refactor, so `skip_specs: true`.

## Impact

- `crates/mbv-core/src/daemon_run.rs` (shrinks), new `crates/mbv-core/src/daemon_loop.rs`, `daemon_control_queue.rs` (`persist_stay_alive_owner_queue` takes the injected save function), `daemon_tests.rs` or a new `daemon_loop_tests.rs`.
- No API, protocol or persistence-format changes.
