---
status: accepted
---

# The GUI Shows One Entry per Queue

In the GUI Client, the shown entry, not the connection alone, picks the
queue the queue panel shows. The Queue chooser lists one entry for each
Named queue of the local Owner, then one entry for each connected peer
Player owner. The connection decides which peers are listed; the shown
entry decides which queue the panel and the edits reach. The TUI keeps
the connection rule of
[ADR 0034](0034-each-player-owner-keeps-its-own-queue.md) unchanged.

## Decision

- The GUI queue header carries the **Queue chooser**, a dropdown, not a
  row of tabs. It lists every Named queue of the local Owner in queue
  order, then one entry for each peer the GUI Client is connected to,
  named by the peer's device name. It marks the shown entry and the
  current queue. It is absent while the list holds exactly one entry.
  Choosing an entry shows it and moves no playback. Showing an entry is
  presentation only: it starts, stops, and moves nothing.
- The **shown entry** decides the shown queue. It is one of the local
  Named queues or the connected peer. Every queue edit, enqueue,
  play-next, replace, and row play from the GUI reaches the shown
  entry's queue, and the whole player panel follows the shown entry's
  owner. The shown queue and the player panel still resolve to one
  link, so ADR 0034's single-link rule stays true; only the choice of
  link changes.
- The **connection decides which peers are listed**. A connect from the
  playback-target chooser adds that peer's entry and shows it. A
  disconnect, a lost connection, or a close of the peer entry removes
  it and shows the current local queue. At most one peer is connected,
  so the list holds at most one peer entry; the model allows more
  later. A connect never hides the local Named queues, which was the
  behaviour before this decision.
- The **TUI keeps the connection rule**. Without the Queue chooser, a
  TUI Client shows the current queue: the local Owner queue, or the
  peer's queue while a peer is connected. The current queue is the only
  Named queue that plays, so the TUI sees every playback change.
- The Owner keeps **one current queue plus a resting store**, not N
  live queues. Exactly one Named queue holds the playhead and drives
  the one Playback run and the one output. The other Named queues rest
  with their slots, name, Queue source, lineage, and start slot.

## Context

[ADR 0034](0034-each-player-owner-keeps-its-own-queue.md) makes the
shown queue follow the connection: while a peer is connected, the queue
panel shows the peer's queue, and nothing shows the local queue. With
several Named queues in the local Owner, that rule hides every local
queue but the current one during a connect, and nothing tells the user
that the local queues still exist. The GUI also needs one place where
the user picks between its own queues and a connected owner. The Queue
chooser is that place. The owner label leaves the queue footer, because
each chooser entry names its owner.

This decision amends
[ADR 0034](0034-each-player-owner-keeps-its-own-queue.md): the sentence
"the queue panel always names the Player owner whose queue it shows ...
the peer queue while a peer is connected, the local Owner queue
otherwise" becomes "the queue panel names the owner of the shown entry,
which the Queue chooser picks". Connect and Disconnect still move only
control and stay side-effect free. Play of a resting queue is the one
new action that moves playback, and it is an explicit user action, not
a side effect of connect.

## Considered options

- **N live queues (rejected).** The Owner would hold N queues, each
  with its own transitions and lineage machinery. Only one Playback run
  and one output exist per Owner, so live state for N queues adds
  playback invariants with no user-visible gain. One current queue
  holds the run; the other Named queues rest in a store.
- **Keep the connection-only shown queue (rejected).** It is ADR 0034's
  rule unchanged. A connect hides every local queue but the current
  one, and the GUI has no way to show a resting local queue or to edit
  it.
- **One entry per queue in the Owner, peers shown as tabs (rejected).**
  A tab strip implies several queues shown at one time and puts the
  peer in the same row as the local queues. The dropdown shows the
  shown entry's name in the header, keeps the peer visually apart, and
  hides itself while one entry is enough.

## Consequences

- A connect no longer hides the local queues. The user shows a resting
  queue, edits it, and plays it without touching the connection.
- The chooser entry names the owner of each queue, so no separate owner
  label is needed in the panel.
- Edits reach the shown queue. Two Clients can show different entries
  and edit different queues; slot-addressed rules apply to each queue
  as they apply to the current queue today.
- A Client that shows a queue another Client closes falls back to the
  current queue when the chooser list reports the close.
- Related decisions:
  [ADR 0034](0034-each-player-owner-keeps-its-own-queue.md) keeps one
  Bound queue per Player owner and side-effect-free connect,
  [ADR 0012](0012-sessions-panel-connection-ownership.md) keeps the
  Sessions-sidebar ownership of connections, and
  [ADR 0040](0040-gui-key-target-follows-the-pointer.md) keeps the
  pointer as the key target, which gates the chooser keys on the Queue
  list box.
