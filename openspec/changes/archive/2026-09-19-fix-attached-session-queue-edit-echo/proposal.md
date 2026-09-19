## Why

In stay-alive mode with an attached Emby client session, queue edits are routed to the home daemon even though playback (and the queue the user sees) belongs to the attached session. The daemon — still holding its own older Bound queue — rejects the edit ("slot not found") and its rejection echo makes the client adopt the daemon's stale snapshot wholesale: the user's loaded playlist visibly reverts to the old one while the session keeps playing. Reported live: load playlist B over A, play B's 5th video through the attached session, delete B's first item → the queue panel flips back to playlist A.

## What Changes

- Canonical-queue edits (remove, move/reorder, append) are dispatched according to the resolved playback target: they reach the Player owner only when the playback target **is** the Player owner (bare in-process player or home Local daemon).
- When the playback target is an attached Emby session or a cast receiver, queue edits stay purely local client-side edits of the canonical queue; no queue command is sent to the home daemon, so no rejection echo can overwrite the visible queue.
- No change to direct-remote (ctrl) queue management: editing a directly controlled remote owner's queue still sends slot-addressed commands to that owner.
- No change to the adopt-owner-snapshot-on-rejection rule itself: it keeps resyncing genuinely stale clients; with correct routing it is simply no longer reachable for session/cast-played queues.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `unified-playback-queue`: add a requirement that canonical-queue edits follow the playback target — an owner that does not hold the edited queue is never sent its edit commands.

## Impact

- `src/app/queue_actions.rs` (`remove_from_queue` send gate), `apply_queue_move_by_slot` send gate, `src/app/queue_scope.rs` (`sync_playback_queue_items_after_append`).
- Gate predicate: `matches!(self.playback_target(), PlaybackTarget::Local)` alongside the existing `active`/`is_remote` conditions; direct-remote scope keeps its current path.
- Tests: unit tests for the gate in the three edit kinds; no ctrl-protocol or daemon changes.
