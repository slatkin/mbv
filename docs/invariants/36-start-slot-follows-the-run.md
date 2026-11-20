# Invariant 36 — The start slot follows the run, and one write site moves it

**Scope:** `SharedQueueState::start_slot`, `SharedQueueState::note_start_slot`,
and `SharedQueueState::start_slot()` (`crates/mbv-daemon/src/core.rs`); the rule
`mbv_queue::start_slot_after` (`crates/mbv-queue/src/start_slot.rs`); the
observation sites in `event_loop/player_events.rs`,
`core/transition_dispatch.rs`, `emby_session/adopt.rs`, `run.rs`
(`record_completed_slot`, `apply_stopped_observation`),
`control/queue_edit.rs`, and `output/advance.rs` (the device advance and its
stale-advance correction); the resting-queue writer
`RestingNamedQueue::note_start_slot` and its removal, clear, and install call
sites in `crates/mbv-daemon/src/core/named_queues.rs` and
`control/named_queue.rs`; the swap sites `park_current` and `finish_swap`
there, which set a parked record's start slot at construction and write the
swap's `Installed` observation; the startup seed in `run.rs` and
`run/restore.rs`, which writes the restored start slots at construction; the
start-a-queue paths `play_from_start_slot` (`control/playback.rs`) and the
swap play in `control/named_queue.rs`; and the projection
`unified_queue_state_for_peer` (`control_queue.rs`).

## The invariant

1. A Named queue has at most one start slot: the slot a Play of that queue
   starts. The current queue's lives in `SharedQueueState.start_slot`; each
   resting queue's lives in its `NamedQueueStore` record.
2. One function writes the current queue's start slot:
   `SharedQueueState::note_start_slot`. The rule it applies is
   `mbv_queue::start_slot_after`, and no other code assigns the cell at run
   time; the startup seed sets the restored start slot at construction instead
   (`run.rs`, `run/restore.rs`). Every
   observation that can move the start slot is expressed as one
   `StartSlotObservation`, so the rule stays in one place and each caller names
   only what it saw.
3. The rule, one outcome per observation: an observed active slot becomes the
   start slot; a stop keeps the slot it stopped on; a natural completion makes
   the slot that followed the completed one the start slot, after any consume,
   and leaves the queue with none at the end of the queue; removing the start
   slot makes the slot now at its position the start slot, or none when no slot
   remains there; an install takes the start slot the installed queue carried: a
   close-install takes the stored start slot, or none when it had none, and a
   swap takes the slot the swap makes active — the requested slot, or the
   stored start slot while it still names a slot, else the first slot.
4. A start slot names a slot and stores no position. The item that plays from it
   takes its position from its Resume source, so a completion or a progress
   update can never leave a stale position in it.
5. A Play that names no items starts the current queue at its start slot, or at
   its first slot when it has none or the start slot names a slot the queue no
   longer holds. An empty queue changes nothing. The raw start slot reaches
   Clients as `UnifiedQueueStateData::start_slot`, which the projection drops
   when the peer does not receive that slot (the same gate `active_slot` uses).

## Why it matters

A Play of a resting or stopped queue has nowhere else to resume from — the
playhead is not a fact the Owner keeps while nothing plays, and the queue's
cursor is a presentation coordinate that every edit shifts. If a stop or a
completion failed to record where playback really was, a later Play would
resume at a row the user never reached. If a start slot were derived from the
queue's active marker instead of the run's observation, a cold-started queue, a
removed active slot, or a remote output's load would leave a Play starting at a
slot nothing played. Nothing in the type system forbids a second writer: the
cell is an `Arc<Mutex<Option<QueueSlotId>>>`, and an assignment next to any
handler compiles and passes every functional test.

## How the code maintains it today

- `SharedQueueState::note_start_slot` is the only writer of the current
  queue's start slot at run time, beside the startup seed. It reads the cell,
  applies `start_slot_after` to the queue as it stands after the observation,
  and stores the result.
