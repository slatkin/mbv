# Tasks

## 1. Protocol version bump

- [ ] 1.1 Bump `CTRL_PROTOCOL_VERSION` from `10` to `11` in
  `crates/mbv-core/src/ctrl.rs`; verify with `cargo check -p mbv-core`.

## 2. Core type change

- [ ] 2.1 In `crates/mbv-core/src/player/types.rs`, change
  `PlayerEvent::Stopped::run_identity` and
  `PlayerEvent::TrackCompleted::run_identity` from
  `(PlaybackRequestId, PlaybackGeneration)` to `PlaybackGeneration`, keeping
  `#[serde(default)]`; update their doc comments to drop the request-id
  mention.
- [ ] 2.2 In `crates/mbv-core/src/daemon/core.rs`, change
  `PendingIdleQueueLoad.stopped_run` from
  `(PlaybackRequestId, PlaybackGeneration)` to `PlaybackGeneration`.
- [ ] 2.3 In `crates/mbv-core/src/daemon/run.rs`, change `PlaybackRunIdentity`
  from a `{ request_id, generation }` struct to a plain `PlaybackGeneration`
  (type alias or tuple-struct wrapper, whichever needs the least call-site
  churn); drop the `From<(PlaybackRequestId, PlaybackGeneration)>` impl; update
  `playback_run_identity_is_current` to compare against
  `player.status.lock().unwrap().sequence_generation` directly. Verify with
  `cargo check -p mbv-core` (expect cascading errors at every call site fixed
  in tasks 3-5).

## 3. `player/` call sites

- [ ] 3.1 Fix `run_identity` construction/assignment in
  `crates/mbv-core/src/player/run/commands.rs` (the `self.run_identity =
  (0, ...)` assignment) and `crates/mbv-core/src/player/run/queue.rs` (the
  `let run_identity = (0, ...)` binding) to produce a bare
  `PlaybackGeneration`.
- [ ] 3.2 Fix the `run_identity` field type in
  `crates/mbv-core/src/player/run/types.rs` and its propagation through
  `crates/mbv-core/src/player/run/run_loop.rs`,
  `crates/mbv-core/src/player/run/events/queue_advance.rs`, and
  `crates/mbv-core/src/player/run/events/termination.rs` (all `run_identity:
  self.run_identity` field-init shorthand sites — no logic change, just
  follow the new type).
- [ ] 3.3 Fix `crates/mbv-core/src/player/submit.rs` (the `let run_identity =
  (0, self.advance_sequence_generation())` binding at line ~78 and its three
  uses). Verify with `cargo check -p mbv-core`.

## 4. `daemon/` call sites

- [ ] 4.1 Fix `crates/mbv-core/src/daemon/control/queue_load.rs`: the
  `stopped_run`/`current_run`/`run_identity` tuple literals and the
  `complete_pending_idle_queue_load` parameter type.
- [ ] 4.2 Fix `crates/mbv-core/src/daemon/event_loop/player_events.rs`: the
  `run_identity.into()` / `(*run_identity).into()` conversions (now a no-op
  copy instead of a `From` call — drop the `.into()`) in
  `handle_track_completed` and `handle_player_event`, and the `stopped_run`
  comparisons. Verify with `cargo check -p mbv-core`.

## 5. Tests

- [ ] 5.1 Fix `run_identity`/`stopped_run` literals in
  `crates/mbv-core/src/daemon/tests/loop.rs` and
  `crates/mbv-core/src/daemon/tests/queue_ops/queue_idle_load.rs` (drop the
  `(0, n)` tuples to bare `n`).
- [ ] 5.2 Fix `run_identity: (0, 0)` literals in
  `src/app/tests/queue/consume.rs`, `src/app/tests/route_state/session.rs`,
  and `src/app/dispatch/actions/queue_enrich_tests.rs` (drop to bare `0`).
- [ ] 5.3 Verify with `cargo nextest run -p mbv-core` and
  `cargo nextest run` (workspace TUI tests) — all pre-existing tests pass
  unchanged, since this is a type-shape change with no logic change.

## 6. Finish

- [ ] 6.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `make check-code-file-lines`; fix any fallout.
