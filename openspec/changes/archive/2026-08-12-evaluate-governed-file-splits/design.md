## Context

PR #386 established an 800-line maximum for governed tracked files and split the previously oversized modules. The repository now has 37 governed files between 500–759 lines. Several are approaching the ceiling:

- `src/app/mod.rs` (759 lines) — main event loop and app orchestration
- `src/app/input_power_music_track_navigation_tests.rs` (756 lines) — test module
- `src/app/input_power_movie_detail_tests.rs` (738 lines) — test module
- `crates/mbv-core/src/ctrl.rs` (734 lines) — daemon control protocol
- `src/app/actions_tests_queue_state.rs` (730 lines) — test module
- `crates/mbv-core/src/remote_player.rs` (728 lines) — remote player client
- `src/app/render/album.rs` (722 lines) — album rendering

The issue asks to evaluate these files for cohesive extraction boundaries and refactor only where it improves maintainability.

## Goals / Non-Goals

**Goals:**
- Identify whether each file over 500 lines has a cohesive extraction boundary
- Split files where extraction improves maintainability without arbitrary fragmentation
- Preserve behavior, test coverage, and module/privacy semantics
- Keep every resulting governed file at or below 800 lines
- Document decisions for files that remain intact

**Non-Goals:**
- Lowering the 800-line limit
- Splitting files that are cohesive and maintainable at their current size
- Refactoring for the sake of hitting a target line count
- Changing any user-facing behavior or API

## Decisions

### 1. Test files: keep intact unless clearly fragmented

**Decision:** Test files (12 of 37) are kept intact unless they contain tests for multiple distinct functional areas that can be cleanly separated.

**Rationale:** Test files are often cohesive by nature — they test one area of functionality (e.g., `input_power_music_track_navigation_tests.rs` tests navigation in the music track view). Splitting them by arbitrary line count would fragment test identity and make it harder to find related tests. The existing test modules already follow a naming convention that groups tests by area.

**Exceptions:** If a test file contains tests for multiple unrelated areas (e.g., both queue state and library routing), it may be split along those boundaries.

**Files affected:**
- `src/app/input_power_music_track_navigation_tests.rs` (756) — keep intact
- `src/app/input_power_movie_detail_tests.rs` (738) — keep intact
- `src/app/actions_tests_queue_state.rs` (730) — keep intact
- `src/app/input_power_music_track_focus_tests.rs` (705) — keep intact
- `crates/mbv-core/src/daemon_tests.rs` (701) — keep intact
- `crates/mbv-core/src/api_tests.rs` (680) — keep intact
- `src/app/tests_feed_group_nav.rs` (653) — keep intact
- `src/app/tests_library_position.rs` (587) — keep intact
- `src/app/tests_route_state.rs` (584) — keep intact
- `src/app/action_tests.rs` (577) — keep intact
- `src/app/input_power_music_track_scope_tests.rs` (566) — keep intact
- `src/app/tests_queue_consume.rs` (557) — keep intact
- `src/app/input_resolver_handle_key_tests.rs` (556) — keep intact
- `src/app/render/list_tests.rs` (555) — keep intact
- `crates/mbv-core/src/remote_player_tests.rs` (544) — keep intact

### 2. `src/app/mod.rs` (759 lines): split event loop and channel draining

**Decision:** Extract channel-draining logic (`drain_notif_actions`, `drain_search_results`, `drain_session_events`) into a new `src/app/run_loop_drains.rs` module. Keep the main `run()` loop and terminal setup in `mod.rs`.

**Rationale:** `mod.rs` currently orchestrates the event loop, terminal initialization, signal handling, and channel draining. The channel-draining methods are distinct responsibilities that can be extracted without breaking the event loop's coherence. This reduces `mod.rs` to ~600 lines and isolates the draining logic for easier testing and maintenance.

**Extraction boundary:**
- `run_loop_drains.rs`: `drain_notif_actions`, `drain_search_results`, `drain_session_events` (estimated ~150 lines)
- `mod.rs`: `run()`, terminal setup, signal handling, remaining orchestration (~600 lines)

**Alternatives considered:**
- Extract terminal setup into `terminal.rs` — but terminal setup is tightly coupled to the event loop's lifecycle and is only ~50 lines.
- Extract signal handling — but signal handling is a single function and constants, not a cohesive module.

### 3. `crates/mbv-core/src/ctrl.rs` (734 lines): keep intact

**Decision:** Keep `ctrl.rs` intact.

**Rationale:** `ctrl.rs` defines the daemon control protocol: message types (`CtrlHello`, `PlaybackIntent`, `WireCommand`), enums (`CtrlCmd`, `CtrlEvent`), compatibility checks, and serialization. These are tightly coupled — the protocol is a single cohesive unit. Splitting message types from protocol logic would create circular dependencies or require excessive re-exports. The file is well-organized with clear sections and is not yet at risk of exceeding 800 lines.

### 4. `crates/mbv-core/src/remote_player.rs` (728 lines): split connection and command logic

**Decision:** Extract connection setup (`DaemonEndpoint`, `ControlStream`, `perform_handshake`) into a new `crates/mbv-core/src/remote_player_connect.rs` module. Keep the `RemotePlayer` struct and its command methods in `remote_player.rs`.

**Rationale:** Connection setup (endpoint parsing, TCP connection, handshake) is a distinct phase from the ongoing command/event loop. Extracting it isolates the one-time setup logic and reduces `remote_player.rs` to ~550 lines focused on the player's state machine and command interface.

