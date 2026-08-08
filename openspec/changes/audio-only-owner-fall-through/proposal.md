## Why

An audio-only `mbvd` refuses any play request whose resolved items are not all
audio, and that refusal covers the whole request — selecting one track from a
playlist that happens to contain a music video plays nothing. Separately, a
client connected to `mbvd` cannot play a video at all: everything routes to the
daemon, the daemon refuses, and the only remedy is to disconnect, play it, and
reconnect.

The refusal cannot simply be removed. The daemon hands its whole item list to
mpv as an mpv playlist and mpv advances through it unaided, so the check is the
only thing keeping video files away from a player with no display.

See ADR 0017 for the model, the rejected alternatives, and the limits this
change does not address.

## What Changes

- An audio-only owner accepts a submission containing non-audio items, minus
  those items, instead of refusing it whole. Nothing non-audio reaches its mpv.
- A controlling client strips non-audio items before submitting and reports how
  many it dropped. The owner discards any that arrive regardless, as a backstop
  and for Emby-started playback where no client is involved.
- `mbvd` advertises that it is audio-only during the ctrl handshake, so the
  client decides before submitting rather than learning by rejection. Additive
  capability string — no `CTRL_PROTOCOL_VERSION` bump.
- A non-audio item explicitly played or enqueued while a client holds Direct
  remote control over an audio-only owner falls through to that client's own
  queue instead. Explicit user action only, never auto-advance. The control
  connection stays up.
- Playing a fallen-through item stops the owner. The owner remains the target
  for the next queue addition.
- "Which player is the active playback target" becomes its own value on the
  client rather than being derived from whether a remote attachment exists.
  Attachment fields are unchanged.
- A playing fallen-through item is pinned at the top of the remote queue view in
  selected-row styling, non-selectable and skipped by cursor navigation.
- The existing `AudioOnly` rejection stays as a defensive backstop rather than
  the primary mechanism. Not breaking: an owner that does not advertise the
  capability produces exactly today's behavior.

## Capabilities

### New Capabilities

- `audio-only-queue-admission`: What an audio-only Player owner accepts into its
  queue, what it discards, and what reaches its mpv. Covers both the ctrl play
  path and the playback-intent path.
- `non-audio-fall-through`: A client's decision to route an explicitly launched
  or enqueued non-audio item to its own queue rather than to the audio-only
  owner it is controlling, including what happens to the owner and where the
  item is shown.

### Modified Capabilities

- `ctrl-protocol`: A daemon SHALL advertise an audio-only capability during the
  hello handshake when it cannot play non-audio items, so a client can route
  before submitting. Additive capability string, no version change.

## Impact

**Daemon** — `daemon_core.rs` (`audio_only_rejection`, `all_audio`),
`daemon_control.rs` (`CtrlCmd::PlayItems`), `daemon_run.rs` (playback-intent
path), `daemon_ws.rs`. Admission filtering replaces whole-request rejection.

**Protocol** — `ctrl.rs`: new capability constant alongside
`CTRL_CAP_QUEUE_STATE` and friends, advertised in `CtrlHello::current()` when
the daemon runs audio-only. `CTRL_PROTOCOL_VERSION` unchanged.

**Client** — `src/app/session_connect.rs` (the `is_remote()`/`player_endpoint`
pairing, `restore_local_mode`), `src/app/library_route.rs` and
`src/app/actions.rs` (explicit play/enqueue sites), `src/app/queue_scope.rs`,
and the queue render path for the pinned row. 27 non-test `is_remote()` call
sites need reading for which question each asks — "is there a connection" or
"where does playback go".

**Not affected** — auto-advance within any queue; the local daemon
(`local_daemon.rs` passes `audio_only: false`, so it never advertises the
capability and nothing falls through); library routing config and resolution
order.
