# Composed and Bound Queue Stages

## Decision

A queue is either Composed or Bound, and which rules apply to it depends on
which.

**Composed**: held in a client's UI, with no Player owner holding it. Editing it
has no playback consequence. A client can build one while a different queue
plays elsewhere.

**Bound**: a Player owner holds it. Its contents answer to that owner's rules.
Bound does not mean playing — a stopped owner still holds its queue, and two
owners can each hold a queue while only one of them plays.

From this: **constraints that protect execution apply when a queue binds, not
when it is edited.**

Applied to an audio-only Player owner (`mbvd`):

- It never holds an item it cannot play. A client with Direct remote control
  strips non-audio items before submitting; the owner discards any that arrive
  regardless. Nothing non-audio reaches its mpv.
- A submission containing non-audio items is accepted minus those items, rather
  than refused whole. Selecting one track from a mixed playlist plays that
  track.
- Items are never dispatched from an owner's queue to a client. By the time a
  queue reaches a given item, no client may be running and any that is may be
  unattended.
- A non-audio item **explicitly** played or enqueued by a controlling client
  falls through to that client's Composed queue. The owner is not asked and not
  told; the control connection stays up. Deliberate user action only, never
  auto-advance.
- Playing a fallen-through item stops the owner rather than pausing it. Pausing
  would hold an mpv process, an open Emby session, and in audio-pipe mode the
  pipe itself, for the length of a film.
- The owner stays the target for the next queue addition. Fall-through is a
  per-item exception, not a mode the client enters and has to leave.

Routing is capability-led. The owner declares it is audio-only during the ctrl
handshake and the client decides before submitting. The existing structured
rejection stays as a backstop.

## Context

`mbvd` runs as a system service with no user session and therefore no display;
the packaged unit runs it with `--audio-only`. It refused any play request whose
resolved items were not all audio, and that refusal covered the whole request,
so selecting one track from a playlist containing a music video played nothing.

The refusal could not simply be deleted. The daemon hands its whole item list to
mpv as an mpv playlist and mpv advances through it unaided, so the check was the
only thing keeping video files away from a player with no display. Any fix had
to change what the daemon gives mpv.

**Dispatching non-audio items back to a client to render was rejected.** It
assumes a client is present, attended, and has a display, none of which holds
for a queue that may reach the item hours later. It silences an audio pipe
feeding other equipment partway through a queue. It requires a
daemon-to-client render protocol, split Emby session reporting,
transport-control routing across two processes, and recovery for a client that
disappears mid-item.

**Holding non-audio items in the owner's queue, marked unplayable and skipped on
advance, was also rejected.** It keeps the queue faithful to what was submitted,
which is the behaviour the editing contract argues for. But it splits the
owner's item list from mpv's playlist, so the owner's cursor and mpv's
`playlist-pos` become different indices over different sequences — a permanent
cost in the daemon for visibility that is better provided at the client, which
is where the user is and where the strip decision is made anyway.

## Consequences

- The owner's queue and mpv's playlist stay the same list. No index mapping is
  introduced.
- The ctrl handshake gains an audio-only capability string. Additive, so no
  `CTRL_PROTOCOL_VERSION` bump, per the rule above that constant.
- On the client, "a remote connection exists" and "the remote player is the
  active playback target" stop being the same fact. `restore_local_mode`
  currently asserts they are identical. Fall-through separates them.
- `restore_local_mode` is not the path back for fall-through: it disconnects the
  remote, which is what fall-through exists to avoid, and it deliberately leaves
  a remote playing.
- A fallen-through item that is playing appears in the Composed queue normally,
  and pinned at the top of the Bound queue's view in selected-row styling,
  non-selectable and skipped by cursor navigation. It is not a member of that
  queue and does not imply a position in it.
- Enqueuing starts nothing, in either stage. Disconnecting from an owner leaves
  the client's Composed queue loaded and idle; disconnection is session
  management, not a play command.

## What this does not account for

Stated as limits, not as work items. Some are acceptable; some need a decision
before implementation.

- **An owner that does not advertise the capability gets today's behaviour.**
  The client submits, the owner rejects, nothing falls through. Mixed-version
  pairs are unchanged rather than broken.
- **A daemon-side discard is silent.** The client is where the user is told, and
  the client strips first, so the owner's discard normally has nothing to
  report. When the client's view of an item's type is wrong or stale, the item
  is discarded with only a log line and the user is not told. Reporting discards
  back over ctrl is not part of this.
- **Playback started from Emby has no client at all.** Non-audio items are
  discarded with a log line and nobody is told. This is the same position as
  before, where the whole request was refused with nobody told.
- **Stopping the owner discards its position.** Returning to the music starts it
  again rather than resuming. This is the accepted cost of stop over pause.
- **If the client exits while a fallen-through item plays, the owner is stopped
  and stays stopped.** Nothing restarts it.
- **The output device changes without an explicit user action.** After a
  fallen-through film ends, the next song plays on the owner again. The queue
  scope indicator shows this, but no action was taken to move it back.
- **Emby sees a handoff, not a continuous session.** The owner's session ends
  and the client's begins. Resume points are per item, so they are unaffected.
- **A client that starts up already attached has one queue, not two.**
  `bootstrap.rs` builds its `player_tab` from the remote items, so there is no
  separate local queue on that path for a fallen-through item to land in. A
  client that attaches after starting locally has both (`player_tab` and
  `remote_player_tab`, per `queue_scope.rs`) and is unaffected.
- **Where stripped items go is undecided.** When a mixed batch is submitted to
  an audio-only owner, the non-audio items are stripped. Whether they are
  dropped, or land in the client's Composed queue the way a wholly non-audio
  batch does, is open.
- **Moving items between a Composed and a Bound queue by hand is not included.**
  The model permits it; nothing implements it.
- **Existing edit-time constraints are left in place**, most visibly the
  route-lineage rule that refuses an enqueue whose resolved route differs from
  the queue's (ADR 0011). This ADR gives the test by which to revisit them. It
  does not revisit them.

This ADR records the model. Implementation is #431.
