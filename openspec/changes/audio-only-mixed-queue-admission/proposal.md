## Why

An audio-only `mbvd` refuses any play request whose resolved items are not all
audio, and that refusal covers the whole request. Selecting one track from a
playlist that happens to contain a music video plays nothing.

The refusal cannot simply be removed. The daemon hands its whole item list to
mpv as an mpv playlist and mpv advances through it unaided, so the check is the
only thing keeping video files away from a player with no display.

This change fixes that one problem, on the daemon, and ships on its own. The
separate problem — a client connected to `mbvd` having no way to play a video at
all — is `audio-only-owner-fall-through`, which depends on this change but is
not required by it.

See ADR 0017 for the model both changes implement.

## What Changes

- An audio-only Player owner accepts a submission containing non-audio items,
  admitting the audio items and discarding the rest, instead of refusing it
  whole. Nothing non-audio reaches its mpv.
- A start index that lands on a discarded item is remapped rather than clamped,
  so playback starts on the item the user picked or the next one that survives.
- A submission that is *wholly* non-audio still gets today's `AudioOnly`
  rejection. That path is unchanged, so an old client sees exactly what it sees
  now.
- Discards are logged, not reported over ctrl.

## Capabilities

### New Capabilities

- `audio-only-queue-admission`: What an audio-only Player owner accepts into its
  queue, what it discards, and what reaches its mpv. Covers the ctrl play path,
  the playback-intent path, and the ws path.

## Impact

**Daemon** — `daemon_core.rs` (`audio_only_rejection`), `daemon_control.rs:361`
(`CtrlCmd::PlayItems`), `daemon_run.rs:559` (playback intents),
`daemon_ws.rs:35` (Emby-started playback). Admission filtering replaces
whole-request rejection at all three call sites.

**Not affected** — the ctrl protocol (no new capability string here; that
belongs to the fall-through change), any client code, auto-advance, the
user-session local daemon (`local_daemon.rs` passes `audio_only: false`, so the
filter is a no-op there).
