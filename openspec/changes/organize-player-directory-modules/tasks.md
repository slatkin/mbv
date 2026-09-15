## 1. Create `player/` directory module

- [x] 1.1 Create `crates/mbv-core/src/player/` directory. `git mv player.rs player/mod.rs`. Add submodule declarations for every `player_*.rs` file (stripping the `player_` prefix). Re-export all public items so `crate::player::*` paths remain valid. Update `lib.rs` module declarations. Verify: `cargo check -p mbv-core`.

- [x] 1.2 `git mv` each `player_*.rs` into `player/` with its prefix stripped (e.g. `player_types.rs` → `player/types.rs`, `player_runtime.rs` → `player/runtime.rs`, `player_sources.rs` → `player/sources.rs`, etc.). Update all intra-crate `use` paths. Verify: `cargo check -p mbv-core && cargo check -p mbv`.

- [x] 1.3 Create `player/run/` submodule. `git mv` `player_run_state.rs` → `player/run/mod.rs` (or `player/run/state.rs` with a new `mod.rs`), `player_run_run.rs` → `player/run/loop.rs`, `player_run_commands.rs` → `player/run/commands.rs`, `player_run_events.rs` → `player/run/events.rs`, `player_run_queue.rs` → `player/run/queue.rs`, `player_run_types.rs` → `player/run/types.rs`. Wire mod declarations. Verify: `cargo check -p mbv-core`.

- [x] 1.4 Move test files: `git mv` `player_tests_*.rs` and `player_proxy_tests.rs` into `player/tests/`. Declare as `#[cfg(test)]` modules. Verify: `cargo nextest run -p mbv-core` passes all existing player tests.

## 2. Split `player_runtime_controller.rs`

- [x] 2.1 Split into `player/controller.rs` (Player struct, construction, lifecycle, command forwarding, QuitHandle, WakeupWriter — everything through `headless_for`) and `player/submit.rs` (submit_queue, play, play_queue, queue_append, assign_slot_ids). Both are `impl Player` blocks. Verify: `cargo check -p mbv-core && cargo check -p mbv`.

## 3. Create `playback/` directory module

- [x] 3.1 Create `crates/mbv-core/src/playback/` directory. `git mv` `playback_queue.rs` → `playback/queue.rs`, `playback_queue_items.rs` → `playback/queue_items.rs`, `playback_execution_sequence.rs` → `playback/execution_sequence.rs`, `playback_transition.rs` → `playback/transition.rs`. Create `playback/mod.rs` with re-exports preserving `crate::playback_queue::*` etc. paths. Move test files (`playback_queue_tests*.rs`) into `playback/tests/`. Update `lib.rs`. Verify: `cargo check -p mbv-core && cargo nextest run -p mbv-core` — all playback tests pass.

## 4. Create `remote_player/` directory module

- [x] 4.1 Create `crates/mbv-core/src/remote_player/` directory. `git mv` `remote_player.rs` → `remote_player/mod.rs`, `remote_player_connect.rs` → `remote_player/connect.rs`, `remote_player_tests.rs` → `remote_player/tests.rs`. Update `lib.rs`. Verify: `cargo check -p mbv-core && cargo nextest run -p mbv-core`.

## 5. Final verification

- [ ] 5.1 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`. Fix any lint or format issues introduced by the moves. Verify: both commands exit 0.
