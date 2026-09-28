# Proposal

## Why

A queue slot's `pending_sync` snapshot stops an Emby refresh that lands
before the server has applied a stop report from silently rewinding the
user's resume point. Every rule around it is held by convention
(`docs/invariants/02-pending-sync-protection.md`), and the conventions already
disagree. The Shell arms protection only for accepted reports. The Player
owner (`PlayerOwnerState::apply_completion_progress`, used by the Stay-alive
daemon and `mbvd`) arms it unconditionally, for any item kind. Protection
fields are public, the slot stores its position twice (`ProgressState::local`
and the item), and `update_slot_item` keeps a snapshot that may describe
content the slot no longer holds. Issue #842, child of #810.

## What Changes

- Arming protection and applying the reported progress become one queue
  transition. That transition takes the stop-report outcome, so a caller
  cannot apply without deciding about protection, or arm without an accepted
  report.
- **Behaviour:** the Player owner (daemon, `mbvd`) arms protection only when
  the stop report was accepted, matching the Shell. Today it arms on every
  completion and stop.
- **Behaviour:** protection is armed only on Emby slots. Feed and
  Audiobookshelf slots have no Emby server counterpart and are never armed.
  Today the owner arms them too.
- **Behaviour:** replacing a slot's item with different content drops that
  slot's protection. Replacing metadata of the same content keeps it.
- The protection snapshot becomes private to `mbv-queue`. Server confirmation
  during a refresh merge is the only path that clears it for a slot that keeps
  its content.
- The slot's current position is derived from its item instead of being
  stored alongside it. `ProgressState` is removed.
- `docs/invariants/02-pending-sync-protection.md` is rewritten to cover only
  what types still don't enforce (the optimistic-accept policy on the wire
  bool, and no expiry).

Out of scope: expiry for a snapshot that is never confirmed (phantom
protection). Keeping it is the existing deliberate bias toward retaining a
fresh position.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: adds the requirement that an accepted stop report
  protects that Emby slot's reported progress from a stale refresh until the
  server confirms it, for every Player owner and the Shell alike.

## Impact

- `crates/mbv-queue`: `QueueSlot`, `ProgressState` (removed), `SlotProgress`,
  `PlaybackQueue::{apply_progress, mark_progress_sync_pending (replaced),
  update_slot_item, merge_fetched_slot}`, and its tests.
- `crates/mbv-player/src/owner_state.rs`: `apply_completion_progress` takes the
  report outcome.
- `crates/mbv-daemon`: `run.rs` observation helpers and
  `event_loop/player_events.rs` pass `progress_report_accepted` through;
  `control_queue.rs` reads position from the new accessor.
- `src/app/dispatch/session/player_event.rs`: the Stopped and TrackCompleted
  arms use the combined transition.
- Tests reading `slot.progress_state` (`src/app/tests/queue/consume.rs`,
  `crates/mbv-daemon/src/tests/basic/adoption.rs`).
- No wire or persistence change: the protection snapshot is never serialized.
