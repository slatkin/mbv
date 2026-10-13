# Design

## Context

See proposal.md - Why. Two places decide the next slot today:

- **Steps (Next / Previous):** the daemon owner core resolves them with `PlayerOwnerState::relative_step_target` (`crates/mbv-player/src/owner_state.rs`), called from `crates/mbv-daemon/src/event_loop.rs` and `crates/mbv-daemon/src/control/playback.rs`. It returns the neighbor slot.
- **End of file and Next Up:** the Playback run (the mpv thread) uses `current_idx + 1`, in `PlaybackRun::settle_forced_jump` (`crates/mbv-player/src/run/events/queue_advance.rs`) and in `queue_next_up_decision` (`crates/mbv-player/src/run/decisions.rs`).

Every local TUI and `mbvd` use the `mbv-daemon` owner core, so the core is the one authority for the order. The core already sees every active-slot change through `note_observed_active_slot`, and every whole-queue replacement through a new `QueueLineage`.

The queue state that Clients receive is `UnifiedQueueStateData` (`crates/mbv-ctrl/src/protocol.rs`). It already carries `source` and `lineage`.

## Goals / Non-Goals

**Goals:**
- One authority (the owner core) for the order and for the picked next slot.
- Sequential behavior stays the same, path by path.

**Non-Goals:**
- MPRIS write support for Shuffle.
- Saving the order across an owner restart. A new owner starts in Sequential.
- A Repeat mode.

## Decisions

### D1. The order is an enum that holds its pick

```rust
pub enum PlayOrder {
    Sequential,
    Shuffle { upcoming: Option<QueueSlotId> },
}
```

It lives in `crates/mbv-queue` (shared by ctrl, player, and UI) and is a field of `PlayerOwnerState`. The picked slot exists only inside `Shuffle`, so "a pick while Sequential" cannot be represented. `upcoming` is `None` only when the queue has no slot other than the active slot.

*Alternative:* a `shuffle: bool` plus a separate `Option<QueueSlotId>`. Rejected: two fields that must change together.

### D2. The core picks; the Playback run obeys

The core re-picks `upcoming` when:
- Shuffle turns on,
- the observed active slot changes (`note_observed_active_slot`),
- a queue edit removes the `upcoming` slot, or an append makes a pick possible where none was.

The pick is a uniform random choice over all slots except the active slot, with `rand` (already a workspace dependency). Put the pick in one pure function that takes the candidate slots and a `&mut impl Rng`, so tests pass a seeded RNG.

The core sends the pick to the Playback run with a new in-process command `PlayerCommand::SetUpcoming(Option<QueueSlotId>)`. It never crosses the ctrl wire (the same rule as `JumpTo`). Sequential sends `SetUpcoming(None)`.

The Playback run stores the value and uses it in exactly two places:
- `settle_forced_jump`: when there is no forced jump, use the index of the `upcoming` slot. With no upcoming value, keep `current_idx + 1`.
- `queue_next_up_decision`: take the next index as an input instead of computing `current_idx + 1`. The run passes the `upcoming` index or `current_idx + 1`.

When Shuffle is on and `upcoming` is `None`, the run must treat the advance as the end of the queue. So the value the run holds is a three-state type, not an `Option`: `Advance::Sequential`, `Advance::To(QueueSlotId)`, `Advance::End`. `SetUpcoming` carries that type.

*Alternative:* the Playback run holds the order and picks. Rejected: the step path is resolved in the core, so the order would be stored twice.

### D3. Steps

`relative_step_target(Direction::Next)` returns `Jump(upcoming)` in Shuffle, or `AtEdge` when `upcoming` is `None`. `relative_step_target(Direction::Previous)` returns `AtEdge` in Shuffle, so Previous does nothing. The coalescing rule stays the same.

### D4. Reset on queue replacement

When the core installs a new `QueueLineage` (a whole-queue replacement), it sets the order to `Sequential` and sends `SetUpcoming(Advance::Sequential)`. Append, remove, and move keep the lineage, so they keep the order.

### D5. Ctrl wire

- New `CtrlCmd::SetPlayOrder(PlayOrderMode)`, where `PlayOrderMode` is `Sequential | Shuffle` (the client never sends a pick).
- New capability `CTRL_CAP_PLAY_ORDER`. The client sends the command only when the owner advertises it. If not, the menu does not offer the entry.
- New field `#[serde(default)] play_order: PlayOrderMode` on `UnifiedQueueStateData`. No protocol version bump: new variant, new capability, new default field.

### D6. Menu

The Queue branch of the context-menu target resolution returns `None` today for non-Emby rows (`src/app/dispatch/context_menu/menu.rs`). Change it so that every Queue row opens a menu. Emby rows keep their current entries. Feed and Audiobookshelf Queue rows get only the shuffle-order entry. The entry is appended after the current Queue entries, so it is the last entry.

New `ContextAction::SetPlayOrder(PlayOrderMode)`. The shell builds it with the opposite of the current mode, and its dispatch arm sends the ctrl command. The label is a paint choice: use "Shuffle Play" when Sequential and "Stop Shuffle" when Shuffle.

### D7. Footer and MPRIS

- The footer shows a shuffle indicator from the projected `play_order`. It does not depend on Queue focus, unlike the queue-source pill in `chrome_status.rs`.
- MPRIS `shuffle()` and `can_go_previous()` read the snapshot. Both then use `&self`, so their approved `#[expect(clippy::unused_self)]` attributes become unfulfilled. Delete them.

## Risks / Trade-offs

- [The run advances before the core's new pick arrives (a fast end of file after a queue edit)] → The run uses the last `upcoming` it got. If that slot is gone, `slot_index` returns `None`, and the run falls back to `current_idx + 1`. This is rare and harmless.
- [Remote Emby authority drives the owner] → Emby remote commands do not touch the order. They play through the same advance code, so Shuffle still applies.
- [An older `mbvd` without the capability] → The menu hides the entry. Nothing breaks.

## Migration Plan

No data migration. The order is not saved. Rollback is a plain revert.
