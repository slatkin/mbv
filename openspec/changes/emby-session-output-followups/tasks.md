# Tasks

## 1. Retire `UnifiedQueueLoadIdle`: ctrl vocabulary and clients

- [ ] 1.1 Remove the wire vocabulary in `crates/mbv-ctrl` (walk the ctrl-protocol variant recipe's site list in reverse): `CtrlCmd::UnifiedQueueLoadIdle`, `CtrlEvent::UnifiedQueueLoadResult`, `QueueLoadResult`, `QueueLoadRequestId`, `OwnerGateRejection::QueueLoadIdle`, every match arm naming them, and the `lib.rs` exports. Delete the ctrl tests that name the variant (`tests/ctrl/queue.rs`, wire cases). Verify: `cargo check -p mbv-ctrl` and `cargo nextest run -p mbv-ctrl` green, and `rg "UnifiedQueueLoadIdle|UnifiedQueueLoadResult|QueueLoadRequestId|QueueLoadResult" crates src` finds nothing.
- [ ] 1.2 Remove the client side in `crates/mbv-remote-player`: `load_queue_idle`, the `UnifiedQueueLoadResult` relay arm in `connect.rs`, the `PlayerEvent::UnifiedQueueLoadResult` variant, and the idle-load cases in `connect/tests/queue.rs`. Keep `supports_owner_queue_load` (it gates `QueueOp::SourceUpdate`). Verify: `cargo nextest run -p mbv-remote-player` green.

## 2. Retire the daemon's pending idle load and the TUI's dead result arm

- [ ] 2.1 Delete `crates/mbv-daemon/src/control/queue_load.rs` and its dispatch arm in `control.rs`; delete `PendingIdleQueueLoad`, `pending_idle_load`, `queue_load_span` once uncalled, and the pending-load arms in `event_loop/player_events.rs` and `event_loop/control_events.rs`; collapse `broadcast_player_event_if_not_replaced` into `broadcast` once `replacement_committed` has no true source. Owner restore of a persisted queue stays untouched. Verify: `cargo check -p mbv-daemon` green and `rg "pending_idle_load|PendingIdleQueueLoad|queue_load" crates/mbv-daemon/src` finds nothing beyond unrelated names.
- [ ] 2.2 Delete the dead `UnifiedQueueLoadResult` handling in `src/app/dispatch/session/player_event.rs` together with the `PlayerEvent` variant; the exhaustive match is the proof. Verify: `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] 2.3 Delete the tests that own the removed contract (`crates/mbv-daemon/src/tests/queue_ops/queue_idle_load.rs`, pending-load cases in `tests/loop/persistence.rs` and `tests/queue_ops/queue_source_updates.rs`), and keep each surviving contract under the test that already owns it: an answered replacement replaces and plays (queue_ops replace tests), a source update still requires `owner-queue-load` (queue_source_updates). Verify: `cargo nextest run -p mbv-daemon` green.

## 3. Adopt the device's queue-end autoplay on an Emby Session output

- [ ] 3.1 Add the queue-end continuation row to the Session observation handling in `crates/mbv-daemon/src/emby_session/` (beside the D14 device-move row): when the tracked item ended within 10 seconds of its runtime, the device holds no queued next item, and the Session starts an item the owner did not load, append that item at the queue tail, make it the active slot and the tracked item, and record the ended slot's completion under the device-move rule. Verify: one test owning the contract "the queue-end autoplay is adopted" (fake `SessionApi`) asserting the appended slot, the active slot, and completion of the ended slot.
- [ ] 3.2 Make the adopted item the tracked item for progress, completion, and end inference (the same tables as a loaded item); a device move to a stale queued item keeps its correction and adopts nothing. Verify: tests "an adopted item is tracked like a loaded item" and "adoption repeats" (a second autoplay after an adopted item), and the existing stale-item correction test extended to assert the queue gains no slot.
- [ ] 3.3 Pin the navigation edge from the spec scenario "Next on an adopted item follows the queue": next on the adopted queue-end slot loads nothing and sends no device command, and device idle after an adopted item loads nothing. Verify: one case in the emby_session tests unless an existing queue-edge test already owns the claim (extend it instead).

## 4. Owner build check: refuse a stale same-version Owner (issue #949 item 3)

- [ ] 4.1 Add `crates/mbv-ctrl/build.rs` and a `BUILD_ID` constant (git short SHA plus `-dirty` when the tree is dirty, `unknown` when git is unavailable, `cargo:rerun-if-changed` on the workspace `.git/HEAD` and its ref), and `CtrlHello.build_id` with `#[serde(default)]`, filled by `current()`. Verify: `cargo check -p mbv-ctrl`; a ctrl test that `build_id` round-trips in the hello and that a hello without it decodes empty.
- [ ] 4.2 Extend the `PeerBuild::MustMatch` comparison in `crates/mbv-remote-player/src/connect/handshake.rs`: refuse when the version or the build id differs and when the Owner's hello carries no build id; keep the refusal before `send_client_hello` (invariant 15); `owner_build_mismatch` carries the Owner's version and build id. Verify: connect tests — same build attaches (existing), differing version refuses before any credential (existing), same version with a different build id refuses (new, cites #949), empty Owner build id refuses (new).
- [ ] 4.3 Show both sides' version and build identity in the mismatch prompt (`src/owner_restart.rs`); the restart/quit contract is unchanged. Verify: prompt tests updated, `cargo nextest run -p mbv` green.
- [ ] 4.4 Update `docs/invariants/15` (the gate compares version and build identity; correct the handshake path to `connect/handshake.rs`; document the `unknown` fallback and the same-commit limitation) and `docs/invariants/13` ("until the next queue replacement or clear"). Verify: the two documents read true against `read_server_hello`, and land in the same commit as the code.

## 5. Integration gates

- [ ] 5.1 After the groups: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `openspec validate --strict emby-session-output-followups`, and `make check-code-file-lines` just before push. Verify: all green.

## Workflow follow-up

- Sync the deltas into `openspec/specs/` and archive only when the user approves the PR
  for merge (the last commit before the merge).
- Issue #949 items 1–3 are done by this change; item 4 (Brave Linux stall) stays open
  with its "reopen if it comes back" note. Whether to close #949 is the user's call.
