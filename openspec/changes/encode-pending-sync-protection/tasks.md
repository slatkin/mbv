# Tasks

Group 1 changes `mbv-queue`'s public API and breaks downstream crates until
group 2 finishes. Groups 1–2 land as ONE commit. No new lint suppression.

Final gate (end of group 2):
- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run -p mbv-queue -p mbv-player -p mbv-daemon -p mbv`

## 1. `mbv-queue` types (design D1, D2, D4)

- [ ] 1.1 In `crates/mbv-queue/src/progress.rs`, delete `ProgressState` and its
  `from_queue_item`/`apply_to_item`, and add
  `pub enum StopReportOutcome { Accepted, NotAccepted }` with
  `pub fn from_accepted(bool) -> Self`. `apply_progress_to_queue_item` stays;
  update its "Sole caller" doc to name its new callers. In
  `crates/mbv-queue/src/lib.rs`:
  - change `QueueSlot` to `{ pub slot_id, pub item, pending_sync: Option<SlotProgress> }`
    (the new field is private);
  - add `pub fn local_progress(&self) -> SlotProgress`, which returns
    `SlotProgress::from_queue_item(&self.item)`;
  - add `pub fn pending_sync(&self) -> Option<SlotProgress>`;
  - re-export `StopReportOutcome` next to `SlotProgress`.

  Verify: `rg -n "ProgressState|progress_state" crates/mbv-queue/src --glob '!tests.rs'`
  is empty.
- [ ] 1.2 Rewrite the progress mutators in `lib.rs`:
  - `apply_progress` writes only the item (via `apply_progress_to_queue_item`)
    and bumps the revision iff `!queue_items_equal`.
  - Replace `mark_progress_sync_pending` with
    `record_reported_progress(slot_id, position_ticks, played, outcome: StopReportOutcome)`.
    It applies progress as `apply_progress` does, then sets
    `pending_sync = Some(slot.local_progress())` only when `outcome` is
    `Accepted` and `slot.item` is `QueueItem::Emby`.
  - `update_slot_item` keeps `pending_sync` when
    `item.content_id() == old_item.content_id()` and sets it to `None`
    otherwise.
  - `merge_fetched_slot` keeps its branch structure. The active branches read
    `slot.local_progress()` from the old item before replacing it, then write it
    into the new item. `should_protect_missing_slot` reads the new field.
  - `set_slot_progress_by_index` reads `local_progress().played`.

  Verify: `rg -n "pending_sync\s*=" crates/mbv-queue/src/lib.rs` shows exactly
  three assignment sites: the arm in `record_reported_progress`, the clear in
  `merge_fetched_slot`, and `update_slot_item`.
- [ ] 1.3 In `crates/mbv-queue/src/tests.rs`, move the existing pending and
  refresh tests (`pending_progress_sync_*`, `active_pending_progress_*`,
  `watched_state_confirmation_requires_exact_match`,
  `refresh_cannot_prune_active_or_pending_sync_slots`) from
  `apply_progress` + `mark_progress_sync_pending` to
  `record_reported_progress(.., StopReportOutcome::Accepted)`. Move their
  `progress_state` reads to the accessors. Keep their assertions unchanged:
  they already own the confirm, stale-hold, played-mismatch and no-prune
  scenarios of the spec delta.

  Add two tests:
  - a named `#[case]` table `record_reported_progress_arms_only_accepted_emby_reports`
    for the arming contract. Cases: Emby + `Accepted` → `Some`;
    Emby + `NotAccepted` → `None`; Feed + `Accepted` → `None`;
    Audiobookshelf episode + `Accepted` → `None`. In every case the slot's
    `local_progress()` equals the recorded values.
  - `update_slot_item_keeps_protection_only_for_same_content` (regression,
    issue #842), with the same-content and different-content cases as a
    two-row `#[case]` table.

  Verify: `cargo nextest run -p mbv-queue` passes.

## 2. Callers (design D3)

- [ ] 2.1 `crates/mbv-player/src/owner_state.rs`:
  `apply_completion_progress(slot_id, position_ticks, played, outcome: StopReportOutcome)`
  calls `self.queue.record_reported_progress(...)`. Update its in-file test to
  pass `StopReportOutcome::Accepted`. Verify: `cargo check -p mbv-player --tests`
  is clean.
- [ ] 2.2 In `crates/mbv-daemon`:
  - `event_loop/player_events.rs`: `handle_track_completed` and the Stopped
    handler destructure `progress_report_accepted`.
  - `run.rs`: `apply_track_completed_observation` and `apply_stopped_observation`
    take it (as `StopReportOutcome`, converted at the handler with
    `from_accepted`) and pass it to `apply_completion_progress`.
  - `control_queue.rs:39` reads `slot.local_progress().position_ticks`.
  - Update existing callers in `src/tests/` to pass an outcome. Use
    `adoption.rs` accessors in place of `progress_state`.
  - Add `stopped_observation_without_accepted_report_does_not_protect_slot`
    (regression, issue #842), next to the existing `apply_stopped_observation`
    tests. It asserts `pending_sync()` is `None` and `local_progress()` holds
    the observed position.

  Verify: `cargo nextest run -p mbv-daemon` passes.
- [ ] 2.3 In `src/app/dispatch/session/player_event.rs`, replace the
  `apply_progress` + `if progress_report_accepted { mark_progress_sync_pending }`
  pair in the Stopped and TrackCompleted arms with one
  `record_reported_progress(slot_id, position, played, StopReportOutcome::from_accepted(progress_report_accepted))`
  each. Update `src/app/tests/queue/consume.rs:57` to `slot.pending_sync()`.
  Then run the final gate and commit groups 1–2 as one commit referencing #842.
  Verify: the gate is green and
  `rg -n "mark_progress_sync_pending|progress_state" src crates` returns only
  comment text in `crates/mbv-player` (`run/queue.rs`, `tests/session.rs`).
  Reword those comments to name `record_reported_progress`.

## 3. Docs

- [ ] 3.1 Rewrite `docs/invariants/02-pending-sync-protection.md` to cover only
  what types don't enforce:
  - the player-thread optimistic-accept policy behind the wire bool
    (`crates/mbv-player/src/run/queue.rs`);
  - no expiry for a snapshot that is never confirmed;
  - non-report `apply_progress` writers leave protection untouched.

  Drop failures 1, 3 and 4 and the "Set/Coherence path" prose that the types
  now enforce. Sync the spec delta into
  `openspec/specs/unified-playback-queue/spec.md`, then tick invariant 02 on
  #810. Verify: `openspec validate encode-pending-sync-protection` passes, and
  the doc no longer mentions `ProgressState` or `mark_progress_sync_pending`.
