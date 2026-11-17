# Proposal

## Why

The GUI queue header shows facts that do not help the user (`Up next · N`, the queue finish
clock, and a Length / Ends at switch), and the GUI can show only one queue. Users want several
durable queues, as a music player has several playlists, and a clear place where a connected
mbvd's queue lives. Today a connect replaces the local queue in the panel, and nothing shows that
the local queue still exists.

## What Changes

- **Named queues in the Owner process.** The Owner holds any number of durable, named queues. One
  of them is the *current queue*: it holds the playhead, it is the queue that plays or played
  last, and every Client without the Queue chooser (the TUI) shows it. The other queues rest:
  they keep their slots, their name, their Queue source, and their start slot, and they do not
  play. All queues persist across a restart, with their slot identities.
- **Start slot.** Each queue has one start slot: the active slot while it plays, and the slot
  where Play starts it while it is stopped or resting. A stop in the middle of an item keeps that
  slot. A natural completion moves the start slot to the next slot.
- **Queue chooser (GUI only).** The GUI queue header shows a dropdown of queues (not a tab strip),
  the queue search, a new-queue control, and a close control. The dropdown hides while only one
  queue exists. Each local queue and each connected peer is one entry. The user can rename a local
  queue.
- **Peer tabs.** A connect from the playback-target chooser adds the peer as its own entry in the
  Queue chooser and shows it. Closing a peer entry disconnects and leaves the peer playing. The
  whole player panel follows the owner of the shown entry.
- **Close.** Closing a resting queue deletes it at once. Closing the current queue while it plays
  asks for confirmation and then stops playback and deletes it. The last local queue cannot
  close.
- **Play.** With nothing playing, Play starts the shown queue at its start slot and makes it
  current. A row play in a resting queue makes that queue current and plays the row. Showing a
  queue never starts playback. The next item always comes from the current queue.
- **Edits follow the shown queue.** In the GUI, enqueue, play-next, replace, and row edits reach
  the shown queue. The TUI keeps editing the current queue.
- **Header clean-up (GUI). BREAKING (GUI only):** the remaining count, the queue finish clock in
  the header, the Length / Ends at switch, the `t` right-column chord, and the rows' right-column
  value are removed. The queue search stays, stays fuzzy, and covers only the shown queue.
- **Footer times (GUI).** The footer shows the total time of the shown queue and, while rows are
  marked, the total time of the marked rows. The owner label leaves the footer, because the Queue
  chooser now names the owner. The other footer contents stay.
- **Start slot mark (GUI).** The start slot row is filled while its queue plays and outlined while
  the queue is stopped or resting.
- **Domain and decision records.** `CONTEXT.md` gains *Named queue*, *Current queue*, *Start slot*,
  and *Queue chooser*. A new ADR amends ADR 0034: the GUI shows one entry per Player owner
  connection, and the connection still decides which owners appear.

## Capabilities

### New Capabilities

- `named-queues`: the Owner process's durable named queues, the current queue, the start slot,
  create, rename, close, play-from-queue, persistence, and how a Client without the Queue chooser
  sees them.
- `gui-queue-chooser`: the GUI queue header's Queue chooser, peer entries, the shown queue, its
  search scope, the new and close controls, the close confirmation, the keys, and the player
  panel that follows the shown entry's owner.

### Modified Capabilities

- `gui-client`: the queue header loses its count, finish clock, and right-column switch; the rows
  lose their right-column value; the footer gains the queue and marked-row totals and loses the
  owner label; the queue marks the start slot.
- `unified-playback-queue`: queue edits reach the owner and the named queue that the Client shows.
- `local-daemon-thin-client`: the local Owner holds and persists every named queue; a Client
  without the Queue chooser shows the current queue.

## Impact

- `crates/mbv-queue`: the queue state carries slot identities and a start slot; a named-queue set
  type.
- `crates/mbv-daemon`: the Owner holds the named-queue set beside its current Bound queue, routes
  queue edits by queue identity, swaps the current queue on play, persists every queue, and
  migrates the single-queue state file.
- `crates/mbv-ctrl`: one additive capability (`named-queues`) with new commands and events and a
  defaulted queue field on the queue edit commands. No protocol version bump.
- `crates/mbv-config`: the named-queue state file and its path.
- `src/app`: the GUI shown-queue selection (local named queue or peer), the resting-queue
  snapshot, queue-addressed edits, the removed right-column state and `t` chord, the new GUI
  chords, and the footer totals projection.
- `crates/mbv-gui`: the queue header, the Queue chooser overlay, the footer, the row right column,
  and the start slot mark.
- Packaged `mbvd` does not advertise `named-queues` and keeps one queue. The TUI gains no control.
- Docs: `CONTEXT.md`, a new ADR, `docs/architecture/gui-client.md`, and an invariant for the
  start slot.
