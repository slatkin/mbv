## 1. Split `src/app/mod.rs` — extract channel draining

- [x] 1.1 Create `src/app/run_loop_drains.rs` and move `drain_notif_actions`, `drain_search_results`, and `drain_session_events` methods from `mod.rs` into it.
- [x] 1.2 Add `mod run_loop_drains;` declaration to `src/app/mod.rs`.
- [x] 1.3 Update any `use` statements in `mod.rs` and other files that reference the moved methods.
- [x] 1.4 Run `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace` to verify the split.

## 2. Split `crates/mbv-core/src/remote_player.rs` — extract connection setup

- [x] 2.1 Create `crates/mbv-core/src/remote_player_connect.rs` and move `DaemonEndpoint`, `ControlStream`, `perform_handshake`, and the `connect_endpoint` method into it.
- [x] 2.2 Add `mod remote_player_connect;` declaration to `crates/mbv-core/src/lib.rs` (or the appropriate parent module).
- [x] 2.3 Update `use` statements in `remote_player.rs` to import types from `remote_player_connect`.
- [x] 2.4 Run `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace` to verify the split.

## 3. Split `crates/mbv-core/src/config_types_paths.rs` — extract path helpers

- [x] 3.1 Create `crates/mbv-core/src/config_paths.rs` and move all path helper functions (`config_dir`, `cache_dir`, `state_dir`, `queue_state_path`, `library_position_state_path`, `save_queue_state`, `load_queue_state`, `clear_queue_state`, `last_remote_connection_path`, `save_last_remote_connection`, `load_last_remote_connection`, `save_library_position_state`, `load_library_position_state`, `migrate_to_state`, `osc_script_path`, `prefs_path`, `osc_fonts_dir`, `runtime_dir`, `mpv_ipc_path`, `mpv_config_dir`, `control_socket_path`, `token_cache_path`, `config_path`) into it.
- [x] 3.2 Add `mod config_paths;` declaration to `crates/mbv-core/src/lib.rs` (or the appropriate parent module).
- [x] 3.3 Update `use` statements in `config_types_paths.rs` and other files that reference the moved functions.
- [x] 3.4 Run `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace` to verify the split.

## 4. Final verification

- [x] 4.1 Run `make check-code-file-lines` to confirm all governed files are at or below 800 lines.
- [x] 4.2 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` to confirm no new warnings.
- [x] 4.3 Confirm the three split files (`mod.rs`, `remote_player.rs`, `config_types_paths.rs`) are each below 650 lines after extraction.
