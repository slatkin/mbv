# Invariant 1 — Slot identity is stable; position is not

**Scope:** `PlaybackQueue` (`crates/mbv-queue/src/lib.rs`), owner/client queue snapshots, and `UnifiedQueue*Slot` control commands.

## The invariant

1. Slot ids are unique within one `PlaybackQueue` value. Reorder, refresh-merge, prune, and consume move existing slots without renumbering them. They are not guaranteed unique across owner replacements: an owner replacement may accept Client-assigned ids, and `UnifiedQueue*Slot` commands carry no queue lineage with which to distinguish those ids.
2. `active_slot_id` is always `None` or resolves to a live slot.
3. `PlayerTab::default` constructs local and remote queues independently, each allocating slot ids from 1, so ids collide across scopes; never compare slot ids across scopes.
4. Active-slot removal has two deliberate semantics. `consume_slot` auto-consumes and re-anchors active playback; `remove_active_slot_confirmed` explicitly removes the active slot and clears the active id. Callers must choose according to their intent.
5. A Client applies queue snapshots in arrival order. This relies on each connection delivering its snapshots in order; the Client does not use owner revisions to order stale snapshots.

## Queue revisions

`QueueRevision` values are minted in `crates/mbv-queue` from an owner-scoped `QueueRevisionMint`: each queue owner (the daemon Player, each client `PlayerTab`) holds one mint, every constructor and `bump()` mints from it, and tab replacements carry the old mint forward so a chain never repeats a revision. See `QueueRevisionMint` and `PlaybackQueue` in `crates/mbv-queue/src/lib.rs` for the implementation.

## Why it matters

Slot ids identify queued items independently of their current positions. Reorders and removals can change indices while preserving slot identity, but ids alone do not establish identity across queue replacements or snapshots. The command protocol carries slot ids without lineage, so callers must respect the queue value and connection boundaries described above.

The two active-removal operations also represent distinct playback intents: automatic consumption preserves playback continuity by selecting a successor, while explicit active removal clears the active selection.

## How the code maintains it today

- `PlaybackQueue` allocates slot ids for its own value. Snapshot/replacement paths may preserve ids assigned by the Client. Queue edits move slots rather than renumbering them; removals maintain or clear the active id.
- Auto-consume callers use `consume_slot`; explicit confirmed removal callers use `remove_active_slot_confirmed`.
- Snapshot application follows delivery order on each connection.
- `QueueRevision` construction and projected-row bumping are handled by `crates/mbv-queue` as described above.
