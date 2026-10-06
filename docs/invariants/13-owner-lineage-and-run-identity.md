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
3. **A cold owner never receives a bare slot jump.** A `JumpTo` reaches only a
   live Playback run. A run spawned after the jump reports its own
   `TrackChanged`, which carries the run identity — never the request identity
   of the jump — so an accepted transition dispatched as a bare `JumpTo` to an
   owner with no live run can never settle, lingers until its expire deadline,
   and wedges every later jump behind it. A dispatch that finds no live run
   must therefore submit the queue at the target slot (cold start) and reset
   the transition instead.

The other former properties are enforced structurally: `QueueView` is
adopt-only and cannot seed or persist an editable Client queue (former property
1), while `queue_op` adopts owner answers rather than merging local fragments
(former property 4).

## Why it matters

A delayed source update must not rename a queue another Client replaced, and a
late completion from a retired playback run must not consume or mark a slot in
the replacement queue. These remain process rules across the control protocol.

The cold-jump property (3) is what makes the first slot jump after an owner
restart work at all: with the bare `JumpTo` form, Enter on a media item looked
like a total no-op while the owner queue silently accumulated appends behind
the wedged transition (queue-owner-process #857).

## How the code maintains it today

- Owner-minted `QueueLineage` is carried by accepted snapshots and checked by
  source-update handlers.
- Playback-run identity is assigned by the owner and checked before applying
  stale observations.
- `QueueView` exposes adoption, not queue mutation; `queue_op` waits for the
  owner's answer and adopts it before subsequent input.
- Every slot-jump dispatch routes through `send_jump_or_cold_start`
  (`crates/mbv-daemon/src/core.rs`): a live run takes the `JumpTo`; a dead run
  gets the shared cold-start whole-queue submission at the target slot plus a
  transition/origin reset, and the queue's active marker follows the requested
  slot so the following broadcast resolves it as active.

## Where it currently fails

No known violation. Two known residuals (observed in review, not regressions):

- On a plain autostart play, the owner's Queue source stays at its previous
  value until the next idle load or clear — a property of the adoption model,
  since submission does not carry a source update.
- Property 3 is not enforced by types: `Player::send_command`'s boolean is the
  only signal that no live run took the jump, so a future call site that
  dispatches a jump directly, bypassing `send_jump_or_cold_start`, would
  reintroduce the wedge. The behavior is pinned by
  `crates/mbv-daemon/src/tests/queue_ops/queue_play_slot.rs` and the cold
  PlaySlot half of the ABS admission broadcast test.
