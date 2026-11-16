# Invariant 36 — The start slot follows the run, and one write site moves it

**Scope:** `SharedQueueState::start_slot`, `SharedQueueState::note_start_slot`,
and `SharedQueueState::start_slot()` (`crates/mbv-daemon/src/core.rs`); the rule
`mbv_queue::start_slot_after` (`crates/mbv-queue/src/start_slot.rs`); the
observation sites in `event_loop/player_events.rs`,
`core/transition_dispatch.rs`, `emby_session/adopt.rs`, `run.rs`
(`record_completed_slot`, `apply_stopped_observation`), and
`control/queue_edit.rs`; the start-a-queue path `play_from_start_slot`
(`control/playback.rs`); and the projection
`unified_queue_state_for_peer` (`control_queue.rs`).

## The invariant

1. A Named queue has at most one start slot: the slot a Play of that queue
   starts. The current queue's lives in `SharedQueueState.start_slot`; each
   resting queue's lives in its `NamedQueueStore` record.
2. One function writes the current queue's start slot:
   `SharedQueueState::note_start_slot`. The rule it applies is
   `mbv_queue::start_slot_after`, and no other code assigns the cell. Every
   observation that can move the start slot is expressed as one
   `StartSlotObservation`, so the rule stays in one place and each caller names
   only what it saw.
3. The rule, one outcome per observation: an observed active slot becomes the
   start slot; a stop keeps the slot it stopped on; a natural completion makes
   the slot that followed the completed one the start slot, after any consume,
   and leaves the queue with none at the end of the queue; removing the start
   slot makes the slot now at its position the start slot, or none when no slot
   remains there.
4. A start slot names a slot and stores no position. The item that plays from it
   takes its position from its Resume source, so a completion or a progress
   update can never leave a stale position in it.
5. A Play that names no items starts the current queue at its start slot, or at
   its first slot when it has none or the start slot names a slot the queue no
   longer holds. An empty queue changes nothing. The raw start slot reaches
   Clients as `UnifiedQueueStateData::start_slot`, which the projection drops
   when the peer does not receive that slot (the same gate `active_slot` uses).

## Why it matters

The GUI Client paints the Queue's start mark from this slot: a filled mark while
the queue plays, an outlined one while it rests or is stopped. A Play of a
resting or stopped queue has nowhere else to resume from — the playhead is not a
fact the Owner keeps while nothing plays, and the queue's cursor is a
presentation coordinate that every edit shifts. If a stop or a completion failed
to record where playback really was, the mark would paint a row the user never
reached and a later Play would resume there. If a start slot were derived from
the queue's active marker instead of the run's observation, a cold-started
queue, a removed active slot, or a remote output's load would move the mark to
a slot nothing played. Nothing in the type system forbids a second writer: the
cell is an `Arc<Mutex<Option<QueueSlotId>>>`, and an assignment next to any
handler compiles and passes every functional test.

## How the code maintains it today

- `SharedQueueState::note_start_slot` is the only writer. It reads the cell,
  applies `start_slot_after` to the queue as it stands after the observation,
  and stores the result.
- The active-slot observation has three sources, one per output, each writing
  `StartSlotObservation::Active`: the local run's `TrackChanged`
  (`handle_track_changed`, which resolves the slot against the canonical queue
  before it writes), a Cast receiver's or Emby Session's loaded slot
  (`settle_remote_transition`), and the queue-end adoption on a Session
  (`commit_adoption`).
- The completion funnel `record_completed_slot` writes
  `StartSlotObservation::Completed { next }`. It reads the completed slot's
  index before the consume rules run, then takes the slot after it, or the slot
  now at its position when a consume removed it, so "after any consume" is one
  lookup instead of an ordering rule spread over two callers. Local runs, Cast
  natural ends, and Session ends all record through it.
- `apply_stopped_observation` writes `StartSlotObservation::Stopped`, which by
  the rule keeps the stored slot. It writes it anyway so the stop path states
  its own outcome instead of relying on the reader to know that no write means
  "keep".
- The removal sites in `control/queue_edit.rs` write
  `StartSlotObservation::StartSlotRemoved { index }` when the removed slot was
  the start slot, before they broadcast, so the snapshot a peer receives
  already carries the new slot. The batch removal reads each slot's position
  before removing it and applies the observation per removal; `QueueClear`
  applies it once, with the start slot's position, against the emptied queue.
- `play_from_start_slot` reads the start slot, validates it against the queue,
  falls back to the first slot, and dispatches one slot-jump transition under
  the Play intent's own request identity. The run's observation of that
  transition settles the intent, so a Play with no items cannot sit unresolved
  and coalesce the user's next press (invariant 28).
- `unified_queue_state_for_peer` maps the slot to the peer's `start_slot` and
  drops it when the peer's projection does not carry that slot.
- Guard tests (`crates/mbv-daemon/src/tests/named_queues.rs`):
  `a_stop_keeps_the_start_slot` stops on the third slot and asserts peers
  receive it; `a_completion_moves_the_start_slot` completes the third slot of
  four and asserts the fourth becomes the start slot; and
  `a_play_with_no_items_starts_the_start_slot` drives the ctrl Play path and
  asserts the dispatched jump targets the start slot.

## Where it currently fails / how it could regress

- A whole-queue replacement installs a new queue and leaves the start slot
  alone. `PlaybackQueue::from_queue_items` mints new slot identities, so the old
  start slot names nothing and a Play falls back to the first slot — but a
  replacement that keeps the incoming slot identities (a Client's
  `UnifiedQueueReplace`, ADR 0034) can leave the start slot naming the position
  it used to hold. The spec does not decide where a replacement's start slot
  points; a change that does should write it through `note_start_slot` with an
  explicit observation rather than adding a second writer.
- A new output or observation path that loads a slot without writing
  `StartSlotObservation::Active` leaves the start slot behind until the next
  `TrackChanged`. `note_observed_active_slot` and `note_start_slot` are two
  calls at each site today; a site that makes only the first is silently
  inconsistent.
- Resting queues keep a start slot in the store but nothing tracks it yet
  (`gui-named-queues` tasks 5.3-5.7): a resting queue's start slot is set only
  by the stop-or-park rule that has not landed.
- The rule is a pure function over a queue and one observation, so a caller that
  passes the wrong queue state — the queue before a removal instead of after,
  or a queue the observation does not belong to — gets a plausible but wrong
  slot with no type error.
- Storing a position in the start slot would be the tempting shortcut for a
  resume: it drifts from the slot's progress after every completion and breaks
  item 4.
