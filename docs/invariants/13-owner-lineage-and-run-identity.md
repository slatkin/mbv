# Invariant 13 — Owner lineage and run identity reject stale queue updates

## The invariant

Two process-level properties remain important because types do not enforce
them:

1. **Owner-minted lineage guards source-only updates and playlist mutations.**
   A `QueueLineage` minted by the Owner process accompanies each accepted
   queue state. A source-only update applies only to the lineage held when it
   was requested; Clients echo owner-minted values and cannot self-authorize.
2. **Playback run identity filters stale observations.** Each playback run
   carries an owner-assigned identity. Stop, completion, and track observations
   from a run retired by queue replacement are filtered so late reports cannot
   affect slots in the replacement queue.

The other former properties are enforced structurally: `QueueView` is
adopt-only and cannot seed or persist an editable Client queue (former property
1), while `queue_op` adopts owner answers rather than merging local fragments
(former property 4).

## Why it matters

A delayed source update must not rename a queue another Client replaced, and a
late completion from a retired playback run must not consume or mark a slot in
the replacement queue. These remain process rules across the control protocol.

## How the code maintains it today

- Owner-minted `QueueLineage` is carried by accepted snapshots and checked by
  source-update handlers.
- Playback-run identity is assigned by the owner and checked before applying
  stale observations.
- `QueueView` exposes adoption, not queue mutation; `queue_op` waits for the
  owner's answer and adopts it before subsequent input.
