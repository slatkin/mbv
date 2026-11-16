# Design

## Context

See `proposal.md` for the motive. The behaviour is in the specs: `named-queues`,
`gui-queue-chooser`, and the deltas for `gui-client`, `unified-playback-queue`, and
`local-daemon-thin-client`.

The current state that shapes the approach:

- **One Owner queue.** The Owner keeps one `PlaybackQueue`, one `QueueSource`, and one
  `QueueLineage` (`ctx.owner.core.queue` and `.source`, mirrored in `SharedQueueState` in
  `crates/mbv-daemon/src/core.rs`). It persists them as one `StayAliveQueueState` in
  `stay_alive_queue_state.json` (`crates/mbv-config/src/state.rs`). The file stores the items and
  a position `cursor`, but no `QueueSlotId`s, so a restore mints new slot ids.
- **Queue commands address "the queue".** The `UnifiedQueue*` commands in
  `crates/mbv-ctrl/src/commands.rs` carry no queue identity. `answer_queue_op` in
  `crates/mbv-daemon/src/control.rs` answers an edit with the one queue snapshot.
- **The shown link follows the connection.** `QueueLinkResolution` in
  `src/app/state/queue_link.rs` picks `QueueLink::Peer` while a Direct remote control connection
  exists, else `QueueLink::Local`. While a peer is connected, `self.player` is the peer's proxy and
  `suspended_local` holds the home link. The shown queue and the player panel resolve to the same
  link (ADR 0034).
- **The GUI queue header** (`crates/mbv-gui/src/window/components/queue_header.rs`) paints the
  `GuiQueueSummary` section: the remaining count, the finish clock (`tail_secs`), the
  `GuiRightColumn` switch, and the filter. The rows carry `ends_after_secs` from
  `QueueEndOffsets` (`src/app/queue_summary_projection.rs`). The `t` chord is
  `KeyPolicyBinding::GuiRightColumnToggle` (`src/app/input/key_policy.rs`).
- **The GUI marks only a playing row.** `active_slot` in `crates/mbv-gui/src/bridge/frame/queue.rs`
  takes the marker from a row in the `NowPlaying` state.
- **In-flight work.** The `gui-pointer-target-interaction` change replaces GUI Panel focus with
  the pointer target and names the Queue's cursor row the *active row*. This change builds on it.

## Goals / Non-Goals

**Goals:**

- Several durable queues in the local Owner with one playback path: every playback invariant still
  concerns one queue only.
- No ctrl protocol version bump: one additive capability.
- The TUI and packaged `mbvd` keep their behaviour with no new control.

**Non-Goals:**

- Several queues in packaged `mbvd`. It does not advertise the capability.
- Several simultaneous peer connections. The GUI keeps today's one Direct remote control
  connection, and the chooser lists at most one peer. The chooser model allows more later.
- A change to the folded-queue line (`q`). It keeps its remaining count and finish clock, so
  `GuiQueueSummary.remaining`, `tail_secs`, and `QueueEndOffsets.tail` stay.
- A change to row removal of the playing slot, Queue handoff, or Take. "The local Owner queue" in
  `queue-handoff` reads as the current queue.
- Persistence of the GUI's shown entry across a GUI restart. The GUI starts on the current queue.

## Decisions

### D1. One current queue plus a resting store, not N live queues

The Owner keeps its existing Bound queue, run, transitions, consume, and lineage machinery for the
**current queue** without change. The other Named queues live in a new `NamedQueueStore` beside
it. A resting queue is a `PlaybackQueue` with a name, a `QueueSource`, a lineage, and a start slot,
and no run. Queue order is the creation order of a `Vec`; "current" is a stored queue identity, not
a position in the list.

*Alternative:* N fully live queues, each with its own transitions and lineage machinery. Rejected:
only one Playback run and one output exist per Owner, so live state for N queues adds invariants
(01, 13, 25, 26, 34) with no user-visible gain.

### D2. A swap makes a resting queue current through the existing replace path

To play resting queue B while A is current, the Owner (in one ctrl handler):

1. Stops A's run when it plays. A's start slot follows the stop rule (D4).
2. Moves A's `PlaybackQueue`, source, lineage, and start slot into the store.
3. Takes B out of the store and installs it as the Bound queue through the internal form of
   `UnifiedQueueReplace`, with B's slots (their ids kept, as ADR 0034 already requires of a
   replace) and `start_idx` at the requested slot or B's start slot. The replace mints a new
   lineage, as every replace does.

Every Client then sees one whole-queue snapshot for the new current queue. The TUI adopts it as it
adopts any replacement. A replace that a GUI Client sends for a resting queue (Library Play) runs
the same swap with the new items in place of B's slots.

### D3. Ctrl: one additive capability, `named-queues`

