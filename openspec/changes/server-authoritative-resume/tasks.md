# Tasks

## 1. Emby start positions come from Emby (design D1, D2)

- [ ] 1.1 Add `refresh_emby_resume` to `crates/mbv-player`, taking the items,
  a fetch closure `FnMut(&[String]) -> Result<Vec<EmbyItem>, EmbyError>`, and
  a delay schedule. It does one batched fetch for Emby non-audio ids only and
  makes at most 3 attempts. An `Ok` that omits a requested id counts as a
  failure for that id, and only missing ids are retried. It returns copies with
  `playback_position_ticks`/`played` taken from the server, or 0/unplayed when
  all attempts fail. Production uses `client.get_items_by_ids` and
  `[500ms, 2s]`. Verify with `cargo nextest run -p mbv-player`, adding tests
  only for these contracts (scripted closure, zero delays):
  - the fetched position replaces the item's position;
  - one failure then success uses the fetched position after 2 attempts;
  - three failures give 0 after exactly 3 attempts;
  - a missing id is retried.
- [ ] 1.2 Call it on the Player thread before start positions are decided: in
  `submit.rs` `load_queue_sources` (on `start.items`), in
  `run/commands/load.rs` before the `loadfile` loop, and in
  `run/commands/queue.rs` append before its `loadfile` loop. Use the run's Emby
  client, the one `SessionReporter` holds. Add a `ponytail:` comment at the
  load site saying that a natural mpv advance uses the load-time value, and
  that the upgrade path is fetch-and-seek on entry activation. Verify with
  `cargo check -p mbv-player`, and confirm by reading the diff that all three
  sites call it before `mpv_load_opts`/`prepare_source`.
- [ ] 1.3 In `cmd_jump_to` (`run/commands/queue.rs:12`), replace the incoming
  `resume_ticks` for an Emby video slot with the refreshed item's
  `resume_ticks_for_item`. Leave Feed and Audiobookshelf slots unchanged.
  Verify with `cargo nextest run -p mbv-player`. Fix existing jump tests that
  assumed the owner-supplied ticks for Emby, using a scripted fetch; don't add
  new ones.
- [ ] 1.4 Update `docs/invariants/06-queue-progress-application-sites.md`:
  for Emby, the re-visit seek now uses the server position fetched at jump
  time, and the canonical-queue position is used only for feeds. Verify by
  reading the doc against the 1.3 diff.

## 2. Persisted state carries no provider positions (design D3)

- [ ] 2.1 Add one `mbv-queue` function that sets the Emby
  `playback_position_ticks` and the Audiobookshelf episode/book
  `position_ticks` to 0 and leaves Feed entries unchanged. Apply it in
  `crates/mbv-config/src/state.rs` on save and on load of both `QueueState` and
  `StayAliveQueueState`. Verify by extending the existing round-trip test in
  `crates/mbv-daemon/src/tests/queue_ops/queue_persistence.rs` (or the
  `mbv-config` equivalent): an Emby item and an Audiobookshelf episode with
  positions restore at 0, and a feed entry keeps its position. Run
  `cargo nextest run -p mbv-config -p mbv-daemon`.

## 3. Owner restore refreshes Emby progress (design D4)

- [ ] 3.1 After `initialize_queue` in `crates/mbv-daemon/src/run.rs`, when
  the restored queue has Emby slots, start the same enrichment that adoption
  uses (`start_queue_enrichment` in `control/queue_setup.rs`, made visible as
  needed). Verify with `cargo check -p mbv-daemon`. If an existing adoption
  enrichment test has a seam for the fetch, extend it with the restore case;
  otherwise add no test.

## 4. Remove pending-sync protection and the max merge (design D5)

- [ ] 4.1 In `crates/mbv-queue`: delete `QueueSlot::pending_sync`, its
  accessor, `SlotProgress` confirmation, `StopReportOutcome`, the arming in
  `record_reported_progress`, the pending branch and the `max(stored)` clamp in
  `merge_fetched_slot`, and the pending clause of
  `should_protect_missing_slot`. Collapse `record_reported_progress` into the
  plain progress apply if nothing else distinguishes them. Delete the pending
  tests in `src/tests/progress.rs`, and change any max-merge test to assert the
  fetched position is taken as-is. Verify with
  `cargo nextest run -p mbv-queue`.
- [ ] 4.2 Remove `progress_report_accepted` from `PlayerEvent::Stopped`,
  `TrackCompleted` and the `mbv-ctrl` wire. Remove the code that produces it
  (`mbv-player` `run/queue.rs`, `run/events/*`, `submit.rs`, `run_loop.rs`,
  `owner_state.rs`) and the code that consumes it (`mbv-daemon`
  `event_loop/player_events.rs`, the TUI queue path under `src/app`). Delete
  the tests that only covered it (`mbv-ctrl` `tests_wire.rs` cases,
  `src/app/tests/queue/consume.rs`, `mbv-daemon` `tests/loop.rs`) and fix the
  rest. Verify with
  `cargo nextest run -p mbv-ctrl -p mbv-player -p mbv-daemon -p mbv`.
- [ ] 4.3 Delete `docs/invariants/02-pending-sync-protection.md`. Verify that
  `rg "pending.sync|progress_report_accepted|StopReportOutcome"` finds nothing
  outside `openspec/`.

## 5. Shutdown waits for the stop report (design D6)

- [ ] 5.1 Reorder `handle_shutdown`
  (`crates/mbv-daemon/src/event_loop/control_events.rs`) to: notify and flush
  clients, then `stop_for_shutdown(remaining(deadline))` and
  `join_or_timeout(remaining(deadline))`, then flush the persist queue and
  `persist_owner_queue()`, then return `SHUTDOWN`. Use an existing shutdown
  bound constant if one fits; otherwise add one named constant of about 5s.
  Verify with `cargo nextest run -p mbv-daemon` and by reading the diff
  against D6. This is a manual-check item: no test needs a live player.

## 6. Integration

- [ ] 6.1 Run `cargo fmt`, then
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo nextest run --workspace`. Verify that all are clean.
- [ ] 6.2 Manual check, done by the user: play an Emby video past 6%, quit the
  Owner (`pkill mbv` or the tray Quit), relaunch, and play the restored slot.
  It should resume at the quit position. Then advance the same item on another
  client and play it again here; it should resume at the other client's
  position.
