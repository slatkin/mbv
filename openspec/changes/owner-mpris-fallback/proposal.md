# Proposal

## Why

Stay-alive keeps the local Owner process playing after the last Client window closes, but MPRIS is
published only by the Client process. When the window closes, `org.mpris.MediaPlayer2.mbv` leaves
the session bus while audio keeps playing, so media keys and desktop MPRIS consumers report
nothing. Observed 2026-11-19: the GUI Client exited five seconds after starting playback; the
Owner kept playing and no MPRIS name stayed on the bus.

## What Changes

- The local Owner process starts an MPRIS fallback publisher. It claims
  `org.mpris.MediaPlayer2.mbv` only while no Client holds the name, publishes the Owner's own
  playback status and metadata, and dispatches transport commands to the Owner's Player.
- A starting Client keeps winning the name: its claim replaces the fallback, as it already does
  today between Clients. When the last Client exits, the Owner re-claims the name and playback
  keeps its MPRIS surface.
- The packaged daemon `mbvd` starts no MPRIS publisher. It runs without a user desktop session,
  like its tray.
- No protocol, capability, or setting changes. The fallback is always on for the local Owner.

## Capabilities

### New Capabilities

- `local-owner-mpris`: The local Owner process publishes MPRIS whenever no Client holds the bus
  name — claim and handoff rules, published state, and the packaged daemon's exclusion.

### Modified Capabilities

None. The Client's MPRIS behavior, the art-cache key rules, and the transport-resolution rules for
MPRIS commands stay as specified today; the fallback publisher upholds the same rules.

## Impact

- `crates/mbv-desktop/src/mpris.rs`: a fallback entry that serves the same interfaces, a
  claim/watch loop gated on the attached-Client count, and a pure claim-decision state machine
  testable without D-Bus.
- `src/local_daemon.rs`: the Local role spawns the fallback from its existing
  `on_player_ready` hook, which already receives the Player status, the transport sender, and the
  attached-Client cell. The packaged entry never runs this code path.
- `mbv_ctrl::AttachedClientsCell`: read-only reuse by the fallback loop; no change to the cell.
- Dependencies: none new or upgraded. `zbus` 5.19.0 (already pinned) supplies the name-claim flags
  the handoff needs.
