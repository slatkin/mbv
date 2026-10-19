# Tasks

Read `design.md` first. Run `cargo fmt` after each Rust change. Add no lint suppression. Tests are hermetic: seeded RNG, no sleeps, no live mpv.

## 1. Queue core: one-revision reorder

- [x] 1.1 Add `PlaybackQueue::reorder_slots(&[QueueSlotId])` in `crates/mbv-queue/src/lib.rs`: replace the slot sequence, bump the revision once, return `NotFound` when the ids are not exactly the current slots. Verify with a `crates/mbv-queue/tests/` case for the contract "one revision bump, slot ids unchanged, mismatched ids leave the queue untouched".
- [x] 1.2 Add the pure shuffle function (active slot first, rest shuffled with an injected `Rng`, no active slot shuffles all). Verify with a seeded-RNG test per spec scenario "Shuffle with a playing item / nothing playing / one slot": active first, same id set, one-slot queue unchanged.

## 2. Ctrl: command and capability (additive)

- [x] 2.1 Add `CtrlCmd::UnifiedQueueShuffle { op: Option<QueueOpId> }` in `crates/mbv-ctrl/src/commands.rs` with exhaustive arms in the role-gate and queue-op lists, and `queue_op_id` in `crates/mbv-daemon/src/control.rs`. Verify with `cargo check --workspace` and a wire round-trip case in `crates/mbv-ctrl/tests/ctrl/wire.rs` (a payload without `op` decodes).
- [x] 2.2 Add the `queue-shuffle` hello capability and `supports_queue_shuffle` in `crates/mbv-ctrl/src/hello.rs` and `protocol.rs`, with no protocol version change. Verify with a hello test: a peer without the capability reads false.

## 3. Owner: shuffle handler and run reorder

- [x] 3.1 Add `handle_queue_shuffle` in `crates/mbv-daemon/src/control/queue_edit.rs`: shuffle with the active slot first, call `reorder_slots`, call `broadcast_queue_state` once, answer the op with the applied state. Verify with a `crates/mbv-daemon/src/tests/queue_ops/` test for the contract "one snapshot, active slot first, same slots" (spec requirement "Shuffle is one queue edit").
- [x] 3.2 Forward the reorder to the run as one `PlayerCommand::QueueReorder(order)` from design D3 and add `PlayerProxy::supports_queue_shuffle`/`shuffle_queue` in `crates/mbv-player/src/proxy.rs`. Verify with a run-level test: active slot in the middle and at the end ends at index 0, and `current_idx` and the mpv playlist order match the queue.
- [x] 3.3 Verify advance after Shuffle. Test that a natural end and Next follow the new order and that the Next Up card names the new next slot. Also read the run's command drain (`run_loop.rs`) and record in `design.md` whether a natural end can land between two moves; if it can, replace D3 with a `QueueReorder` command before going on.
- [x] 3.4 Emby Session: test that a Shuffle during an Emby Session output marks the queued next item stale and that the active item is not interrupted (`crates/mbv-daemon/src/emby_session/tests.rs`, spec scenario "Queue Shuffle changes the queued next slot").

## 4. TUI: the Shuffle entry

- [x] 4.1 Replace `push_play_order_toggle` in `src/app/dispatch/context_menu/menu.rs` with `push_queue_shuffle_entry` (label "Shuffle", gated on `supports_queue_shuffle`), add `ContextAction::ShuffleQueue`, and route it in `actions.rs` to `player.shuffle_queue()`. Update `src/app/tests/context_menu_entries.rs` for the contract "every Queue row offers Shuffle; library rows do not; owner without the capability hides it". Verify with `cargo nextest run -p mbv`.

## 5. Remove the playback order

- [x] 5.1 Delete the TUI side: `ContextAction::SetPlayOrder`, `QueueView.play_order`, the footer shuffle indicator in `chrome_status.rs` and its test, and `PlayerProxy::supports_play_order`/`set_play_order`. Verify with `cargo check -p mbv` and the footer tests passing.
- [x] 5.2 Delete the wire side: `CtrlCmd::SetPlayOrder`, `PlayerStatus.play_order`, `PlayerCommand::SetUpcoming`, the `play-order` capability, and the remote-player handshake flag. Verify with `cargo check --workspace`; a payload that carries `play_order` still decodes (unknown field ignored) in a `wire.rs` case.
- [x] 5.3 Delete the owner side: `PlayOrder`/`PlayOrderMode` and `crates/mbv-queue/src/play_order.rs`, the order field and hooks in `owner_state.rs`, `SharedQueueState.play_order`, the order hooks in `mbv-daemon` (`play_order.rs`, `queue_edit.rs`, `queue_setup.rs`, `queue_load.rs`, `output/`, `emby_session/`), `PlayerRunState.upcoming`, and the run's advance reverts to `current_idx + 1`. Delete `crates/mbv-daemon/src/tests/queue_ops/play_order.rs` and the tests of removed code. Verify with `cargo check --workspace` and `cargo nextest run -p mbv-player -p mbv-daemon -p mbv-queue`.
- [x] 5.4 MPRIS: `Shuffle` reports false and `CanGoPrevious` follows the queue; remove the order helpers and their tests in `crates/mbv-desktop/src/mpris.rs`. Verify with `cargo nextest run -p mbv-desktop`.

## 6. Docs and final checks

- [x] 6.1 Update `CONTEXT.md` (the retired Cast receiver entry that names `PlayOrder`; check *Avoid* terms before any new term). Verify with `rg -n "PlayOrder|play order" CONTEXT.md docs openspec/specs` showing only archived or intended mentions.
- [x] 6.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`; run `make check-code-file-lines` just before push. Verify all pass.
- [x] 6.3 Live test passed (user, 2026-10-19): Shuffle on a playing queue keeps the playing item first and plays on; Next follows the new order.

## Workflow follow-up

- Sync the delta specs into `openspec/specs/` and archive this change only when the user approves the PR for merge, as the last commit before the merge. Do not sync or archive when the PR opens.
