## Why

Closing a playing mpv video (directly or via quitting mbv) incurs a ~2s delay.
The dominant cause is a hardcoded `Duration::from_secs(2)` fallback in the
player run loop (`player_run_run.rs:97`) that waits for mpv's `Shutdown` event
after sending `quit`. When mpv doesn't deliver `Shutdown` promptly (common for
video), the full 2s elapses before teardown even begins. Secondary costs —
visualizer worker join, progress-thread join, and WS flush — add to that.

## What Changes

- Reduce the post-quit fallback timeout from 2s to a shorter value that still
  gives mpv a reasonable window to deliver `EndFile`/`Shutdown` cleanly.
- Skip the synchronous `ProgressGuard::stop_and_join` during quit shutdown —
  fire the stop signal without waiting; the `Stopped` HTTP report uses
  `last_valid_pos` captured independently, and Emby handles a late progress
  ping arriving after `Stopped`.
- Drop the WS flush in `report_stopped_for_shutdown` — the HTTP `Stopped` call
  is the authoritative position report; pending WS progress pings are
  unnecessary bookkeeping once playback ends.
- Parallelize the visualizer worker join with the player shutdown instead of
  blocking before it.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

(none — pure performance refactor, no behavioral change)

## Impact

- `crates/mbv-core/src/player_run_run.rs` — quit fallback timeout
- `crates/mbv-core/src/player_run_queue.rs` — `report_stop_now_or_background` shutdown path
- `crates/mbv-core/src/player_report_worker.rs` — `report_stopped_for_shutdown` WS flush
- `src/app/run_loop_events_teardown.rs` — teardown ordering (visualizer ∥ player)
- `src/app/visualizer.rs` — extract worker handle for concurrent join
- Test in `src/app/run_loop_events_teardown.rs` — `outer_bound` formula changes