`CTRL_CAP_NAMED_QUEUES = "named-queues"`, advertised only by `DaemonRole::Local`. No version bump:
every addition is a new variant or a `#[serde(default)]` field. The golden fixtures in
`protocol/ctrl/v10/` gain one fixture per new variant and case, in place.

- **New commands:** `NamedQueueCreate { op }`, `NamedQueueRename { queue, name, op }`,
  `NamedQueueClose { queue, op }`, `NamedQueuePlay { queue, slot: Option<u64>, op }`, and
  `NamedQueueShow { queue }`. All of them are `OwnerOnly` in `requires_owner()`.
- **Defaulted field on the edit commands:** `queue: Option<NamedQueueId>` on `UnifiedQueueReplace`,
  `UnifiedQueueAppend`, `UnifiedQueueRemoveSlot(s)`, `UnifiedQueueMoveSlot`, `UnifiedQueueShuffle`,
  `UnifiedQueuePlaySlot`, `UnifiedQueueClearOp`, and `UnifiedQueueSourceUpdate`. `None`, or the
  current queue's id, takes today's path. A resting queue's id takes the store path (D5).
- **New events:** `NamedQueueList { queues: Vec<NamedQueueEntry { id, name, len }>, current }`,
  sent on attach and on every create, rename, close, and swap. `NamedQueueSnapshot { queue, data:
  UnifiedQueueStateData }` for a resting queue, sent to each connection whose `NamedQueueShow`
  names it, on show and after each change of that queue.
- **Defaulted fields on `UnifiedQueueStateData`:** `queue: Option<NamedQueueId>` and
  `start_slot: Option<u64>`. A Client without the capability ignores them.
- `NamedQueueId` is an Owner-minted `u64` newtype in `mbv-queue`, persisted, and never reused
  within one state file.

The wire identity changes, so the Owner mismatch gate prompts for an Owner restart after an
upgrade, as for any wire change (invariant 15).

*Alternative:* a Client receives every resting queue's snapshot. Rejected: queues carry full
`QueueItem`s, and a Client needs only the queue it shows.

### D4. The start slot is Owner state with one write site

The Owner holds `start_slot: Option<QueueSlotId>` for the current queue beside the observed active
slot, and the store holds one for each resting queue. One function updates the current queue's
start slot from run observations:

- An observed active slot sets it to that slot.
- A stop (user stop, a close, or a swap) keeps the last observed active slot.
- A natural completion that advances sets it to the next observed slot. A completion at the end of
  the queue (invariant 24) sets it to `None`.
- A removal of the start slot sets it to the slot now at its index, or `None`.

`PlaybackIntentAction::Play` with no items, and `NamedQueuePlay` with no slot, start the start
slot, or index 0 when it is `None`. The item's position comes from its *Resume source*, so the
start slot never stores a position. A new invariant (`docs/invariants/36-start-slot-follows-the-run.md`)
records the single write site.

### D5. Edits of a resting queue apply in the store

A resting queue's edits call the same `PlaybackQueue` methods (`append`, `remove_slot`, `move`,
`shuffle`, `clear`) with no mpv projection, bump its revision, and persist. The answer to the
sending Client is a `NamedQueueSnapshot` with the edit's `op`, so the "answered before the next
input" rule holds. An edit that names an unknown queue is rejected through the existing
command-rejection path.

### D6. Persistence: one new state file, slot ids kept

`named_queues_state.json` in the state directory (`mbv_config::named_queues_state_path()`) holds
`{ current, next_queue_id, queues: [{ id, name, source, lineage, start_slot, slots: [{ slot_id,
item }] }] }`. Items go through the same Service-position clearing that
`save_stay_alive_queue_state_at` applies today. The existing persistence worker
(`spawn_queue_persistence_worker`) writes the whole file atomically on each dirty tick and at
shutdown.

On restore, each `PlaybackQueue` is built with its saved slot ids, and `next_slot_id` starts above
the largest id (the existing adopt rule in `mbv-queue`). When the new file is absent and
`stay_alive_queue_state.json` exists, the Owner loads it as one current queue named `Default` with
fresh slot ids, writes the new file, and leaves the old file untouched.

### D7. GUI: the shown entry decides the link

The Client core gains `gui_shown_entry: ShownEntry { Local(NamedQueueId) | Peer }`, set only when
the GUI Client is enabled. `QueueLinkResolution` takes it into account: for the GUI, `Peer` maps
to `QueueLink::Peer` and `Local(_)` to `QueueLink::Local`, even while a peer is connected. The TUI
keeps the connection rule. Because the shown queue and the player panel still resolve to one link,
ADR 0034's single-link rule stays true; only the choice of link changes.

