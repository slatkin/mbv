## Context

In stay-alive mode the client's `self.player` is a ctrl proxy to the home Local daemon, so `PlayerProxy::is_remote()` is always true. Queue-edit send gates in `remove_from_queue` (`src/app/queue_actions.rs`), `apply_queue_move_by_slot`, and `sync_playback_queue_items_after_append` (`src/app/queue_scope.rs`) treat `is_remote()` as "an owner holds this queue", which is false when playback was submitted to an attached Emby session (`submit_attached_sequence`) or dispatched to a cast receiver — both bypass the daemon entirely and leave its Bound queue stale. The daemon then rejects slot-addressed edits ("slot not found"), and `reject_command`'s echo is adopted wholesale by the client (`PlayerEvent::UnifiedQueueUpdated` → `set_unified_state` + `queue_source = source`), replacing the visible queue with the daemon's snapshot.

The app already resolves "where playback goes" in one place: `App::playback_target()` (`src/app/actions.rs`) → `PlaybackTarget::Local | Remote(session) | Cast`. Queue edits must use the same resolution instead of inferring an owner from `is_remote()`.

## Goals / Non-Goals

- Goals: one shared predicate for "may this edit be sent to the Player owner"; the three edit kinds (remove, move, append) gated by it; the visible queue and `queue_source` untouched by daemon rejection echoes in the session/cast case.
- Non-Goals: changing the adopt-on-rejection rule; changing direct-remote (ctrl) queue management; mirroring session/cast-played queues back into the daemon's Bound queue; any ctrl-protocol or daemon-side change.

## Decisions

- **D1: Gate on `playback_target() == Local`, per edit site.** Each of the three send sites adds the condition `matches!(self.playback_target(), PlaybackTarget::Local)` to its existing `active || scope == Remote || is_remote()` gate. Alternative considered: a queue-scope-resolution change so an attached session flips the playing scope — rejected, because `QueueScope` resolution is about which *panel view* is shown and direct-remote handling, not about which external target received the submission; reusing `playback_target()` matches how `QueuePlayCursor` already routes play/jump/stop.
- **D2: Direct-remote scope keeps its own arm.** `scope == QueueScope::Remote` sends regardless of the new predicate — the remote owner really does hold that queue (`has_direct_remote_queue()`).
- **D3: No compensating daemon sync.** When a session is attached, edits are not replayed to the daemon later either; the generation fence (`replace_playback_queue` bump + cold-start check in `QueuePlayCursor`) already forces a full resubmit the next time playback goes through the daemon, so the daemon self-heals on next local play.
- **D4: Pure predicate, no new state.** The gate is a derived read of existing state; no flags, no invalidation, no persistence.

## Risks / Trade-offs

- [Daemon is actively playing while a session is attached and the user edits the queue] → The daemon's playing queue is a different occurrence from the edited one; not sending is correct, and the daemon's own playback is unaffected. Transport controls already resolve their target the same way.
- [Future edit kind added without the gate] → Mitigate by keeping the three sites' gate shape identical and covering each with a unit test asserting no command is emitted under an attached session.

## Migration Plan

No data or protocol migration; single binary change, rollback by revert.

## Open Questions

None.
