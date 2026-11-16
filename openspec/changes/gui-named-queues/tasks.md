# Tasks

This change builds on `gui-pointer-target-interaction` (design, Context). Start it on a base that
holds that change.

## 1. Domain records

- [ ] 1.1 Add *Named queue*, *Current queue*, *Start slot*, and *Queue chooser* to `CONTEXT.md`
  under `## Queue`, each with an `_Avoid_` line, and add one sentence each to *Owner-held queue and
  source*, *Queue scope*, and *Tab selection* that tells them apart (design D10). Verify that
  `rg -n "Named queue|Current queue|Start slot|Queue chooser" CONTEXT.md` finds the four entries.
- [ ] 1.2 Write `docs/adr/0041-gui-shows-one-entry-per-queue.md` per `docs/adr/README.md`: the GUI's
  shown entry picks the shown queue and the connection decides which peers are listed; it amends
  ADR 0034 and names the rejected "N live queues" option (design D1, D7). Add an amendment line
  to ADR 0034 that names ADR 0041. Verify that both files render and that the ADR index lists 0041.

## 2. Queue model (`crates/mbv-queue`)

- [ ] 2.1 Add the `NamedQueueId` newtype (serde, `JsonSchema`, `raw`/`from_raw`) and a
  `PlaybackQueue` constructor that keeps given slot ids and sets `next_slot_id` above the largest
  (design D3, D6). Verify with one test in `crates/mbv-queue/tests/queue.rs` that a queue built
  from saved ids keeps them and mints the next id above the largest.
- [ ] 2.2 Add the start-slot rule as a pure function over a queue and an observation (active slot,
  stop, completion with next slot, end of queue, removal of the start slot) (design D4). Verify
  with one `#[case]` table test that has one row per outcome in the `named-queues` spec "Each
  queue has a start slot".

## 3. State file (`crates/mbv-config`)

- [ ] 3.1 Add `named_queues_state_path()` and the `NamedQueuesState` record with atomic save and
  load, clearing Service positions as `save_stay_alive_queue_state_at` does (design D6). Verify
  with a round-trip test of two queues with names, sources, slot ids, and start slots.