The local link holds two views: the existing `local_view` (the current queue, from
`UnifiedQueueState`) and a new `shown_resting: Option<(NamedQueueId, QueueView)>` (from
`NamedQueueSnapshot`). `displayed_queue()` returns the resting view when the shown local queue is
not current. Each local edit that the GUI sends carries the shown queue id.

A connect from the target chooser sets `Peer`. A disconnect or a lost link sets
`Local(current)`. Showing an entry clears the Queue filter and the marks and puts the active row on
the start slot (spec "Showing an entry resets the Queue active row and marks").

### D8. GUI chooser, rename, and close reuse existing overlays

- The Queue chooser is a new `GuiOverlay::QueueChooser { cursor, renaming: Option<String> }` in
  `src/app/gui_client_state/overlay.rs`, beside `TargetChooser`, projected as a new
  `GuiOverlayFrame::QueueChooser` and painted as a dropdown under the header.
- Rename text entry follows the Queue filter's pattern (`apply_gui_filter_key`): the core holds the
  draft. Enter sends `NamedQueueRename`; Esc drops the draft.
- The new-queue control sends `NamedQueueCreate`. When the `NamedQueueList` that answers its `op`
  arrives, the core shows the new queue and opens its rename draft with the given name.
- The close confirmation reuses the core confirm prompt that Queue handoff and Take use
  (`confirm_overlay.rs` in the GUI).
- Keys: `KeyPolicyBinding::GuiRightColumnToggle` becomes `GuiQueueChooserToggle` on `t`, and two new
  GUI-only literal bindings take `Ctrl+T` (`GuiNewQueue`) and `Ctrl+W` (`GuiCloseQueue`). They are
  gated on the Queue list box pointer target from `gui-pointer-target-interaction`. While the
  chooser overlay is open, the core takes the arrow keys, Enter, `r`, and Esc.

### D9. GUI header, rows, and footer

- **Header:** `QueueHeader` paints the chooser button (only with two or more entries), the filter
  control, the new control, and the close control. `GuiRightColumn`, the switch, its
  `rightColumn` region, `PanelIntent::ToggleQueueRightColumn`, and the matching `PlaybackRequest`
  are deleted. New pointer regions: `queueChooser`, `newQueue`, `closeQueue`.
- **Rows:** `ends_after_secs`, `apply_ends_at`, and `QueueEndOffsets.rows` are deleted. The row
  hover keeps the grip and puts the actions button at the row's end.
- **Start mark:** `GuiQueue` gains `start: Option<u64>` and `playing: bool`, projected from the
  snapshot's `start_slot` and the playback state. The row paints a filled mark when it is the
  start slot and the queue plays, and an outlined mark when it is the start slot and the queue is
  stopped or resting.
- **Footer:** `GuiQueueSummary` gains `total_secs: i64`, `total_partial: bool`,
  `marked_secs: Option<i64>`, and `marked_partial: bool`, summed in
  `queue_summary_projection.rs` from `QueueItem::runtime_ticks` over every slot of the shown queue
  and over the marked slots. A zero runtime counts as unknown and sets the partial flag. The
  filter does not narrow the sums. The footer drops the owner label and the liveness dot.

### D10. Docs

`CONTEXT.md` gains *Named queue*, *Current queue*, *Start slot*, and *Queue chooser*, and its
*Owner-held queue and source*, *Queue scope*, and *Tab selection* entries name the difference (the
Queue chooser is not a Tab and not the retired Queue scope). A new ADR 0041 amends ADR 0034: in the
GUI Client the shown entry, not the connection alone, picks the shown queue, and the connection
still decides which peers are listed. `docs/architecture/gui-client.md` replaces its queue header
and footer text.

### Dependencies

No new or upgraded dependency.

## Risks / Trade-offs

- [A swap stops and starts playback in one handler, and a slow resolve leaves a gap] → The swap uses
  the same replace-and-start path as a Library Play, which already settles through one transition
  (invariant 28).
- [Two Clients edit the same resting queue] → Edits are slot-addressed and answered with a
  snapshot, so the same rules as for the current queue apply. Each showing Client gets the new
  snapshot.
- [A Client shows a queue that another Client closes] → The Owner sends the new `NamedQueueList`.
  The GUI then shows `Local(current)`, and its pending edits are rejected as stale.
- [Rollback to an older build] → The old build reads `stay_alive_queue_state.json`, which is as it
  was at the upgrade. Queues made later are lost on rollback. This is accepted.
- [The wire identity change prompts an Owner restart] → Expected after every wire change
  (invariant 15).
- [The pending `gui-pointer-target-interaction` change edits the same GUI files] → This change
  starts after that change merges, and its tasks use the pointer-target gate and the active-row
  term.

## Migration Plan

The first start of the new Owner converts the single-queue file as D6 says. No user step is needed.
The TUI needs no migration. A packaged `mbvd` is unchanged.

## Open Questions

None.
