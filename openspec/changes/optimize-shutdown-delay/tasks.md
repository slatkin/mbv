## 1. Player thread shutdown (mbv-core)

- [x] 1.1 Reduce mpv quit fallback from 2s to 200ms in `player_run_run.rs:97` — change `Duration::from_secs(2)` to `Duration::from_millis(200)`. Verify: `cargo check -p mbv-core`
- [x] 1.2 In `report_stop_now_or_background` (`player_run_queue.rs:46–49`), skip `progress.stop_and_join()` during shutdown: send the stop signal (`stop_tx.send(())`) but don't join. Keep the non-shutdown path unchanged. Verify: `cargo check -p mbv-core`
- [x] 1.3 In `report_stopped_for_shutdown` (`player_report_worker.rs:287–290`), remove the `ws_tx.flush()` call. Verify: `cargo check -p mbv-core`
- [x] 1.4 Update or remove the `cancel_pending_quit_clears_quit_at_and_shutdown_timeout` test and any other tests that assert on the old shutdown budget or progress-join behavior. Verify: `cargo nextest run -p mbv-core`

## 2. App teardown (mbv binary)

- [x] 2.1 In `App::teardown` (`run_loop_events_teardown.rs:52–58`), move `stop_visualizer_worker()` to run concurrently with the player join: take the visualizer worker via `self.visualizer.take()` and send its stop signal before `player.stop_for_shutdown()`; join the extracted handle after signaling but before or during `player.join_or_timeout()`. Verify: `cargo check -p mbv`
- [x] 2.2 Update the `outer_bound` formula (`run_loop_events_teardown.rs:237`) to reflect the reduced player-thread budget: `quit_timeout + Duration::from_millis(200) + Duration::from_secs(1)`. Update the comment. Verify: `cargo check -p mbv`
- [x] 2.3 Update the teardown boundedness test (`run_loop_events_teardown.rs:231+`) to match the new `outer_bound`. Verify: `cargo nextest run -p mbv`

## 3. Final gate

- [x] 3.1 Full workspace check: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check && cargo nextest run --workspace`
