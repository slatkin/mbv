# Design

## Context

See proposal.md (Why). Current shape, all in `crates/mbv-queue`:

- `QueueSlot { pub slot_id, pub item, pub progress_state: ProgressState }` and
  `ProgressState { pub local: SlotProgress, pub pending_sync: Option<SlotProgress> }`
  (`progress.rs`).
- Every mutation keeps `local` equal to the progress embedded in `item`:
  `apply_progress` writes both, `update_slot_item` and every
  `merge_fetched_slot` branch rebuild one from the other. `local` is a
  hand-synced mirror of `SlotProgress::from_queue_item(&item)`. That round
  trip is exact for all kinds: Audiobookshelf `played()` reads
  `played || is_finished`, and `apply_progress_to_queue_item` writes both.
- `mark_progress_sync_pending` is public and has three callers. Two are in the
  Shell (`src/app/dispatch/session/player_event.rs`, Stopped and TrackCompleted
  arms), each gated on `progress_report_accepted`. The third,
  `PlayerOwnerState::apply_completion_progress`
  (`crates/mbv-player/src/owner_state.rs`), is ungated and was added by #746 for
  the cold-adopt enrichment race. It is reached from the daemon's
  `apply_track_completed_observation` and `apply_stopped_observation`
  (`crates/mbv-daemon/src/run.rs`), whose handlers
  (`event_loop/player_events.rs`) drop the event's `progress_report_accepted`
  with `..`.
- Every `PlayerEvent` built with `progress_report_accepted: false` is a failure
  stop that made no report: mpv startup, audio-output or load failure, failed
  advance, panic (`mbv-player` `submit.rs`, `run_loop.rs`, `termination.rs`,
  `queue_advance.rs`, `commands/load.rs`). The ordinary background stop reports
  `Accepted` optimistically (`run/queue.rs`). So the owner's ungated arm
  protects progress that nobody reported.
- The snapshot is never serialized: `QueueSlot` has no serde derive, and ctrl
  snapshots use `UnifiedQueueSlot`. Changing the type has no wire or
  persistence impact.

## Goals / Non-Goals

**Goals:**
- Outside `mbv-queue`, protection can be read but never set or cleared
  directly.
- Applying reported progress and deciding protection are one call that must
  name the report outcome.
- Only Emby slots can be armed.
- A slot's position is stored once.

**Non-Goals:**
- Expiry for a snapshot that is never confirmed. It stays the documented
  residual.
- Replacing the ctrl wire's `progress_report_accepted: bool`, or the
  player-thread optimistic-accept policy.
- Changing non-report progress writers (`apply_progress` callers: library
  UserData reconcile, hydration, Feed store, Audiobookshelf progress, relative
  step). They keep today's behaviour and leave protection untouched.
- Making protection unrepresentable on non-Emby kinds by restructuring
  `QueueSlot` into per-kind content. `slot.item` is read across the workspace,
  and a checked transition behind a private field gives the same guarantee for
  far less churn.

## Decisions

**D1. Derive position; remove `ProgressState`.** `QueueSlot` becomes
`{ pub slot_id, pub item, pending_sync: Option<SlotProgress> }`. The new field
is private, so no crate outside `mbv-queue` can build a `QueueSlot` literal.
Add these read accessors:
- `QueueSlot::local_progress() -> SlotProgress`, which is
  `SlotProgress::from_queue_item(&self.item)`. `from_queue_item` becomes the
  implementation behind it and can stay `pub(crate)`.
- `QueueSlot::pending_sync() -> Option<SlotProgress>`.

The public fields `slot_id`/`item` stay. External code only reaches a slot
through `&QueueSlot` or a detached owned value (`into_slots`, `RemoveSlotResult`),
so it cannot mutate a live queue's slot.

`apply_progress` then only writes into `item`, and its change check reduces to
`!queue_items_equal`.

In the active branch, `merge_fetched_slot` reads `local_progress()` from the old
item before the swap and writes it into the fetched item.

*Alternative:* keep `local` but make it private. Rejected: it still stores
the same fact twice, which was the doc's failure 4.

**D2. One reporting transition.** Replace the public `mark_progress_sync_pending`
with:

```rust
pub enum StopReportOutcome { Accepted, NotAccepted }
impl StopReportOutcome { pub fn from_accepted(accepted: bool) -> Self }

pub fn record_reported_progress(
    &mut self, slot_id, position_ticks, played, outcome: StopReportOutcome,
) -> QueueMutationResult<()>
```

It applies progress exactly as `apply_progress` does. Then, when `outcome` is
`Accepted` and the slot's item is `QueueItem::Emby`, it snapshots
`local_progress()` into `pending_sync`. Otherwise it leaves `pending_sync`
unchanged. `pending_sync` is assigned only in `record_reported_progress` (arm),
in `merge_fetched_slot`'s confirm branch (clear), and in `update_slot_item`
(D4).

*Alternative:* a proof token only `StopReport::Accepted` can mint. Rejected:
acceptance crosses the ctrl wire as a bool. A token minted from that bool at
the receiver proves nothing more than an enum does. The enum's value is that
the one call must state the outcome.

**D3. The owner gates on the report.** `PlayerOwnerState::apply_completion_progress`
gains an `outcome: StopReportOutcome` parameter and calls
`record_reported_progress`. The daemon's `handle_track_completed` and stopped
handler destructure `progress_report_accepted` and pass it through
`apply_track_completed_observation` / `apply_stopped_observation`. This is the
spec's behaviour change.

The #746 race is still covered in two ways. The ordinary stop and natural
advance report `Accepted`, including optimistically on the background path.
The monotonic enrichment floor in `merge_fetched_slot` (inactive branch) keeps
a greater stored position when the stop was not accepted.

*Alternative:* keep the owner ungated. Rejected: that is two rules for one
queue concept. It also arms protection on failure stops that reported nothing.

**D4. `update_slot_item` keys protection on content identity.** If the
replacement's `content_id()` equals the old item's, `pending_sync` is kept.
Callers `dispatch/queue/playlist.rs` (clears `playlist_item_id`) and
`run_loop/teardown.rs` (writes `last_valid_pos`) both keep the same content.
Otherwise it is cleared, because the snapshot described content the slot no
longer holds. Position needs no rebuild (D1).

*Alternative:* reject a content-changing replacement. Rejected: no caller
needs it, and it would add a result variant for an unused path.

## Risks / Trade-offs

- [Daemon behaviour change on failure stops] A stop the player did not report
  no longer shields the slot, so the next enrichment may adopt server
  progress. → This matches the Shell. The inactive-branch monotonic floor keeps
  a greater stored position unless the server reports the item as played. A
  spec scenario and a daemon unit test pin it.
- [Readers of `slot.progress_state.local` elsewhere] → The compiler forces
  every site: `crates/mbv-daemon/src/control_queue.rs:39`,
  `src/app/tests/queue/consume.rs`,
  `crates/mbv-daemon/src/tests/basic/adoption.rs`, and `mbv-queue`'s
  `tests.rs`. Each moves to `local_progress()` / `pending_sync()`.
- [`set_slot_progress_by_index` test helper in the public API] It routes
  through `apply_progress` and never arms protection. Leave it as is.

## Migration Plan

None. The data is in-memory only, with no wire or persistence change.
