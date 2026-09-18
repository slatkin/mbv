## Context

See proposal.md for motivation. The stay-alive daemon's queue tracking depends on a closed feedback loop: JumpTo → mpv EndFile → `on_end_file` → TrackChanged → daemon updates `observed_active_slot` → Next/Previous resolves from the new position. Three independent gaps break this loop.

Key files:
- `player/run/commands.rs:33-82` — JumpTo handler, active_file early return
- `player/run/queue.rs:350-370` — `select_active_slot` (no TrackChanged)
- `player/run/events.rs:525-689` — `on_end_file` (emits TrackChanged)
- `player/mod.rs:79-157` — `queue_load_indices`, `reassert_queue_layout`
- `daemon_control.rs:261-316` — Next/Previous base_idx resolution
- `daemon_run.rs:273-314` — TrackChanged handler (sole `observed_active_slot` writer)
- `playback/transition.rs:146-161` — `settle` dual match

## Goals / Non-Goals

**Goals:**
- Next/Previous works correctly in active_file mode (ABS items)
- Next/Previous works correctly in normal mode without 5s stall on settle mismatch
- No wrong-track flash on queue load at non-zero index
- `observed_active_slot` lifecycle is coherent across queue replacement

**Non-Goals:**
- Changing the active_file projection model itself
- Changing the OwnerTransitionState two-slot pipeline design
- Addressing cold-start idx=0 playback (separate investigation needed — adoption seeds the queue but something else triggers playback; instrumentation required before a fix)

## Decisions

### D1: Emit TrackChanged from the active_file JumpTo path

**Choice**: After `select_active_slot` succeeds in the active_file branch of the JumpTo handler (`commands.rs:52-58`), emit a `TrackChanged` event carrying the transition's `(request_id, generation)` and the target `slot_id` — the same event shape that `on_end_file` produces for non-active_file transitions.

**Why**: The active_file path calls `select_active_slot` which updates `current_idx` and syncs status, but never produces the TrackChanged event that the daemon needs to update `observed_active_slot` and settle the transition. Without this event, `observed_active_slot` stays `None` permanently and every Next/Previous resolves from the original queue position.

**Alternative**: Have `select_active_slot` itself emit TrackChanged. Rejected because `select_active_slot` is also called from non-JumpTo paths (initial load, natural advance) where the transition tag would be wrong or absent. The JumpTo handler is the right place because it owns the `forced_transition`.

### D2: Clear `observed_active_slot` on queue replacement

**Choice**: In `UnifiedQueueReplace` (and the owner's `submit_queue_slots` path), clear `observed_active_slot` to `None` on both the `PlayerOwnerState` and the `SharedQueueState` mutex when a new queue replaces the old one.

**Why**: `observed_active_slot` holds a `QueueSlotId` from the prior queue. After replacement, that slot ID is meaningless. The `reset_slot_jumps` call already clears transitions; observed state should be cleared alongside it.

**Alternative**: Reset to the new queue's start slot immediately. Rejected because that would violate the "observed active slot follows playback observations" requirement — we should wait for the first TrackChanged from the new playback run.

### D3: Load-then-play for `queue_load_indices`

**Choice**: Restructure `queue_load_indices` so that the "replace" loadfile does NOT start playback immediately. Instead: (1) load start_idx with `replace` + `no` (mpv's "don't play" flag for loadfile), (2) insert items before, (3) append items after, (4) set `playlist-pos` to the correct index, which starts playback at the right item.

**Why**: The current approach loads start_idx with "replace" which starts playback at mpv pos 0 immediately. Subsequent insert-at operations shift that position, creating a window where mpv's pos 0 points at a different item. `reassert_queue_layout` corrects this after loading, but the damage (brief wrong-track flash, potential wrong TrackChanged) is already done.

**Alternative**: Keep load-then-reassert but suppress TrackChanged until reassert completes. Rejected — the mpv API doesn't support transactional playlist construction, but it does support `loadfile ... no` (append without playing), so load-then-play is clean and simple.

### D4: Investigate settle matching relaxation

**Choice**: Investigate (with instrumentation) whether `settle` should match on `request_id` alone rather than `(request_id, target_slot)`. If a natural track advance fires EndFile during an in-flight JumpTo, the EndFile carries no transition tag, so it won't interfere. The dual match was designed to prevent a wrong slot from settling the transition, but it may be over-constrained.

**Why**: If mpv reports the requested slot under a different mechanism (e.g., property change rather than EndFile), the exact dual match would reject a correct observation. This needs instrumentation to determine whether it happens in practice.

**Deferral**: This is lower priority than D1-D3. D1 fixes the active_file path entirely. D3 eliminates the loading-race source of mismatches. D4 is a robustness improvement for the non-active_file path.

## Risks / Trade-offs

- **D1 double TrackChanged**: If active_file mode somehow also triggers EndFile (it shouldn't — there's no playlist-pos change), we'd get two TrackChanged events. Mitigation: the transition pipeline already handles duplicate settle attempts; the second is Ignored.
- **D3 playback start delay**: Loading the entire playlist before starting playback adds latency proportional to queue size. For 126 items this is sub-second (IPC-local mpv commands). If profiling shows it's noticeable, batch the loadfile commands.
- **D2 momentary None observed**: Between queue replacement and the first TrackChanged from the new run, `observed_active_slot` is None. This is correct — Next/Previous falls back to `queue.active_slot_id()` which is the start position of the new queue.
