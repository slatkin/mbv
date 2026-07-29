## Context

`crates/mbv-core/src/daemon_tests.rs` is included via `include!("daemon_tests.rs")` inside `#[cfg(test)] mod tests` in `daemon.rs`. It contains 18 tests and 6 shared helper functions covering four distinct concerns: media-type filtering, client/driver connection lifecycle, queue operations, and WebSocket handling. The file has hit the 800-line limit, and mixing concerns makes navigation harder.

The crate's convention for test files is flat files with descriptive names (e.g., `player_tests_basic.rs`, `config_tests_paths.rs`, `playback_queue_tests.rs`), not test directories.

## Goals / Non-Goals

**Goals:**

- Split `daemon_tests.rs` into focused, sub-800-line files following crate naming conventions.
- Extract shared helpers into a dedicated file so every split file can use them.
- Preserve every existing test with identical assertions — no behavior changes.
- Keep `daemon.rs` test wiring simple and idiomatic.

**Non-Goals:**

- Changing test logic, assertions, or coverage.
- Introducing new test frameworks or fixtures beyond the existing helpers.
- Refactoring production code.

## Decisions

**1. Flat file layout matching crate conventions**

Split into four new test files plus one helpers file, all under `crates/mbv-core/src/`:

| File | Contents | ~Lines |
|------|----------|--------|
| `daemon_tests_helpers.rs` | `item`, `connect_client`, `shared_queue_state`, `cold_player`, `recv_event`, `assert_close` | ~120 |
| `daemon_tests_media_filter.rs` | `all_audio_accepts_audio_items`, `all_audio_rejects_video_items`, `audio_only_daemon_rejects_non_audio_play_request`, `audio_only_daemon_accepts_audio_play_request`, `non_audio_only_daemon_never_rejects` | ~50 |
| `daemon_tests_connection.rs` | `connecting_ctrl_client_becomes_driver_immediately`, `second_connect_evicts_first_and_becomes_sole_connection`, `emby_remote_takeover_disconnects_current_ctrl_driver`, `ctrl_reconnect_after_emby_remote_takeover_becomes_driver_and_receives_broadcasts`, `emby_remote_takeover_without_ctrl_client_still_records_authority`, `sole_client_disconnect_clears_registry_without_touching_playback`, `cold_ctrl_player_command_keeps_connection_as_driver` | ~220 |
| `daemon_tests_queue.rs` | `adopt_queue_rejection_sends_authoritative_state_to_sole_client`, `ctrl_queue_move_updates_authoritative_queue_and_broadcasts_state`, `ctrl_queue_append_updates_authoritative_queue_and_broadcasts_state`, `stale_ctrl_queue_move_is_rejected_and_resyncs_sender` | ~280 |
| `daemon_tests_ws.rs` | `cold_websocket_noop_does_not_evict_ctrl_driver`, `websocket_takeover_helper_records_emby_remote_authority` | ~50 |

**Rationale**: Matches existing crate pattern (`player_tests_basic.rs`, `config_tests_paths.rs`, etc.). Each file stays well under 800 lines. Helpers in a separate file avoid circular imports.

**2. Keep `include!` mechanism in `daemon.rs`**

Replace the single `include!("daemon_tests.rs")` with multiple includes — one per new file — inside the existing `mod tests` block:

```rust
#[cfg(test)]
mod tests {
    include!("daemon_tests_helpers.rs");
    include!("daemon_tests_media_filter.rs");
    include!("daemon_tests_connection.rs");
    include!("daemon_tests_queue.rs");
    include!("daemon_tests_ws.rs");
}
```

**Rationale**: Minimal disruption to `daemon.rs`. The `include!` pattern is already established in this file. Each included file participates in the same `mod tests` scope, so helpers are accessible to all test files without `pub` or module-path gymnastics.

**3. Delete `daemon_tests.rs`**

Remove the original file after splitting. No need to keep it as a re-export or shim.

**Alternative considered**: Converting to a `daemon_tests/` directory module with `mod.rs`. Rejected because it breaks the crate's established flat-file convention and requires changing `include!` to `mod` declarations, which is a larger structural change for no functional benefit.

## Risks / Trade-offs

- **[Import duplication]** → Each split file will need its own `use super::...` imports. Mitigated by keeping the existing import block identical across files; helpers file avoids re-importing helper types in every test.
- **[Build-time include ordering]** → `include!` files are textually inserted, so helpers must be included before tests that use them. Mitigated by listing `daemon_tests_helpers.rs` first in `daemon.rs`.
- **[Review overhead]** → Moving code across files creates a large diff. Mitigated by keeping all test logic unchanged — this is a pure move operation, verifiable with `cargo test`.
