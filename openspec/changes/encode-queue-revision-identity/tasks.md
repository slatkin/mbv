# Tasks

One implementer session covers all groups, in order. Gate for every group:
`cargo check --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo nextest run -p <crates touched>` all pass. Run `cargo fmt` after
edits. If `encode-playback-lifecycle-types` has landed, rebase first; it
touches `control/queue_setup.rs` and `control/queue_load.rs`.

## 1. Pin the symptom

- [ ] 1.1 In the `#[cfg(test)] mod tests` of `src/app/shell/queue.rs`, add a regression test (#836). Contract: "Queue rows follow a re-adopted owner snapshot even when its wire revision equals the previous one." Layer: the shell Queue projection. Build a `Model` whose `app.player_tab` comes from `PlayerTab::from_unified_state` with two slots, revision `7`, and playback inactive, then call `sync_queue()`. Then call `model.app.player_tab.set_unified_state(..)` with a snapshot that has revision `7` and **one** slot, and call `sync_queue()`. Assert that the Queue component now holds one row (or that `last_queue_projection` changed, if rows are not observable from the test). Verify: the test **fails** on current code. If it passes, stop and report back: the proposal's "Fix" bullet is then wrong.

## 2. Mint revisions in `mbv-queue`

- [ ] 2.1 In `crates/mbv-queue/src/lib.rs` (design D1/D2): add a private `static NEXT_REVISION: AtomicU64` and a private `QueueRevision::next()` (`fetch_add(1, Relaxed)`). Change `bump()` to `*self = Self::next()`. Remove `Default` from `QueueRevision`'s derive list and delete `QueueRevision::from_raw` (the one on `QueueRevision`, **not** `QueueSlotId::from_raw` above it). Delete `from_queue_items_with_revision`, and have `from_queue_items` build its queue with `QueueRevision::next()`. Drop the `revision` parameter from `from_slot_items` (mint with `next()`). Give `PlaybackQueue` a `Default` that mints (a manual impl if it is currently derived or delegated). Verify: `cargo check -p mbv-queue` passes. Callers in other crates will fail until task 3.
- [ ] 2.2 In `crates/mbv-queue/src/tests.rs`: rewrite `projected_row_mutation_matrix_tracks_revision_without_noop_bumps` to build with `from_queue_items` (it keeps only relative assertions), and fix the `from_slot_items` call near line 135. Add one test. Contract: "two independently constructed queues never share a revision, and a clone shares its source's". Layer: `QueueRevision` minting. Verify: `cargo nextest run -p mbv-queue` passes.

## 3. Callers

- [ ] 3.1 Drop the revision argument at `crates/mbv-daemon/src/reconciliation.rs` (`purge_queue`; also delete its `let revision = queue.revision();`), `crates/mbv-daemon/src/control/queue_setup.rs`, and `crates/mbv-daemon/src/control/queue_load.rs`. Verify: `cargo nextest run -p mbv-daemon` passes.
- [ ] 3.2 Drop the `QueueRevision::from_raw(state.revision)` argument in `PlayerTab::from_unified_state` (`src/app/state/player_tab.rs`, design D3). Fix any remaining compile errors that the compiler lists (tests building queues with explicit revisions) by removing the argument. Verify: the task 1.1 test now passes, and `cargo nextest run -p mbv` passes.

## 4. Docs

- [ ] 4.1 Delete `docs/invariants/05-queue-revision-unread.md`. Rewrite `docs/invariants/01-slot-identity-active-revision.md` to hold only what types don't enforce (design Non-Goals):
  - slot ids are unique per `PlaybackQueue` value, owner replacement accepts Client-assigned ids, and `UnifiedQueue*Slot` commands carry no lineage;
  - the two active-removal semantics;
  - Clients apply snapshots in arrival order and rely on one connection being ordered.

  Point to `QueueRevision` minting for the revision rule. Remove the stale claims (`slots_mut()`, non-bumping active/progress changes, "revision unread"). Verify: `rg -l "05-queue-revision-unread|invariants/05|Invariant 5" docs openspec/specs AGENTS.md CONTEXT.md` is empty (fix any references it finds).
- [ ] 4.2 Tick invariants 01 and 05 on #810 with a pointer to #836 (`gh issue edit 810` body). Final gate: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace` pass.
