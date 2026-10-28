# Proposal

## Why

The F3 Sessions sidebar has one verb, connect, but it does two different things. On an Emby Session or a Cast receiver, connect makes the device an output of the local Owner, and the queue stays the same. On an mbv or mbvd peer, connect makes the Client control the peer Owner, and the queue panel changes to the peer queue. On disconnect it changes back. The user cannot see which verb they used, so the queue seems to change at random. The Local/Remote queue scope still exists in code, but commit `b17763718` removed its visible pills, so the split is now hidden.

## What Changes

- Each Player owner keeps its own queue. Two verb pairs act on peers:
  - **Connect / Disconnect** move only control. The Client sees and edits the queue of the connected Player owner. The music stays where it plays. Disconnect leaves the peer playing.
  - **Hand off / Take** copy a queue between Player owners and move the music. Hand off copies the local queue and position to the peer, starts it there, stops local playback, and connects. Take copies the peer queue and position to the local Owner, starts it here, stops the peer, and disconnects when connected. Take also works from an F3 row with no connection.
- Hand off and Take ask for confirmation before they replace a populated target queue. They never stop the source until the target plays.
- A new additive ctrl capability lets a whole-queue replacement carry a start position for its start slot. Hand off and Take use it, so the item continues where it was. A peer without the capability refuses Hand off with a clear message.
- The queue panel always names the Player owner whose queue it shows, local or peer.
- **BREAKING (user-facing):** the Local/Remote queue scope as user state is removed: the `[` and `]` scope keys, the scope indicator semantics, and scope switching on play. The queue panel shows the queue of the connected Player owner, or the local Owner queue when no peer is connected.
- MPRIS follows the connected Player owner, as it does today. The Tray stays local by design.
- Out of scope: Library routes, the Tray, and Emby Session and Cast rows (their activation stays as it is).

## Capabilities

### New Capabilities

- `queue-handoff`: Hand off and Take between the local Owner and a directly reachable peer Player owner. It covers the copy, the start position, the stop order, the confirmation, the admission and failure behavior, and the resulting connection state.

### Modified Capabilities

- `sessions-sidebar`: F3 offers Hand off and Take on a target with a direct endpoint.
- `queue-scope-remote-handoff`: removed. No user-selectable queue scope exists to switch.
- `remote-queue-disconnect`: disconnect returns to the local Owner queue, leaves the peer playing, and refers to no scope pill or scope value.
- `unified-playback-queue`: queue edits reach the Player owner whose queue is shown, not a Local or Remote scope.
- `queue-canonical-list`: the Queue projection and its active-index pushes are keyed by the shown Player owner, not by a viewed scope.
- `queue-playback-panel`: the queue panel names its Player owner. The header rule no longer refers to a viewed scope.
- `playback-resume`: the start slot of Hand off or Take starts at the carried position, not at the per-medium source.
- `ctrl-protocol`: an additive capability for a whole-queue replacement with a start position.

## Impact

- `crates/mbv-ctrl`: new capability and an optional start-position field on `UnifiedQueueReplace`. No `CTRL_PROTOCOL_VERSION` bump. New fixtures under `protocol/ctrl/v10/`.
- `crates/mbv-daemon`: owner handling of the start position for the start slot.
- `crates/mbv-player`: a `Handoff` resume source value for that start slot.
- `crates/mbv-remote-player`: send the field and expose the peer capability.
- `crates/mbv-components` (`sessions.rs`): Hand off and Take keys. `crates/mbv-ui-msg`: new `ShellRequest` variants.
- `src/app/dispatch/session/`: Hand off and Take orchestration in the Client. `src/app/state/queue_scope.rs` and about 30 files that use `QueueScope`: remove the user-selectable viewed scope.
- `crates/mbv-components/src/queue*` and the queue footer: owner label, no scope keys.
- Docs: `CONTEXT.md` (*Queue handoff*, *Take*, *Queue scope* retired, *Direct remote control* updated) and a new ADR that records the copy model and updates ADR 0012.
