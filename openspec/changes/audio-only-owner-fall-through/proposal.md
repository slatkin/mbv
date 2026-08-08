## Why

A client directly controlling an audio-only `mbvd` cannot play a video without
ending that control relationship first. The client needs to keep the owner and
its Bound queue attached while routing an explicit non-audio action to its own
Player.

See ADR 0017 for the queue-stage and ownership model.

**Depends on `audio-only-mixed-queue-admission`**, which independently ensures
an audio-only owner never binds an unplayable item and accepts the audio portion
of a mixed submission. It must ship first, but its implementation status does
not block implementing this client-side change.

## What Changes

- `mbvd` advertises audio-only capability during the ctrl handshake. This is an
  additive capability string; `CTRL_PROTOCOL_VERSION` does not change.
- Fall-through applies to explicit play and enqueue actions made through
  Sessions-panel Direct remote control or an explicit remote daemon attachment.
  Library routes, Session watch, auto-advance, resume, and owner-initiated events
  do not invoke it.
- Attachment, Transport owner, remote queue availability, visible Queue scope,
  and per-action Submission destination are modeled independently. Local
  fall-through playback does not disconnect the owner or hide its Bound queue.
- Player events retain Local or Attached-owner origin, so events from the parked
  owner cannot mutate the local fall-through queue or end local playback.
- A non-audio explicit play prepares a local Player if necessary, then stops the
  attached owner and plays locally. Ending local playback returns transport
  control to the still-attached owner.
- A wholly non-audio enqueue goes to the client's own queue without starting
  playback. A mixed selection is stripped before owner submission and reports
  the dropped count; stripped items are not staged.
- While a fallen-through item plays, the Remote queue view remains available and
  shows that item as a derived, pinned, non-selectable row.
- An owner that does not advertise audio-only produces today's behavior,
  including the structured rejection for wholly non-audio submissions.

## Capabilities

### New Capabilities

- `non-audio-fall-through`: Per-action routing, ownership, queue visibility, and
  event-origin behavior when a directly controlled owner is audio-only.

### Modified Capabilities

- `ctrl-protocol`: An audio-only daemon advertises that capability during the
  hello handshake without a protocol-version bump.

## Impact

**Protocol** — `crates/mbv-core/src/ctrl.rs`, daemon hello construction, and the
remote-player connection state gain an additive audio-only capability fact.

**Client** — player arrangement/session ownership, queue-scope resolution,
explicit play/enqueue routing, player-event draining, MPRIS rebinding, and the
remote queue render path change together. Remote queue commands must reach the
attached-owner session while transport commands reach the Transport owner.

**Not affected** — Library-route selection or lifecycle, Session watch,
auto-advance within Bound queues, the user-session Local daemon (which is not
audio-only), or the prerequisite daemon admission filter.
