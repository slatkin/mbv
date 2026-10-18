# Design

## Context

See `proposal.md` for the motivation. Today the owner holds a `PlayOrder` (queue-play-order spec). In Shuffle it picks a random next slot (`PlayerOwnerState::repick_upcoming`) and forwards it to the playback run (`PlayerCommand::SetUpcoming`). The queue slots never move.

Facts that shape the approach (all observed in the code):

- Queue edits already follow one pattern: a `CtrlCmd::UnifiedQueue*` handler in `crates/mbv-daemon/src/control/queue_edit.rs` edits the canonical `PlaybackQueue`, calls `broadcast_queue_state` once, then forwards per-slot `PlayerCommand`s to the playback run (`QueueMove`, `QueueRemove`).
- `PlaybackQueue::move_slot` bumps the revision on each call. Nothing reorders many slots in one revision.
- The run keeps its own queue copy and applies `QueueMove` with `cmd_queue_move`. That function also moves the mpv playlist entry (unless `active_file`) and shifts `current_idx`.
- The Emby Session output already copes with a queue edit that changes the queued next item: `check_queued_next` marks it stale, and the device is corrected when it moves to the stale item. A Cast receiver holds only the active item.
- `rand` is already a dependency of `mbv-player` (used by the pick being removed) and of the TUI crate.

## Goals / Non-Goals

**Goals:**

- One owner-side action that reorders the queue once, keeps the active slot first, and reaches all Clients as one snapshot.
- Remove the second playback mode with no leftover state, field, capability, or UI.

**Non-Goals:**

- No undo of a Shuffle, and no "stay shuffled" setting. The new order is the queue.
- No change to the library Shuffle entry that replaces the queue (`ShuffleFolder`, `shuffle_play_target`). That entry keeps its name and behavior. Note that its Rust helper names still say `shuffle_play`; they are not renamed here.
- No change to `QueueSource::Shuffle`, the recorded origin of a queue.

## Decisions

**D1. The owner shuffles, not the Client.** The handler in `queue_edit.rs` computes the order from the canonical queue, because the owner knows the active slot and is the only queue authority (AGENTS.md: Playback and queue authority). A Client-side shuffle would send many moves and would race with the playing slot.
*Alternative:* Client sends the full new order. Rejected: it carries a stale view of the queue and needs lineage and revision checks that the owner already has.

**D2. One new `PlaybackQueue` method reorders all slots in one revision.** `PlaybackQueue::reorder_slots(order: &[QueueSlotId])` replaces the slot sequence and bumps the revision once. It requires `order` to hold exactly the current slot ids and returns `NotFound` otherwise. The shuffle itself (pure function: active slot first, rest shuffled with an injected `Rng`) lives beside it, so tests inject a seeded RNG and need no sleep or randomness.
*Alternative:* call `move_slot` per slot. Rejected: N revisions, and a Client could see an intermediate order.

**D3. The run receives a move list, not a new command.** After one `broadcast_queue_state`, the handler sends `PlayerCommand::QueueMove(slot, index)` for each position whose slot is not yet in place (target index `k` from 0 up). `cmd_queue_move` already keeps the mpv playlist and `current_idx` right for each move, and the active slot moves first, so `current_idx` ends at 0.
*Alternative:* a new `PlayerCommand::QueueReorder(Vec<QueueSlotId>)` that rewrites the mpv playlist. Rejected for now: a second implementation of playlist surgery for mpv. The risk below covers the one thing a move list can lose.

**D4. The ctrl command mirrors the other queue edits.** `CtrlCmd::UnifiedQueueShuffle { op: Option<QueueOpId> }`, gated by a new hello capability `queue-shuffle` (`supports_queue_shuffle` on the peer hello, `PlayerProxy::supports_queue_shuffle` and `shuffle_queue`). The capability is additive, so there is no protocol version bump (ctrl-protocol: capability rule). `queue_op_id` and the role-gate lists in `commands.rs` get the new variant with an exhaustive arm (AGENTS.md: no wildcard on boundary requests).

**D5. Removing the play order is a deletion, not a deprecation.** Delete `mbv-queue/src/play_order.rs`, `mbv-daemon/src/play_order.rs` hooks that only serve the pick (`forward_play_order_advance`, `reset_play_order_for_queue_replacement`, `next_up_slot`'s Shuffle arm, `note_queue_slot_removed`, `note_queue_replaced`), `PlayerStatus.play_order`, `SharedQueueState.play_order`, `PlayerCommand::SetUpcoming`, `CtrlCmd::SetPlayOrder`, the `play-order` capability, `PlayerRunState.upcoming`, and the decision helper in `run/decisions.rs` reverts to `current_idx + 1`. `next_up_slot` and `dispatch_relative_step` stay but resolve the queue neighbor only. The `play-order` capability string is dropped from the hello of this build. An older peer that still sends `SetPlayOrder` gets the normal unknown-command handling of the ctrl protocol; an older Client never offers it, because the new owner does not advertise `play-order`.
*Alternative:* keep the order type and add the reorder next to it. Rejected by the user's request: Shuffle is "only shuffle the contents of the queue".

**D6. Menu: one entry, one label.** `push_play_order_toggle` becomes `push_queue_shuffle_entry` and always pushes `Shuffle` for `ContextAction::ShuffleQueue`, offered only when `supports_queue_shuffle` is true. `ContextAction::SetPlayOrder` is deleted; the dispatch arm in `actions.rs` calls `player.shuffle_queue()`. `QueueView.play_order` and the footer shuffle indicator in `chrome_status.rs` are deleted. The label text is paint, so it is not pinned in the spec; the entry test asserts the label because the user named it.

**D7. MPRIS.** `Shuffle` reads false and `CanGoPrevious` follows the queue as for any queue. The write side of `Shuffle` stays a no-op, as before.

**D8. Docs.** `CONTEXT.md` line on the retired Cast receiver term mentions `PlayOrder`. Reword it to say the Owner advances through the queue. Add a domain term *Queue shuffle* only if `CONTEXT.md` has no entry that covers it (check *Avoid* terms first; ask before a collision). No ADR: this removes a feature and adds an action inside an accepted architecture.

## Risks / Trade-offs

- [The run applies the move list in several commands, and a natural end could land between two moves] → The owner's event loop sends all moves back to back in one handler call, and the run drains its command channel per loop turn. Task 3.3 adds a run-level test that a natural advance after the full list follows the new order, and a check of the drain loop; if the drain is not atomic, switch to the `QueueReorder` alternative in D3.
- [Moving the active slot to index 0 shifts mpv's current index] → `cmd_queue_move` and `shift_index_for_move` already own this. Task 3.2 tests a Shuffle with the active slot in the middle and at the end.
- [Breaking removal of `play-order` and `SetPlayOrder`] → Mixed versions work: the new owner never advertises `play-order`, so an old Client hides its toggle; an old owner never advertises `queue-shuffle`, so a new Client hides Shuffle.
- [A saved queue written while Shuffle was on] → The play order was never persisted, so nothing to migrate.
- [Emby Session next item goes stale] → Covered by existing `check_queued_next`; task 4.2 tests that a Shuffle marks it stale.

## Migration Plan

Single release. No data migration. Rollback is a revert of the change; no persisted format changes.
