# Proposal

## Why

On 2026-09-23 a client attached to a remote mbvd (stay-alive) showed a queue of
Emby music, an Audiobookshelf podcast was added to it, and the podcast would
not play. Log forensics across both machines (mbvd journald + client
`mbv.log`) proved the queue/playback machinery was never reached: every
client→owner command after the attach silently vanished. The mixed-queue
report was a symptom; the cause is that a remote daemon connection can die
without the client noticing or saying anything:

- The ctrl writer thread (`remote_player/connect.rs`) breaks on the first
  write error with no log and without setting the `disconnected` flag, so
  every later `send_ctrl_cmd` still "succeeds" into a dead socket.
- The app never consults `is_disconnected()` outside MPRIS. A reader-side
  drop only logs an ambiguous `daemon disconnected` info line and emits a
  spurious `Stopped` event that the app treats as an ordinary playback stop.
- The enqueue path ignores `submit_queue_item`'s `false` return, so a failed
  append is invisible; the play path flashes "Requesting playback…" and
  nothing ever happens.

The user is left looking at a stale adopted queue whose every interaction
does nothing — indistinguishable from a playback bug. mbvd itself logs
nothing at info level about client connects/disconnects, so neither side
leaves actionable evidence.

## What Changes

- The ctrl writer thread detects write failure, logs it, and marks the
  connection disconnected (the existing shared `disconnected` flag) before
  exiting, so send-side death is observable exactly like reader-side death.
- `send_ctrl_cmd` and the queue/play submission paths refuse to send on a
  disconnected remote: queue edits roll back with a visible toast, and
  playback requests flash a connection-lost warning instead of
  "Requesting playback…".
- An unexpected remote disconnect surfaces a toast (e.g. "Lost connection to
  the daemon's device") and drives the same return-to-local-presentation behavior the
  `remote-queue-disconnect` spec already requires for user-initiated and
  daemon-announced disconnects — closing the gap where only the `d`
  disconnect path implemented it.
- Deliberately not in scope: automatic reconnection to the remote daemon
  (a follow-up if the surfaced loss proves annoying), and any change to
  local-daemon recovery (the recovery dialog spec is untouched).

## Capabilities

### Modified Capabilities

- `daemon-disconnect-handling`: A remote-daemon client shall detect and
  surface connection loss from both the reader side and the writer side, and
  shall not accept queue or playback commands for a known-dead connection.
- `remote-queue-disconnect`: An unexpected (non-user, non-announced) remote
  disconnect shall trigger the same return-to-local-presentation behavior the
  spec already requires, so the stale remote queue snapshot cannot remain
  presented and commandable.

## Impact

- Affects `crates/mbv-core/src/remote_player/connect.rs` (writer thread),
  `crates/mbv-core/src/player/proxy.rs` and the app submission paths
  (`src/app/actions.rs` `submit_queue_item`, jump/enqueue send paths), and
  the app's remote-disconnect handling in the run loop.
- No wire-protocol change; no daemon-side change required (though a daemon
  log line on client disconnect would aid future forensics — optional).
- Evidence for the diagnosis lives in the 2026-09-23 daily log; the incident
  itself is not reproducible on demand (the trigger for the original drop is
  unknown), so acceptance is by hermetic unit tests over the disconnect
  paths, not by a live repro.