- [ ] 3.2 Add the migration load: with no new file and an existing single-queue file, return one
  current queue named `Default` and leave the old file unchanged (spec "An earlier single-queue
  state loads"). Verify with one test over a temporary directory.

## 4. Ctrl vocabulary (`crates/mbv-ctrl`)

- [ ] 4.1 Add `CTRL_CAP_NAMED_QUEUES`, the `NamedQueueCreate`, `NamedQueueRename`,
  `NamedQueueClose`, `NamedQueuePlay`, and `NamedQueueShow` commands as `OwnerOnly`, and the
  defaulted `queue` field on the edit commands that design D3 lists. Verify that `cargo check
  -p mbv-ctrl` passes and that a payload without `queue` still decodes (one decode test).
- [ ] 4.2 Add the `NamedQueueList` and `NamedQueueSnapshot` events and the defaulted `queue` and
  `start_slot` fields on `UnifiedQueueStateData` (design D3). Verify that a v10 payload without the
  new fields decodes.
- [ ] 4.3 Add one golden fixture per new variant and case under `protocol/ctrl/v10/`, regenerate
  the schemas, and update the wire identity. Verify with the existing ctrl conformance tests
  (`cargo nextest run -p mbv-ctrl`).

## 5. Owner (`crates/mbv-daemon`)

- [ ] 5.1 Add `NamedQueueStore` (ordered resting queues, current id, next queue id) to the Owner
  core and advertise `named-queues` only for `DaemonRole::Local` (design D1, D3). Verify with one
  test that a packaged-role hello carries no `named-queues` capability.
- [ ] 5.2 Track the current queue's start slot with the 2.2 rule at one write site, include it in
  `UnifiedQueueStateData.start_slot`, and make a Play with no items start the start slot or index
  0 (design D4). Write `docs/invariants/36-start-slot-follows-the-run.md`. Verify with tests for
  "A stop keeps the slot" and "A completion moves the start slot".
- [ ] 5.3 Route queue edits by `queue`: `None` or the current id takes today's path; a resting id
  edits the store queue, persists, and answers the op with `NamedQueueSnapshot`; an unknown id is
  rejected (design D5). Verify with tests for "Append to a resting queue" and "An edit names a
  closed queue".
- [ ] 5.4 Handle `NamedQueueCreate` (`Queue N` naming), `NamedQueueRename` (trim, refuse empty),
  and `NamedQueueClose` (refuse the last queue; stop and delete the playing current queue and make
  the first remaining queue current without play), each broadcasting `NamedQueueList`. Verify with
  tests for "A new queue is made", "An empty name is refused", "Close the playing queue", and "The
  last queue does not close".
- [ ] 5.5 Handle `NamedQueuePlay` and a `UnifiedQueueReplace` or `UnifiedQueuePlaySlot` that names
  a resting queue with the swap of design D2 (stop, park the current queue, install the resting
  queue through the replace path with its slot ids kept). Verify with tests for "Play a resting
  queue" and "Play a row of a resting queue while another queue plays", and one test that a
  Client without `named-queues` receives the new current queue as one whole-queue snapshot.
- [ ] 5.6 Handle `NamedQueueShow` per connection: send that queue's snapshot on show and after
  each change of it, and drop the subscription on close and on disconnect (design D3). Verify with
  one test that only the subscribed connection receives the resting snapshot.
- [ ] 5.7 Persist the store and the current queue through `spawn_queue_persistence_worker` into
  the 3.1 file, and restore it (with the 3.2 migration) at startup before the first Client is
  served (design D6). Verify with a restart test for "Restart restores every named queue".

## 6. Client core: shown entry (`src/app`)

- [ ] 6.1 Adopt `NamedQueueList` and `NamedQueueSnapshot` into Client core state (the queue list,
  the current id, and `shown_resting`), and send `NamedQueueShow` when the GUI shows a resting
  queue (design D7). Verify with a core test that a resting snapshot fills the shown view and the
  current queue view does not change.
- [ ] 6.2 Add `gui_shown_entry` and make `QueueLinkResolution` follow it for the GUI Client only,
  so `displayed_queue()`, the player panel, and edits resolve to the shown entry's link; the TUI
  keeps the connection rule (design D7). Verify with tests for "A GUI Client connected to a peer
  shows a local queue" and one test that the TUI link does not change.
- [ ] 6.3 Make every GUI queue edit, enqueue, play-next, replace, and row play to the local Owner
  carry the shown queue id, and make Play with nothing playing send `NamedQueuePlay` for the shown
  queue (spec "Edits and enqueue reach the shown queue", "Play with nothing playing starts the
  shown queue"). Verify with tests for "Enqueue to a resting queue" and "Play a stopped queue".
- [ ] 6.4 Set the shown entry on connect (`Peer`), on disconnect and lost link (`Local(current)`),
  and on a close of the shown queue; on each change clear the Queue filter and the marks and put
  the active row on the start slot (spec "A connect adds a peer entry", "Showing an entry resets
  the Queue active row and marks"). Verify with tests for "The connection is lost" and "Switch
  with marks".

## 7. Client core: GUI queue controls (`src/app`)

- [ ] 7.1 Delete the right column: `GuiRightColumn`, its state, `PanelIntent::ToggleQueueRightColumn`,
  its `PlaybackRequest`, `QueueEndOffsets.rows`, and their tests; keep `remaining`, `tail_secs`,
  and `QueueEndOffsets.tail` for the folded line (design D9, Non-Goals). Verify with `cargo check
  --workspace` and `rg -n "RightColumn|ends_after" src crates` finding nothing.
- [ ] 7.2 Add the `GuiOverlay::QueueChooser` overlay with its cursor, Enter to show, `r` to open a
  rename draft, Enter to send `NamedQueueRename`, and Esc; rebind `t` as `GuiQueueChooserToggle`
  and add `Ctrl+T` (`GuiNewQueue`) and `Ctrl+W` (`GuiCloseQueue`) as GUI-only literals gated on the
  Queue list box pointer target (design D8). Verify with core key tests for "Open the chooser by
  key", one rename by key, and one test that the TUI router does not bind the three chords.
- [ ] 7.3 Make the new-queue control send `NamedQueueCreate` and, on the answering
  `NamedQueueList`, show the new queue with its rename draft open (spec "Make a queue"). Verify with
  one core test.
- [ ] 7.4 Make the close control act on the shown entry: close a resting queue at once, ask
  through the core confirm prompt before closing the playing current queue, disconnect a peer,
  and do nothing while the shown queue is the only local queue (spec "The close control closes the
  shown entry"). Verify with tests for "Close the playing queue", "Cancel the close", and "Close a
  peer entry".
- [ ] 7.5 Add `total_secs`, `total_partial`, `marked_secs`, and `marked_partial` to
  `GuiQueueSummary` from `runtime_ticks` over the whole shown queue and its marks, and add `start`
  and `playing` to the queue projection (design D9). Verify with one `#[case]` table test for the
  footer scenarios ("The footer totals the marked rows", "No row is marked", "A runtime is unknown",
  "The filter narrows the rows").

## 8. GUI window (`crates/mbv-gui`)

- [ ] 8.1 Carry the new summary, queue, list, and overlay facts through the bridge frame sections
  (`bridge/frame/queue.rs`, `bridge/frame/overlay.rs`) and delete `ends_after_secs` and
  `apply_ends_at`. Verify with the bridge frame tests in `crates/mbv-gui/tests/bridge/frame.rs`,
  updated for the removed fields and one new start-slot projection test.
- [ ] 8.2 Rewrite `QueueHeader` to paint the chooser button (only with two or more entries), the
  filter control, the new control, and the close control (disabled on the only local queue), each
  reporting its pointer region (`queueChooser`, `newQueue`, `closeQueue`, `filter`). Verify with
  the queue panel header tests in `queue_panel/tests/header.rs` for "One queue and no peer" and
  "Several entries".
- [ ] 8.3 Paint the Queue chooser dropdown from the overlay frame: local queues in order, then the
  peer, with the shown, current, and peer marks, and the inline rename entry. Verify with one
  component test that a press on an entry reports the entry's region.
- [ ] 8.4 Paint rows with no right column, the actions button at the row's end on hover, and the
  filled or outlined start mark from `start` and `playing`. Verify with the row tests for "A stopped
  queue marks its start slot" and "A row is hovered".
- [ ] 8.5 Paint the footer totals at the right of `QueueFooter`, move the dirty mark beside the
  source text, and remove the owner label and liveness dot. Verify with one footer component test
  for "The footer totals the marked rows".
- [ ] 8.6 Update `docs/architecture/gui-client.md` for the queue header, the Queue chooser, the
  rows, the start mark, and the footer, and remove the right-column and owner-label text. Verify
  that `rg -n -i "Ends at|right column|owner label" docs/architecture/gui-client.md` finds no stale
  text.

## 9. Integration checks

- [ ] 9.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo nextest run --workspace`, and fix every failure.
- [ ] 9.2 Run `openspec validate gui-named-queues --strict` and `make check-code-file-lines`, and
  split any file over 800 lines along its responsibility seam.

## Workflow follow-up

- When the user approves the PR for merge, sync the deltas into `openspec/specs/` and archive the
  change as the last commit before the merge.
