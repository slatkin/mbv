# Proposal

## Why

Stay-alive keeps the local Owner process playing after the last Client window closes, but MPRIS is
published only by the Client process. When the window closes, `org.mpris.MediaPlayer2.mbv` leaves
the session bus while audio keeps playing, so media keys and desktop MPRIS consumers report
nothing. Observed 2026-11-19: the GUI Client exited five seconds after starting playback; the
Owner kept playing and no MPRIS name stayed on the bus.

## What Changes

- The local Owner process publishes MPRIS. It holds `org.mpris.MediaPlayer2.mbv` for as long as it
  runs, whatever Clients are attached, publishes its own playback status and metadata, and
  dispatches transport commands to its Player. No other process can take that name from it.
- A Client attached to the local Owner publishes no MPRIS. The Owner is the one mbv player that
  MPRIS consumers see for local playback.
- A Client that controls a remote Player owner (Direct remote control or a Library route)
  publishes that Player owner under its own name, `org.mpris.MediaPlayer2.mbv.instance<pid>`, as
  the MPRIS Bus Name Policy prescribes for additional instances. Local playback wins: the Client
  holds that name only while the local Owner's playback status is not `Playing`.
- The packaged daemon `mbvd` starts no MPRIS publisher. It runs without a user desktop session,
  like its tray.
- No protocol, capability, or setting changes.

## Capabilities

### New Capabilities

- `local-owner-mpris`: Which mbv process publishes MPRIS under which bus name — the local Owner
  process for local playback, a Client for the remote Player owner it controls, the local-wins
  rule between them, and the packaged daemon's exclusion.

### Modified Capabilities

None. No main spec covers MPRIS publishing today. The art-cache key rules
(`queue-artwork-title-overlay`) and the transport-resolution rules for MPRIS commands
(`unified-playback-queue`, `daemon-playback-intents`) stay as specified; both publishers uphold
them.

## Impact

- `crates/mbv-desktop/src/mpris.rs`: an Owner entry point that holds the fixed name; the Client
  entry point serves its interfaces without a name and claims and releases its instance name by a
  pure publishing rule.
- `src/local_daemon.rs`: the Local role starts the Owner publisher from its existing
  `on_player_ready` hook, which already receives the Player status and the transport sender. The
  packaged entry never runs this code path.
- `src/app/state/construct/remote.rs` and `src/app/dispatch/session/switch.rs`: the Client's
  MPRIS start and rebind pass the publishing rule's inputs (the attached endpoint and the
  suspended local link's status).
- Dependencies: none new or upgraded. `zbus` 5.19.0 (already pinned) supplies the name-claim
  flags and `release_name`.