**Extraction boundary:**
- `remote_player_connect.rs`: `DaemonEndpoint`, `ControlStream`, `perform_handshake`, `connect_endpoint` (estimated ~180 lines)
- `remote_player.rs`: `RemotePlayer` struct, command methods, event handling (~550 lines)

**Alternatives considered:**
- Extract command serialization — but commands are tightly coupled to the `RemotePlayer` state and would require passing the player struct around.

### 5. `src/app/render/album.rs` (722 lines): keep intact

**Decision:** Keep `album.rs` intact.

**Rationale:** `album.rs` renders the album detail view: album header, track list, action hints. These are all part of a single rendering context (`AlbumRowCtx`) and are called sequentially during the album view render pass. Splitting them would fragment the rendering logic and make it harder to understand the album view as a whole. The file is well-structured with clear helper functions and is not yet at risk of exceeding 800 lines.

### 6. `src/app/actions.rs` (662 lines): keep intact

**Decision:** Keep `actions.rs` intact.

**Rationale:** `actions.rs` contains navigation and selection actions (`playback_target`, `select`, `activate_album_folder_row`, `go_back`). These are all part of the app's navigation state machine and are tightly coupled through shared helper functions. The file is already organized with clear impl blocks and is not yet at risk of exceeding 800 lines.

### 7. `crates/mbv-core/src/config_types_paths.rs` (680 lines): split path helpers

**Decision:** Extract path helper functions (`config_dir`, `cache_dir`, `state_dir`, `queue_state_path`, etc.) into a new `crates/mbv-core/src/config_paths.rs` module. Keep the `Config` struct and `QueueState`/`LibraryPositionState` types in `config_types_paths.rs`.

**Rationale:** The file currently mixes type definitions (`Config`, `QueueState`, `LibraryPositionState`) with path resolution and persistence functions. The path helpers are a distinct concern (filesystem layout) and can be extracted without breaking the type definitions. This reduces `config_types_paths.rs` to ~400 lines and creates a focused `config_paths.rs` module (~280 lines).

**Extraction boundary:**
- `config_paths.rs`: `config_dir`, `cache_dir`, `state_dir`, `queue_state_path`, `library_position_state_path`, `save_queue_state`, `load_queue_state`, `clear_queue_state`, `last_remote_connection_path`, `save_last_remote_connection`, `load_last_remote_connection`, `save_library_position_state`, `load_library_position_state`, `migrate_to_state`, `osc_script_path`, `prefs_path`, `osc_fonts_dir`, `runtime_dir`, `mpv_ipc_path`, `mpv_config_dir`, `control_socket_path`, `token_cache_path`, `config_path` (estimated ~280 lines)
- `config_types_paths.rs`: `Config`, `QueueState`, `LibraryPositionState`, `LibraryPosition`, `QueueSource`, `LastRemoteConnection`, `TestStateDirGuard` (~400 lines)

**Alternatives considered:**
- Split `QueueState` persistence into `queue_state.rs` — but queue state persistence is tightly coupled to the path helpers and would create a third module for a narrow concern.

### 8. Remaining files (500–620 lines): keep intact

**Decision:** Keep the remaining 22 files intact.

**Rationale:** Files in the 500–620 line range are not yet at risk of exceeding the 800-line limit and are likely cohesive. Splitting them now would be premature optimization. They should be re-evaluated if they grow past 650 lines.

**Files affected:**
- `crates/mbv-core/src/daemon_core.rs` (618)
- `src/app/render/power_widgets.rs` (615)
- `src/app/library_browse_actions.rs` (607)
- `crates/mbv-core/src/api_types.rs` (596)
- `src/app/feed_actions.rs` (590)
- `src/app/render/queue.rs` (583)
- `crates/mbv-core/src/daemon_run.rs` (579)
- `crates/mbv-core/src/player_runtime.rs` (568)
- `src/app/session_connect.rs` (562)
- `src/app/library_route.rs` (547)
- `src/app/action.rs` (538)
- `crates/mbv-core/src/player_types.rs` (528)
- `src/app/render/chrome_status.rs` (524)
- `crates/mbv-core/src/ws.rs` (522)
- `src/app/render/detail.rs` (515)
- `src/app/render/mod.rs` (513)
- `crates/mbv-core/src/visualizer.rs` (505)
- `crates/mbv-core/src/player_session_events.rs` (505)

## Risks / Trade-offs

- **[Risk] Over-splitting may fragment related logic** → Mitigation: Only split files with clear extraction boundaries. Keep files intact if they are cohesive. Document decisions.

- **[Risk] Extracted modules may create circular dependencies** → Mitigation: Use Rust's module system to re-export types as needed. Ensure extracted modules depend on the parent, not vice versa.

- **[Risk] Test files may lose context when split** → Mitigation: Keep test files intact unless they contain tests for multiple distinct areas. Preserve test identities and naming conventions.

- **[Trade-off] Some files at 600–650 lines may grow past 800 in the future** → Accepted. The goal is to evaluate and refactor where it improves maintainability now, not to preemptively split every file. Files that grow past 650 lines should be re-evaluated in a future change.

- **[Trade-off] Extraction may require updating `use` statements across the codebase** → Accepted. This is a mechanical change and is verified by `cargo check` and `cargo test`.

## Open Questions

None. All decisions are resolved above.
