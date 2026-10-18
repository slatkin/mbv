# Proposal

## Why

When a Client connects to an Emby Session in F3, the TUI sends the Emby items of the queue to the device as one list (`session_play_items`). From that point, the device app owns its own copy of the queue. mbv cannot follow it: Feed and Audiobookshelf slots are dropped from the list, next and previous match the device item to a queue row by content id, and nothing advances or records completion in the canonical queue. The Session state also lives in the TUI (`connected_session_id`, a 1 s `/Sessions` poll, position extrapolation). So a second Client does not see it, and control stops when the TUI closes (#945).

The Cast output (#944, PR #947) fixed the same problem for Cast receivers. The `owner-playback-output` capability is already written for any "remote receiver". This change makes an Emby Session the second remote output.

## What Changes

- **An Emby Session becomes a Playback output of the Owner.** `PlaybackOutput` gets an Emby Session variant. The Owner holds the queue and the active slot. It sends the active slot and the next slot to the device, never more. When the device moves to the next slot by itself, the Owner appends the slot after it. Without the next slot on the device, the device app autoplays its own next episode.
- **A connect in F3 selects the output.** Activation of an Emby Session row sends `SelectPlaybackOutput`. The Owner hands the active slot off to the device at its position (the same D10 hand-off as Cast). There is no watch-only mode. If a Session offers Direct remote control but that connect fails, the TUI reports the failure and selects the Session as the output.
- **The Owner observes the Session.** The Owner polls `/Sessions` every 1 s while the Session is the output. It presents the position, pause state, volume, and tracks to every Client through its normal status broadcast.
- **The Owner infers a natural end.** Emby reports no end reason. The Owner counts an end when the Session goes idle after it played the loaded item and the estimated position is within 10 s of the item runtime. A stop in the last 10 s therefore counts as an end. That is a stated limit.
- **The Owner infers a load failure.** If the device does not show the loaded item within 30 s, the Owner reports the failure and does not advance.
- **The Owner sends no progress reports for the Session.** The device reports its own progress to Emby.
- **Only Emby items the device can play reach it.** A Feed or Audiobookshelf slot, or a video on a Session that advertises audio only, cannot play on this output. The Owner reports it and steps past it on an advance, as Cast does for an uncastable slot.
- **Reattach after a device app restart.** The Owner keys the output on the Emby `DeviceId`. When the session id changes, it attaches to the new session of the same device.
- **Launch never selects a plain Emby Session.** The launch auto-reconnect restores only Direct remote control. A stay-alive Owner keeps its own output.
- **BREAKING (internal ctrl):** `OutputSelection` and `OutputState` get an Emby Session variant, behind a new capability `emby-session-output`. `CastConnection` is renamed to `OutputConnection` (wire form unchanged).
- **Removed from the TUI:** `connected_session_id`, `connected_session_state`, `RemotePlaybackTarget`, `submit_attached_sequence`, `session_jump_track`, `remote_jump_target`, the 1 s poll, the position extrapolation, the miss counting, and the populate-only load before a session play.
- **Out of scope:** *Direct remote control* (`mbv-direct-tcp-port`) and Library routes are not changed. Packaged `mbvd` keeps local playback only (#946).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `owner-playback-output`: the output can be one Emby Session. New requirements for the Emby Session end rule, load failure, admission, progress ownership, device reattach, and launch. "Only a natural end advances" states the Emby Session limit.
- `unified-playback-queue`: an Emby Session output holds only the active slot's item and the next playable slot's item. A Cast receiver holds only the active slot's item. The populate-only load scenario for an attached Emby Session is reworded to cover any Stay-alive Client, because no session play needs it.
- `non-audio-fall-through`: the eligible relation is the Owner's Emby Session output, not an Emby Session that the Client controls. Accepting the prompt returns the Owner's output to local playback.
- `queue-scope-remote-handoff`: a play on an Emby Session output is a play on the local Owner, not a hand-off to the session.
- `cast-session-control`: a watched session is no longer a Client target. A Session selection while a receiver is the output, and a cast selection while a Session is the output, are hand-offs of the active slot.

## Impact

- `crates/mbv-emby`: `SessionInfo` gets `device_id`.
- `crates/mbv-ctrl`: Emby Session variants in `OutputSelection` and `OutputState`, the `emby-session-output` capability, and the `OutputConnection` rename.
- `crates/mbv-daemon`: the Emby Session output (poll, end rule, load timeout, reattach, transport routing, status projection); a generic advance and step-over that the Cast output and the Emby Session output share; a runtime hook that enables the output in the Local daemon only.
- `src/` (TUI): F3 Session activation sends the selection; play, next, previous, and transport use the local Owner path; the attached-session code listed above is deleted. About 40 files read `connected_session_id` or `connected_session_state` today.
- Docs: `CONTEXT.md` (*Playback output*, *Session watch* retired, new *Session output*), AGENTS.md "Playback and queue authority".
- Follow-ups: the ctrl `UnifiedQueueLoadIdle` command loses its last TUI sender. Its removal is a separate change. A device that autoplays the next episode by itself shows as foreign content; adopting it as the next slot is deferred.
