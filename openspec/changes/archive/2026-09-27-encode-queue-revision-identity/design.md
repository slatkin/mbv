# Design

## Context

See proposal.md (Why). Current facts that shape the approach:

- `QueueRevision(u64)` in `crates/mbv-queue/src/lib.rs` derives `Default`
  and exposes `from_raw`. It is private-bumped (`bump()` is `+1`) only when
  a projected row changes (`projected_row_mutation_matrix_tracks_revision_without_noop_bumps`
  pins the "no no-op bumps" half).
- The revision's only reader is the Queue projection fingerprint
  (`src/app/shell/queue.rs`, `queue_projection_changes`). Nothing orders
  revisions across queues. The wire field `UnifiedQueueStateData.revision`
  is read only by `PlayerTab::from_unified_state`.
- Callers that supply a revision: `from_slot_items` (owner:
  `reconciliation.rs` purge, `queue_setup.rs`, `queue_load.rs`; client:
  `player_tab.rs`), `from_queue_items_with_revision` (tests only), and
  `QueueRevision::default()` at those same owner sites.

## Goals / Non-Goals

**Goals:**

- Equal revisions imply the same queue state, by construction, for every
  `PlaybackQueue` in a process.
- Keep the existing "bump only on projected-row change" contract.

**Non-Goals:**

- Ordering stale snapshots by owner revision (invariant 05 §1). One
  connection delivers snapshots in order, so there is no observed failure.
  The residual is recorded in the rewritten invariant 01.
- Keeping slot ids unique across owner replacements, or putting lineage on
  `UnifiedQueue*Slot` commands. Owner replacement accepts Client-assigned
  slot ids, and fixing that is a protocol change. Recorded as a residual.
- Merging the two active-removal methods (`consume_slot` vs
  `remove_active_slot_confirmed`). Each has deliberate callers.

## Decisions

### D1. Revisions come from one process-wide counter

`QueueRevision::next()` (private to `mbv-queue`) takes
`static NEXT_REVISION: AtomicU64` with `fetch_add(1, Relaxed)`. Every
`PlaybackQueue` constructor, including a manual `Default` impl, calls it,
and `bump()` becomes `*self = Self::next()`. Within one queue, revisions
stay strictly increasing, so the existing `>` assertions still hold. Across
queues, no two values are ever equal.

*Alternative: keep the per-queue `+1` and make every owner replacement an
in-place `&mut self` transition (`replace_slots`, `retain`).* This covers
only the owner. Client `PlayerTab` swaps (connect, switch, daemon restart)
and adoption would still collide, and Rust cannot stop
`*queue = PlaybackQueue::…` through a `&mut`. It needs more edits for less
coverage.

*Alternative: add a queue-identity field (lineage or epoch) to the
fingerprint.* That spreads one fact across two fields that every reader
must remember to compare together, which is the hand-coordination this
umbrella removes.

### D2. No caller-supplied revisions

Delete `QueueRevision::from_raw`, the `Default` derive,
`from_queue_items_with_revision`, and the `revision` parameter of
`from_slot_items`. With no constructor that takes a revision, "rebuild with
the old revision" (the purge bug) no longer compiles. `raw()` stays for the
wire and for diagnostics.

### D3. The Client mints on adoption and ignores the wire number

`PlayerTab::from_unified_state` builds through `from_slot_items`, which now
mints a fresh revision. The owner's number means nothing in the Client's
process after an owner restart, and nothing else reads it. The wire field
is kept, not removed, to avoid a ctrl protocol change for no gain.

Cost: every adopted snapshot (`PlayerTab::set_unified_state`, reached from
`handle_unified_queue_updated`, and bootstrap) rebuilds the Queue rows. I
audited the `broadcast_queue_state` callers in `crates/mbv-daemon/src`. They
fire only on discrete events: ctrl queue edits, loads, and replacements,
Service reconciliation, enrichment that changed a slot, websocket play, and
the player events `TrackChanged`, `OutputStarted`, `TrackCompleted`, and
`Stopped`. None fires per progress tick; position flows through
`PlayerStatus`. Each of these events already changes the queue, active
flag, or transition, so the rebuild rate is essentially unchanged.

## Risks / Trade-offs

- [A test compares a revision to a literal, or builds one with `from_raw`]
  → compile errors find them. Rewrite them to relative (`>`, `!=`)
  assertions (`tests.rs:543` is the known case).
- [A future path broadcasts a snapshot per progress tick and turns every
  tick into a row rebuild] → the audit in D3 found none today. The
  "Unchanged queue still skips projection" scenario keeps the skip path
  pinned.
- [The global counter is shared state] → it is write-only, monotonic, and
  lock-free. Nothing reads it except to mint a value. Tests use relative
  assertions, so parallel tests are unaffected.