- The active-slot observation has three run sources, one per output, each
  writing `StartSlotObservation::Active`: the local run's `TrackChanged`
  (`handle_track_changed`, which resolves the slot against the canonical queue
  before it writes), a Cast receiver's or Emby Session's loaded slot
  (`settle_remote_transition`), and the queue-end adoption on a Session
  (`commit_adoption`). The advance module adds two more, both for a slot its
  output already holds and neither of them a settled transition: a device
  advance's queued slot (`apply_device_advance`) and a stale-advance correction's
  corrected slot (`correct_stale_device_advance`, written after the completion
  record so the slot the device really plays beats its `Completed { next }`).
  Its funnel re-bases (`begin_advance`, `rebase_on_failed_step`) write the
  observed record alone: the completion, or the load the funnel dispatches next,
  moves the start slot there.
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
- A resting queue's start slot lives in its `RestingNamedQueue` record and is
  written by `RestingNamedQueue::note_start_slot`, the mirror of
  `SharedQueueState::note_start_slot`: it applies the same rule to the resting
  queue. A resting queue sees no run, so the only observation it records is
  `StartSlotRemoved`, on a slot removal and on a clear of its slots
  (`control/named_queue.rs`). A swap moves a start slot into and out of the
  store outside this writer: `park_current` sets the parked record's start
  slot at construction, from `SharedQueueState` after its `Stopped`
  observation, and the record the swap takes out leaves the store, so its
  stored start slot ends and the swap's `Installed` sets the new current
  queue's slot.
- Closing the current queue installs the first remaining resting queue as
  current without playing it. The install writes
  `StartSlotObservation::Installed { start_slot }` through
  `SharedQueueState::note_start_slot`, so the installed queue's stored start
  slot becomes the current queue's (`control/named_queue.rs`). A swap installs
  a resting queue the same way through `finish_swap`, with the slot the swap
  makes active as its `Installed` start slot, so that slot is the new current
  queue's while it plays.
- The startup seed sets the restored start slots at construction, never
  through the rule: `run/restore.rs` builds the current queue with its saved
  start slot as its active slot, so a restored stopped queue carries that slot
  as its `Active` slot in `PlaybackQueue`, and `run.rs` initializes
  `SharedQueueState.start_slot` from it; each resting queue's record takes its
  saved start slot while it still names a slot the queue holds.
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
  asserts the dispatched jump targets the start slot. The swap and restart
  tests in its submodules cover the newer writes: `swap.rs` holds
  `play_a_resting_queue_starts_its_start_slot`,
  `play_a_row_of_a_resting_queue_while_another_queue_plays`, and
  `replace_of_a_resting_queue_installs_the_new_items`, and `restore.rs` holds
  `a_restart_restores_every_named_queue`. Two more guard the install and the
  correction: `a_replace_of_the_current_queue_moves_the_start_slot_to_its_playhead`
  replaces the current queue with the incoming slot identities and asserts the
  installed playhead is the start slot, and
  `the_correction_makes_the_corrected_slot_the_start_slot`
  (`tests/emby_session_end/device_window.rs`, where the correction's own tests
  live) uncovers a Session's stale device move whose completion names a slot
  that output cannot play and asserts the corrected slot is the start slot.

## Where it currently fails / how it could regress

- A whole-queue replacement installs a new queue and writes its own
  `Installed` observation on the current-queue path of `handle_queue_replace`
  (`control/queue_setup.rs`): the playhead the replace installed is the start
  slot, as `finish_swap` writes it for a swap. A replacement that keeps the
  incoming slot identities (a Client's `UnifiedQueueReplace`, ADR 0034)
  therefore no longer leaves the start slot naming the position it used to
  hold. The fix settles where a replacement's start slot points; the spec does
  not say it yet.
- A new output or observation path that reports a slot its output holds must
  write `StartSlotObservation::Active` beside `note_observed_active_slot`
  (`output/advance.rs` `observe_active_slot` is the pair). A site that makes
  only the first call leaves the start slot behind until the next load settles,
  and a Play of the queue starts a slot nothing played. The two sites that write
  the observed record alone (`begin_advance`, `rebase_on_failed_step`) do so
  because their own record or the load they dispatch next moves the start slot,
  not because the pair is optional.
- The startup seed bypasses the rule (`gui-named-queues` tasks 5.5-5.7): it
  sets the current queue's and each resting queue's start slot at construction
  from the saved state file, so a save that wrote a wrong slot would restore
  it. The show is a reader: it forwards a resting queue's stored start slot
  and adds no rule of its own.
- The rule is a pure function over a queue and one observation, so a caller that
  passes the wrong queue state — the queue before a removal instead of after,
  or a queue the observation does not belong to — gets a plausible but wrong
  slot with no type error.
- Storing a position in the start slot would be the tempting shortcut for a
  resume: it drifts from the slot's progress after every completion and breaks
  item 4.
