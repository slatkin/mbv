---
status: accepted
---

# Each Player Owner Keeps Its Own Queue

Each Player owner keeps its own Bound queue. Connect moves only control: a
Client attached to a peer Player owner sees and edits the peer's queue, and
the music stays where it plays. Disconnect leaves the peer playing. Hand off
and Take copy a queue between Player owners and move the music. This decision
extends [ADR 0012](0012-sessions-panel-connection-ownership.md) and supersedes
the Queue-scope sentences of
[ADR 0017](0017-composed-and-bound-queue-stages.md). The full design and its
alternatives are in
[the change design](../../openspec/changes/queue-handoff-and-take/design.md)
(decisions D1 to D9).

## Decision

No Player owner holds another owner's queue as its own. Each keeps one
canonical Bound queue, and two copies of one queue can exist at one time.

Connect and Disconnect move only control. A Client that connects to a peer
Player owner sees and edits the peer's queue; the music stays where it plays.
Disconnect returns the queue panel to the local Owner queue and leaves the
peer playing. Connect is reversible and side-effect free.

Two Client actions copy a queue and move the music:

- **Hand off** copies the local Owner queue and its position to the peer,
  starts the copy there, stops local playback, and stays connected.
- **Take** copies the peer queue and its position to the local Owner, starts
  the copy here, and stops the peer. It works with or without a connection,
  and it disconnects when it succeeds from a connection.

Both need playback on the source queue. Both ask for confirmation before they
replace a populated target queue, and neither stops the source until the
target reports the copied start slot playing. The start slot carries its
position over an additive ctrl field and a capability, so the item continues
where it was instead of restarting from the per-medium source.

The queue panel always names the Player owner whose queue it shows: the peer
queue while a peer is connected, the local Owner queue otherwise. No
user-selectable queue scope exists, and edits go to the owner whose queue is
shown.

## Context

[ADR 0012](0012-sessions-panel-connection-ownership.md) gives F3 ownership of
the connections its selections create: `Enter` is an explicit takeover, and
`d` disconnects only a Sessions-sidebar connection. This decision extends the
same ownership rule to copying: F3 now owns Hand off and Take as well as
connect. They are fixed sidebar keys like `r` and `d`, offered only on a row
with a direct endpoint, and dispatch resolves the row at action time. Nothing
in ADR 0012 changes: disconnect stays F3-owned and asymmetric, and library
routes stay outside F3's authority.

[ADR 0017](0017-composed-and-bound-queue-stages.md) treats queue visibility as
independent presentation state. Two of its sentences are superseded: "Queue
scope remains independent presentation state, and Submission destination is
calculated for each explicit action", and "Queue visibility is derived from
queue availability plus the requested Queue scope, not from the Transport
owner". Visibility now derives from the connection, not from a requested
scope. The rest of ADR 0017 stands: queues stay Bound to a Player owner, the
owner's queue stays canonical, and admission still runs at the target owner
when a copy arrives.

The removed scope was already invisible to the user: its pills were dropped,
so the Local and Remote split had no visible name and the same connect verb
did two different things. Making the shown queue follow the connection removes
the hidden state instead of restoring the pill.

## Considered options

- **Keep and re-show the Local/Remote scope (rejected).** The scope asked the
  user to manage presentation state that the connection already decides, and
  its pill was gone. Two verbs that reach the same two queues is one too many.
- **Copy the queue on every connect (rejected).** Connect would stop the
  peer's music or fork its queue as a side effect of a control action. The
  move gets a verb of its own, and connect stays side-effect free.
- **Owner-to-Owner links, so the local Owner holds the peer queue
  (rejected).** It needs an Owner-to-Owner ctrl client and a second authority
  relation, and its only gain is a Tray that follows the peer. The Tray stays
  local by design.

## Consequences

- Two copies of one queue drift by intent after a Hand off. The owner label on
  the queue panel names which copy edits reach.
- A copy carries the item sequence, the start index, and the Queue source. The
  target keeps the submitted slot ids and mints a new lineage. Feed positions
  and Audiobookshelf progress stay in their own stores.
- The start slot of a copy resumes from the carried position, so *Resume
  source* gains a Handoff value.
- A peer without the start-position capability refuses a Hand off with a clear
  message. Take needs only the local Owner, which ships with the Client.
- Related decisions: [ADR 0012](0012-sessions-panel-connection-ownership.md)
  owns the Sessions-sidebar relations this extends,
  [ADR 0017](0017-composed-and-bound-queue-stages.md) keeps the Bound stage
  and owner admission, [ADR 0019](0019-player-owners-resolve-service-playback.md)
  keeps Service playback lifecycle with the Player owner, and
  [ADR 0030](0030-owner-process-is-the-only-local-player-owner.md) keeps the
  Owner process as the only local Player owner.
