# Tasks

## 1. Inject the owner-queue store

- [x] 1.1 Change `persist_stay_alive_owner_queue` (`daemon_control_queue.rs:3`) to take a store `&mut dyn FnMut(&StayAliveQueueState) -> Result<(), String>` instead of calling `config::save_stay_alive_queue_state` directly. Update both call sites in `run_with_options` (end-of-pass persist and Shutdown) to pass `&mut config::save_stay_alive_queue_state`. Verify: `cargo check -p mbv-core`.

## 2. Extract `DaemonLoop`

- [x] 2.1 Create `crates/mbv-core/src/daemon_loop.rs` with `enum LoopFlow { Continue, Shutdown }` and `struct DaemonLoop` holding every loop-read local listed in design.md Context, plus `store: Box<dyn FnMut(&StayAliveQueueState) -> Result<(), String>>`. Wire it the same way the other `daemon_*.rs` files are wired (check how `daemon_control_queue.rs` is included).
- [x] 2.2 Move the `match ev { ... }` body and the end-of-pass persist into `DaemonLoop::handle_event(&mut self, ev: DaemonEvent) -> LoopFlow`. Each `continue` becomes `return LoopFlow::Continue`. Shutdown keeps its persist/notify/flush/stop/join steps and returns `LoopFlow::Shutdown`, without the pid removal or `process::exit`.
- [x] 2.3 Move the keepalive/capabilities timer work into `DaemonLoop::tick(&mut self)`.
- [x] 2.4 Reduce `run_with_options` to: setup, building `DaemonLoop`, and `loop { tick; recv_timeout; if handle_event(ev) == Shutdown { remove pid_file; process::exit(0) } }`. Move `apply_track_completed_observation`, `apply_stopped_observation`, `apply_queue_enriched` and related helpers to `daemon_loop.rs` if that is needed to keep both files at 800 lines or fewer. Update the `use` list in `daemon_tests.rs`. Verify: `cargo check -p mbv-core`, `wc -l` of both files is 800 or fewer, and there is no `owner_queue_dirty` left in `daemon_run.rs`.
- [x] 2.5 In the generic `DaemonEvent::Player(pe)` arm, bind the `PlayerEvent::Stopped` fields once and remove the repeated `match &pe` and the `unreachable!()`. Behaviour unchanged. Verify: `cargo nextest run -p mbv-core` stays green.

## 3. Unit tests (`daemon_loop_tests.rs`)

- [x] 3.1 Add a `test_loop()` builder: `cold_player()`, `CtrlClients::default()`, `role = DaemonRole::Local`, and a store closure that pushes snapshots into a shared `Rc<RefCell<Vec<_>>>`. No real config or state dirs, no sockets.
- [x] 3.2 TrackCompleted: a current-run completion consumes or advances the slot and persists exactly one snapshot. A stale run identity leaves the queue alone and persists nothing.
- [x] 3.3 Stopped: a current-run Stopped that updates the queue persists. A stale-run Stopped persists nothing. With a pending idle load whose `stopped_run` matches, a successful commit replaces the queue and persists once. A Stopped for a different run cancels the pending load.
- [x] 3.4 Ws: with a matching `emby_runtime.generation`, a non-network `WsEvent` (for example `Stop`) persists. With a mismatched generation, nothing is persisted.
- [x] 3.5 PlaybackResolved: a resolved playback replaces the queue/source and persists. If the arm has a non-dirty path (stale request), cover that it does not persist.
- [x] 3.6 Role gate: the same dirty event with `role != Local` persists nothing.

## 4. Gates

- [x] 4.1 `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-core` all clean.
