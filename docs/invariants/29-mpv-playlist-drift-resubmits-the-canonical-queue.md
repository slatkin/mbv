# Invariant 29 — A playlist drift observation resubmits the canonical queue

**Scope:** `on_playlist_count_changed` (`crates/mbv-player/src/run/events/playlist_drift.rs`), `cmd_submit_queue` (`crates/mbv-player/src/run/commands/load.rs`), and `handle_queue_desynced` (`crates/mbv-daemon/src/event_loop/player_events.rs`).

## The invariant

1. mpv's playlist is a projection of the Owner's canonical queue. The player run's queue copy is the identity → mpv-index map every slot-addressed mpv operation reads: a `playlist-remove` on a queue removal, and a jump's `playlist-pos`.
2. A count observation that differs from the run's queue copy is drift the run cannot repair by identity: the removed or added entries' identities are unknown, so the handler may not guess (its best-effort tail removal and ignored additions are placeholders, never a repair).
3. The Owner repairs drift by resubmitting the canonical queue to the player (start slot = the canonical active slot, start position = the player's current position), which rebuilds the run's copy and mpv's playlist from the same slots. The Owner never treats the run's copy as authority.
4. A queue load plan's own rebuild (`cmd_submit_queue`'s `stop` plus no-play loads) moves mpv's count while the copy still holds the outgoing queue. That window runs under the transition gate (`pending_initial_playlist_layout`), so the run never reports its own surgery as drift. The gate outlives the arming command: mpv's count observations arrive on later run loop iterations, so the gate disarms only on a post-drain settle — the first observed count equal to the copy's length (`on_playlist_count_changed`), or the start item's PlaybackRestart (`on_playback_restart`). A disarm inside the arming command itself would leave the gate down exactly when the rebuild's own count events land.

## Why it matters

When the mapping corrupts, every later slot-addressed operation addresses the wrong mpv entry: removing a queued slot removes the wrong file from mpv, and playing a newly queued slot starts whatever file sits at the stale index — the user's deleted items kept playing after they removed rows and queued replacements (local-daemon.log 2026-11-08T19:33:53, the GUI queue fixes). Before the resubmit, the Owner relayed `QueueDesynced` to Clients and repaired nothing, so the divergence was permanent until a cold start.

## How the code maintains it today

- The count-drift handler shares the transition gate `on_playlist_pos_changed` reads, and `cmd_submit_queue` arms the gate for its rebuild. The gate disarms on the rebuild's settle, never inside the arming command: the settle is the first observed count equal to the copy's length, or the start item's PlaybackRestart.
- On real drift the run emits `PlayerEvent::QueueDesynced`; `handle_queue_desynced` resubmits the canonical queue through `submit_queue_slots_for_replace` inside a 2 s cooldown, so one drift burst resubmits once. The resubmit's own rebuild runs under the same gate and settles the same way, so it emits no further `QueueDesynced` and cannot loop.
- The resubmit's rebuild disarms nothing synchronously; if its loads all fail, the gate stays armed until the run ends (nothing can play), so no resubmit loop starts either.
- Guard tests: `playlist_count_drift_is_suppressed_during_a_pending_queue_rebuild` and `a_rebuilds_count_sequence_settles_the_gate_without_desync` (`crates/mbv-player/src/tests/session.rs` — the second drains the rebuild's recorded 0-then-M count sequence and fails if the gate is down when they arrive), and `a_queue_desync_resubmits_the_canonical_queue_to_the_player` (`crates/mbv-daemon/src/tests/loop.rs`, whose second desync inside the cooldown asserts the coalescing).

## Where it currently fails / how it could regress

- The drift handler's decrease arm still removes the copy's tail as a placeholder between the observation and the Owner's resubmit; if a future change stops resubmitting (or the cooldown swallows a second, unrelated drift inside its window), the placeholder semantics are wrong for a non-tail removal. Nothing type-enforces the resubmit.
- `QueueDesynced` during an in-flight transition resubmits over it; the submit's own identity seeding replaces the run, which is correct for a broken projection but loses any transition the Owner was waiting to settle.
