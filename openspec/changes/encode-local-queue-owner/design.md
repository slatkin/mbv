# Design

## Context

See proposal.md for why this change exists. These are the current call sites
on `main` (file paths have moved since issue #770 was filed):

| Site | Stay-alive behaviour today |
|---|---|
| `state/queue_scope.rs` `local_queue_is_owner_queue` | always true (no generation fence) |
| `state/queue_scope.rs` `stamp_queue_generation` | no-op |
| `state/queue_scope.rs` `set_queue_source_if_not_local_daemon` | no-op |
| `state/queue_scope.rs` `replace_playback_queue` | skips the fence bump |
| `dispatch/queue/mod.rs` `execute_pending_queue_action` PlayItems | non-autostart goes to the owner idle load |
| `dispatch/queue/mod.rs` ClearQueue | also sends `player.clear_queue()` |
| `dispatch/queue/mod.rs` `save_queue_as_playlist` | reads owner lineage into `owner_queue_lineage` |
| `dispatch/queue/playlist_mutation.rs` `save_queue_state`, `save_queue_state_no_clear`, `maybe_restore_queue_state`, `restore_queue_state` | early return |
| `dispatch/run_loop/session.rs` `PlaylistCreateComplete` | separate owner-update branch + `reject_stay_alive_queue_source_update` |
| `dispatch/run_loop/session.rs` `PlaylistReplacementComplete` | goes through the no-op source writer (**bug**) |
| `dispatch/session/player_event.rs` `UnifiedQueueUpdated` | adopts source, reconciles `pending_owner_source_update` |

`remote_queue_lineage: u64` is a client-local counter that is advanced on
every Client-side queue replacement or clear. It fences async completions in
every mode. The owner-minted `QueueLineage` is what the Stay-alive process
uses to authorize source-only updates (invariant 13 §2). These are two
separate concepts. The glossary lists "client lineage" as a term to avoid.

## Goals / Non-Goals

**Goals:**
- One exhaustive `match` per behaviour that differs by owner, with no raw
  bool left.
- One field that records which queue a playlist mutation was requested
  against, shaped per owner.
- Overwrite and Save As share one completion path.

**Non-Goals:**
- Daemon side, ctrl protocol, persistence formats.
- `QueueScope::Remote` / direct-remote handling. It already has its own type.
- Renaming `local_queue_is_owner_queue`, `set_queue_source_if_not_local_daemon`
  or other methods whose names still read correctly. Only the implementations
  change.

## Decisions

### D1. `LocalQueueOwner { ThisProcess, StayAlive }`, derived, not stored

`App::local_queue_owner()` matches `player_endpoint`:
`Some(DaemonEndpoint::Local)` gives `StayAlive`, and every other value gives
`ThisProcess`. It replaces `stay_alive_owner_is_queue_authority()`. That
function is deleted, and each caller becomes a `match` with no wildcard arm.
`is_local_daemon()` stays because it has unrelated callers.

Considered but not chosen: the issue's three-variant `Bare/StayAlive/DirectRemote`.
At every site listed above, a packaged-mbvd attachment does exactly what Bare
does, so a third variant would duplicate an arm at every site with no
difference in behaviour. The name `ThisProcess` means "this process holds
the authoritative Local queue", which is true for both Bare and direct remote.

Also considered: a trait object per owner. That needs `&mut App` access from
the implementors, which makes it an indirection with no gain over a `match`.

### D2. `QueueEpoch` newtype replaces `remote_queue_lineage: u64`

`struct QueueEpoch(u64)` derives `Copy, Eq, Ord, Default` and has
`advance()`. The field becomes `App::queue_epoch`, and
`advance_remote_queue_lineage` becomes `advance_queue_epoch`. This is a
mechanical rename across about 20 sites. The newtype means an epoch can't be
compared with, or passed where the code expects, an owner `QueueLineage`.
Add **Queue epoch** to CONTEXT.md, with "remote queue lineage" and
"client lineage" listed under _Avoid_.

### D3. `QueueOrigin` is the single fence value

```rust
enum QueueOrigin {
    ThisProcess { epoch: QueueEpoch },
    StayAlive { epoch: QueueEpoch, lineage: QueueLineage },
}
```

- `App::queue_origin() -> Option<QueueOrigin>` captures the origin when a
  request is made. It returns `None` only for `StayAlive` when
  `player.as_remote().and_then(unified_queue_state)` is `None`.
- `App::origin_is_current(origin) -> bool` compares `origin.epoch()` with
  `self.queue_epoch`. The owner lineage is not checked here. The owner is
  the final judge of its own lineage (invariant 13 §2), and the client's
  last snapshot can lag behind a replacement the client itself just sent.
- The `Save`, `CreateAs` and `Replace` variants of `PlaylistMutation`, and
  their three `SessionEvent` completions, replace `queue_lineage` (and
  `CreateAs`/`PlaylistCreateComplete`'s `owner_queue_lineage`) with
  `origin: QueueOrigin`. The thread-spawn closures in `start_playlist_mutation`
  copy one field instead of two.

Considered but not chosen: using only the owner lineage under Stay-alive.
Snapshot lag would let a completion pass the client-side stale checks
(clearing dirty, running the save-before-replace continuation) after this
same Client had already replaced the queue. Both values are kept, but they
travel as one value whose shape comes from the owner.

### D4. Refuse at request time when a Stay-alive origin is missing

`save_playlist_to_emby`, `save_queue_as_playlist` and `do_overwrite_playlist`
call `queue_origin()`. On `None` they show the error toast "Stay-alive queue
not available yet" and return without enqueueing. This happens before any
Emby call, so no playlist is created and left orphaned. It replaces the
`owner_queue_lineage == None` rejection at completion time.

### D5. One source-update path: `apply_saved_playlist_source`

`App::apply_saved_playlist_source(&mut self, source, origin) -> bool`
(`true` means the source was applied or sent to the owner):

- `QueueOrigin::ThisProcess`: set `queue_source`, clear `queue_dirty`,
  `clear_local_playlist_entry_ids()`, `save_queue_state()`. This is what the
  Bare branches of both completions do today.
- `QueueOrigin::StayAlive { lineage, .. }`: `remote.update_queue_source(source, lineage)`.
  On success it sets `pending_owner_source_update = Some((source, lineage))`.
  On failure it shows "Could not update the Stay-alive queue source" and
  returns `false`.

`PlaylistCreateComplete` and `PlaylistReplacementComplete` both call it
after their existing `origin_is_current` and source-playlist checks. The
success toast ("Saved as playlist …") stays in the Save As arm, and only
fires when the call returns `true` and the origin is `ThisProcess`.
Stay-alive keeps today's behaviour, where the owner snapshot confirms the
save without a toast. Both arms then call `finish_playlist_mutation`
unconditionally, as they do today. `reject_stay_alive_queue_source_update`
is deleted.

The origin's variant is enough to choose the arm, because the origin was
captured from `local_queue_owner()` when the request was made, and the
owner kind can't change while the mutation is in flight without advancing
the epoch. Attach, detach and switch all call `advance_queue_epoch`, which
makes the origin stale.

### D6. Owner-snapshot adoption becomes one method

The `if stay_alive { … }` block in `player_event.rs` after
`set_unified_state` moves into `App::adopt_owner_source(&mut self, unified)`.
It matches `local_queue_owner()`: `ThisProcess` does nothing, and `StayAlive`
runs the existing body. The redundant bool check goes away.

### D7. Persistence gate collapses

`save_queue_state`, `save_queue_state_no_clear` and `restore_queue_state`
each open with
`match self.local_queue_owner() { LocalQueueOwner::StayAlive => return, LocalQueueOwner::ThisProcess => {} }`.
`maybe_restore_queue_state` is deleted. Its one production caller
(`shell/run/mod.rs`) and its two test callers call `restore_queue_state`
directly, since that function already applies the same guard.

## Risks / Trade-offs

- [Overwrite under Stay-alive now leaves the queue dirty until the owner's
  snapshot arrives. It used to clear dirty immediately.] → This matches Save
  As and invariant 13. A rejected update keeps quit-save eligibility, which
  is the intended outcome.
- [queue/mod.rs is 793 lines.] → Net change there is negative (the
  `owner_queue_lineage` block becomes one `queue_origin()` call). The
  pre-push line check still applies.
- [Owner kind flips while a mutation is in flight.] → Covered by the epoch
  advance on attach, detach and switch (D5). A stale origin is discarded
  before `apply_saved_playlist_source` is reached.
