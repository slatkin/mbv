## 1. Active-file TrackChanged emission (D1)

- [x] 1.1 In `player/run/commands.rs`, after the `select_active_slot` call succeeds in the active_file branch (line ~55), emit a `PlayerEvent::TrackChanged` carrying `(request_id, generation)` from `self.forced_transition` and the target `slot_id`. Use the same event construction as `on_end_file` (events.rs ~685). Remove the early `return cancel_stop` so the transition tag flows through the same settlement path. Verify: `cargo check -p mbv-core`

- [x] 1.2 Add a unit test in `daemon_tests.rs` (or `daemon_tests_abs_queue.rs`) that dispatches a JumpTo in active_file mode and asserts `observed_active_slot` advances to the target slot. The test should mock the Playback run's TrackChanged response. Verify: `cargo nextest run -p mbv-core` passes the new test

## 2. Clear observed_active_slot on queue replacement (D2)

- [x] 2.1 In `daemon_control.rs` `UnifiedQueueReplace` handler (line ~401 area, after `reset_slot_jumps`), clear `shared_queue.observed_active_slot` to `None` and call `owner.core.note_observed_active_slot(None)`. Verify: `cargo check -p mbv-core`

- [x] 2.2 Add a unit test that replaces the queue while `observed_active_slot` is `Some(old_slot)` and asserts it becomes `None` after replacement. Verify: `cargo nextest run -p mbv-core` passes the new test

## 3. Fix mpv loading-order race (D3)

- [x] 3.1 In `player/mod.rs` `queue_load_indices`, change the first `loadfile` (start_idx, "replace") to use the `no` append flag instead of starting playback. After all items are loaded (inserts + appends), set `playlist-pos` to the correct index to start playback. Remove `reassert_queue_layout` if it becomes unnecessary — or keep it as a safety net that should now be a no-op. Verify: `cargo check -p mbv-core`; add a log assertion or instrument to confirm `reassert_queue_layout` no longer detects a mismatch

- [x] 3.2 Test with a multi-item queue at a non-zero start index. Verify: no `queue layout mismatch` log line appears during loading; playback starts at the correct item

## 4. Instrumentation for settle matching (D4, deferred)

- [ ] 4.1 Add `target: "transition"` log lines in `settle()` when the dual match fails: log the expected `(request_id, target)` vs the observed `(request_id, slot)`. This will tell us whether the dual match is rejecting correct observations in practice. Verify: `cargo check -p mbv-core`; the log lines appear in debug runs when Next is pressed during active playback
