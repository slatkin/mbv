# Invariant 25 — The observed active slot is the slot the output plays

**Scope:** `PlayerOwnerState::note_observed_active_slot` and `relative_step_target` (`crates/mbv-player/src/owner_state.rs`), `observe_slot` and `correct_stale_device_advance` (`crates/mbv-daemon/src/output/advance.rs`), `settle_remote_transition` (`crates/mbv-daemon/src/core/transition_dispatch.rs`), and `SharedQueueState::publish_observed` (`crates/mbv-daemon/src/core.rs`).

## The invariant

1. The observed active slot is the slot the output plays now. It is not the slot the Owner last asked the output to load. Relative steps resolve from it when no transition is in flight, and Clients highlight it.
2. Only `note_observed_active_slot` writes it. Every path that changes the slot an output plays records the new slot, then publishes it with `publish_observed`.
3. A path that loads a slot with no in-flight transition records the slot itself. No later observation settles a transition for that slot, so nothing else records it.

## Why it matters

A relative Next or Previous resolves from the observed slot. If the slot still names the item the device has left, Next reloads the corrected item instead of stepping past it. A stale published slot makes Clients highlight the wrong row. Commit `8484f0ef1` fixed both for the Emby Session stale-device correction.

## How the code maintains it today

- A settled load: `settle_remote_transition` records the loaded slot, settles the transition, and publishes.
- A device-driven advance: `apply_device_advance` calls `observe_slot` for the slot the device plays, then publishes.
- A stale-device correction: `correct_stale_device_advance` checks the Session id, then calls `observe_slot` before it resolves the window. It publishes the corrected slot after that.
- Guard test: `a_removed_queued_slot_is_corrected_with_the_queue_next_slot` in `crates/mbv-daemon/src/tests/emby_session_end/device_window.rs` asserts the observed slot, the published slot, and that a following Next steps from the corrected slot.

## Where it currently fails / how it could regress

- A new path that changes the played slot on a remote output, and skips both `settle_remote_transition` and `observe_slot`, leaves the observed slot on the old item. Nothing enforces this. The guard test covers only the stale-correction path.
- A note without a following `publish_observed` leaves Clients highlighting the old row while the Owner steps from the new slot.
