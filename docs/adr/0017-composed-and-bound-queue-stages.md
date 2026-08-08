# Composed and Bound Queue Stages

## Decision

A queue passes through stages, and the rules that apply to it depend on which
stage it is in.

While **Composed**, a queue is held in a client's UI and no Player owner is
executing it. Editing it has no playback consequence. A client may compose a
queue while a different, Bound queue plays elsewhere — the composed one sits
idle until something binds it.

Once **Bound**, a Player owner is executing the queue, and its contents answer
to that owner's rules.

From this: **constraints that protect execution belong at bind time, not at
edit time.** A rule that refuses an edit in order to keep execution valid is in
the wrong place. Compose freely; binding is where a queue meets its owner's
limits.

Applied to an audio-only Player owner (`mbvd`, per ADR 0007's control
authority):

- It accepts a queue containing non-audio items rather than refusing the whole
  request. The non-audio entries stay in the queue as Unplayable items, visible
  where they were put, and are skipped on advance. They are neither dropped at
  accept time nor grounds for rejecting the queue they arrived in.
- Items already inside a Bound queue are never dispatched anywhere. By the time
  the queue reaches an Unplayable item, no client may be running, and any that
  is may be unattended. Skipping is the only behaviour that does not depend on
  someone being there.
- A non-audio item **explicitly** played or enqueued by a client holding Direct
  remote control over that owner falls through to the client's own Composed
  queue. The owner is not asked and not told; the control connection stays up.
  This applies to a deliberate user action only, never to auto-advance.
- Playing a fallen-through item stops the owner's playback rather than pausing
  it. The intent is to play this instead, not to hold a place, and a paused
  owner would sit holding an mpv process, an open Emby session, and (in
  audio-pipe mode) the pipe itself for the length of a film.
- The owner remains the target for the next queue addition. Fall-through is a
  per-item exception, not a mode the client enters and must leave.

A batch is never split across two owners in either direction. A batch
containing any audio goes to the owner whole, with its non-audio entries held
there as Unplayable. Only a wholly non-audio batch falls through. The
consequence is accepted: a video played on its own goes to the client, and the
same video inside an album goes to the owner and is skipped there.

Fall-through is driven by an advertised capability, not by a rejection. The
owner declares that it is audio-only during the ctrl handshake, and the client
decides before submitting. The existing structured rejection remains as a
defensive backstop.

## Context

`mbvd` runs as a system service with no user session and therefore no display;
the packaged unit runs it with `--audio-only`. Until now it refused any play
request whose resolved items were not all audio. That refusal covered the
entire request, so selecting a single track out of a playlist that happened to
contain a music video played nothing at all.

The refusal could not simply be deleted. The daemon hands its whole item list
to mpv as an mpv playlist and mpv advances through it unaided, so the check was
the only thing keeping video files away from a player that has no display. Any
fix had to change what the daemon gives mpv, not just what it accepts.

Two models were considered for the non-audio items themselves.

The first dispatched them back to a connected client to render, keeping the
daemon as queue authority throughout. It was rejected. It assumes a client is
present, attended, and has a display, none of which is reliable for a queue
that may reach the item hours later. It would silence an audio pipe feeding
other equipment partway through a queue. And it required a daemon-to-client
render protocol, split Emby session reporting, transport-control routing
between two processes, and recovery for a client that disappears mid-item.

The second — adopted — separates the two things a queue is. The daemon's
contract is execution: items, a cursor, an mpv playlist, advancing, skipping
what it cannot play. The UI's contract is editing. Both are true of the same
queue at once today, which is why constraints belonging to execution are
currently enforced against edits. Naming the stages lets each contract apply
where it belongs.

An earlier variant of the adopted model had the daemon filter non-audio items
out when accepting a queue. That keeps the daemon's items, its cursor, and
mpv's playlist in agreement, which is cheaper. It was rejected as inconsistent
with the editing contract: entries would vanish from the queue without the user
being told, which is worse than the refusal it replaced.

This ADR records the model. Implementation is #431.

## Consequences

- The daemon's queue and mpv's playlist stop being the same list. Its cursor
  and mpv's `playlist-pos` become distinct indices over distinct sequences and
  must not share a type.
- The ctrl handshake gains an audio-only capability string. Additive, so no
  `CTRL_PROTOCOL_VERSION` bump, per the rule above that constant.
- On the client, "a remote connection exists" and "the remote player is the
  active playback target" stop being the same fact. They are currently asserted
  to be identical in `restore_local_mode`; fall-through separates them, and the
  pair wants to become one state rather than two fields kept in agreement by
  convention.
- `restore_local_mode` is not the path back for fall-through. It disconnects
  the remote, which is exactly what fall-through exists to avoid, and it
  deliberately leaves a remote playing.
- A fallen-through item that is playing appears in both queue views: normally
  in the Composed queue, and pinned at the top of the Bound queue's view in
  selected-row styling, non-selectable and skipped by cursor navigation. It is
  not a member of that queue and must not imply a position in it.
- Enqueuing never starts playback, in either stage. Disconnecting from an owner
  leaves the client's Composed queue loaded and idle rather than starting it;
  disconnection is session management, not a play command.
- `--audio-only` is currently a flag on `mbvd` because `run_with_options` is
  shared with the user-session local daemon, which passes false. As packaged,
  `mbvd` always sets it. Running `mbvd` by hand without it advertises video
  capability the process cannot deliver, and fall-through would not fire.
  Hardcoding it for `mbvd` is a separate cleanup.
- Existing edit-time constraints, most visibly the route-lineage rule that
  refuses an enqueue whose resolved route differs from the queue's
  (ADR 0011), are left in place. This ADR gives the test by which they should
  later be revisited, not a mandate to remove them now.
