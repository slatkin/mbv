## 1. ID newtypes

- [ ] 1.1 Define `ItemId`, `MediaSourceId`, `EmbySessionId` in a new `crates/mbv-core/src/id_types.rs`. Tuple struct, `new()`, `as_str()`, `Display`. Match `QueueSlotId` style. Re-export from `lib.rs`
- [ ] 1.2 Migrate `SessionReporter.ids` from `(String, String, String)` to `(ItemId, MediaSourceId, EmbySessionId)` in `player_runtime.rs`. Update constructor and all access sites
- [ ] 1.3 Migrate `PlaybackRun.mark_played_id` to `Option<ItemId>` and `series_id` to `ItemId` in `player_run_types.rs`. Fix callers in `player_run_events.rs`, `player_run_queue.rs`
- [ ] 1.4 Migrate API boundary functions in `api_client_sessions.rs` and `api_client_reporting.rs` to accept newtypes, call `.as_str()` at HTTP call sites
- [ ] 1.5 Compiler-driven migration: `rtk cargo check -p mbv-core` until clean, converting remaining bare-String ID sites as errors surface
- [ ] 1.6 Verify: `rtk cargo test -p mbv-core` passes, `rtk cargo clippy --workspace --all-targets` clean

## 2. Eliminate `is_queue_mode`

- [ ] 2.1 Remove `is_queue_mode: Arc<AtomicBool>` from `PlaybackRun` (`player_run_types.rs:14`) and `RuntimeController` (`player_runtime_controller.rs:70`)
- [ ] 2.2 Replace each read of `is_queue_mode.load()` with `origin == PlaybackOrigin::Queue` — in `player_runtime_controller.rs` use the controller's own `origin`, in `player_run_*` use `self.origin`
- [ ] 2.3 Remove `set_origin` helper at `player_run_queue.rs:22` (it only existed to sync the AtomicBool). Replace its call sites with direct `self.origin = origin`
- [ ] 2.4 Remove the `is_queue_mode` parameter from `PlaybackRun` construction at `player_run_queue.rs:195,237` and its threading through `RuntimeController`
- [ ] 2.5 Verify: `rtk cargo check -p mbv-core`, `rtk cargo test -p mbv-core`, `rtk cargo clippy --workspace --all-targets` clean

## 3. Drop `power` prefix

- [ ] 3.1 Rename source files: `power_widgets.rs` → `widgets.rs`, `power_home_actions.rs` → `home_actions.rs`, `input_lib_power_keys.rs` → `input_lib_keys.rs`, `power_cw_library_tab_actions.rs` → `cw_library_tab_actions.rs`. Update `mod` declarations in `mod.rs`/parent modules
- [ ] 3.2 Rename test files that carry `power` in the filename to match their renamed siblings
- [ ] 3.3 Rename `power_*` identifiers (functions, types, fields) to drop the prefix. Use word-boundary replacement — must not touch `powerline` in `render/indicators.rs`, `render_cadence.rs`, `config.rs`
- [ ] 3.4 Update `mod` and `use` statements across `src/app/` to reflect the renames
- [ ] 3.5 Verify: `rtk cargo check --workspace`, `rtk cargo clippy --workspace --all-targets`, `make check-code-file-lines` (renamed files must stay under 800 lines)

## 4. Final verification

- [ ] 4.1 Full workspace build: `rtk cargo test --workspace`
- [ ] 4.2 Manual smoke test: play a queue, skip mid-item, let an item end naturally. Confirm Emby marks items watched correctly
