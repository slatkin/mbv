# Proposal

## Why

The Queue projection skips rebuilding rows when its fingerprint
`(scope, revision, playback.active, active target, progress bucket)` is
unchanged (#675, #803). The spec already requires that "queue replacement
SHALL invalidate previously issued revisions", but nothing enforces it. A
`QueueRevision` is a plain counter that starts at 0 in every new
`PlaybackQueue` value, and callers can pass any value they like. Many paths
replace the whole value:

- Owner: `purge_queue` (`crates/mbv-daemon/src/reconciliation.rs`) rebuilds
  the queue with its **old** revision. The replacement paths in
  `control/queue_setup.rs`, `control/queue_load.rs`, `control/playback.rs`,
  and `ws.rs` assign fresh queues that restart at revision 0.
- Client: `PlayerTab::from_unified_state` adopts the owner's raw revision.
  Connect, switch, and daemon restart swap whole `PlayerTab` values.

So two different queue states can carry the same revision. When the other
fingerprint fields also match (the idle case, with nothing playing), the
Queue keeps painting the previous queue's rows. Examples: a Service purge
under Stay-alive, or two consecutive idle playlist loads. Issue #836, child
of #810 (invariants 01 and 05).

Both invariant docs are also out of date. `slots_mut()` is gone, active,
progress, and item changes now bump the revision, and the revision now has
a reader.

## What Changes

- **A revision identifies one queue state.** `QueueRevision` values are
  minted from one process-wide monotonic counter. Construction and every
  bump take the next value. No caller supplies a revision, so two different
  queue states in one process can never compare equal. A clone shares its
  source's revision, which is correct because the content is the same.
- **BREAKING (internal API):** `QueueRevision::from_raw`,
  `QueueRevision`'s `Default`, `PlaybackQueue::from_queue_items_with_revision`,
  and the `revision` parameter of `PlaybackQueue::from_slot_items` are
  removed. Every constructor mints a fresh revision.
- **The Client mints its own revision when it adopts a snapshot.**
  `PlayerTab::from_unified_state` no longer copies the wire revision. The
  wire field `UnifiedQueueStateData.revision` stays, as owner-local
  diagnostic data; no ctrl protocol change.
- **Fix:** the Queue repaints after an owner purge, after consecutive idle
  loads, and after a Client re-adopts an owner snapshot (reconnect, switch,
  daemon restart). Task 1.1 confirms the stale-rows symptom with a failing
  shell test first.
- **Docs.** Delete `docs/invariants/05-queue-revision-unread.md`. Rewrite
  `docs/invariants/01-slot-identity-active-revision.md` to cover only what
  types cannot reach:
  - slot ids are unique per queue value, and owner replacement accepts
    Client-assigned ids, while `UnifiedQueue*Slot` commands carry no
    lineage;
  - the two ways of removing the active slot (auto-consume vs explicit
    removal);
  - a Client applies snapshots in arrival order, relying on each connection
    delivering them in order.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `queue-canonical-list`: adds a requirement that a queue revision is never
  reused for a different queue state. This covers owner replacement, owner
  purge, and Client adoption of an owner snapshot, so the Queue rows always
  follow the queue's current contents.

## Impact

- `crates/mbv-queue` (`QueueRevision` minting; `PlaybackQueue` constructors
  lose their revision parameter).
- `crates/mbv-daemon` (`reconciliation.rs` `purge_queue`;
  `control/queue_setup.rs`, `control/queue_load.rs` constructor calls).
- `src/app/state/player_tab.rs` (`from_unified_state`).
- `src/app/shell/queue.rs` (no logic change; gains the regression test).
- `docs/invariants/01`, `docs/invariants/05`.
- Sequencing: `encode-playback-lifecycle-types` (#824) also edits
  `control/queue_setup.rs` and `control/queue_load.rs`, near these call
  sites. Land this change after #824, or expect trivial one-line conflicts.
