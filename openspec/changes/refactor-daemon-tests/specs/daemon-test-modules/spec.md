## ADDED Requirements

### Requirement: Daemon tests split into focused modules

The daemon integration tests SHALL be organized into separate files by concern, each well under the 800-line limit. Shared test helpers SHALL be in their own file and accessible to all test files.

#### Scenario: Media-type filter tests isolated

- **WHEN** a developer wants to find or modify media-type filtering tests
- **THEN** those tests SHALL be in `daemon_tests_media_filter.rs` and contain exactly the tests: `all_audio_accepts_audio_items`, `all_audio_rejects_video_items`, `audio_only_daemon_rejects_non_audio_play_request`, `audio_only_daemon_accepts_audio_play_request`, `non_audio_only_daemon_never_rejects`

#### Scenario: Connection lifecycle tests isolated

- **WHEN** a developer wants to find or modify client connection and driver lifecycle tests
- **THEN** those tests SHALL be in `daemon_tests_connection.rs` and contain exactly the tests: `connecting_ctrl_client_becomes_driver_immediately`, `second_connect_evicts_first_and_becomes_sole_connection`, `emby_remote_takeover_disconnects_current_ctrl_driver`, `ctrl_reconnect_after_emby_remote_takeover_becomes_driver_and_receives_broadcasts`, `emby_remote_takeover_without_ctrl_client_still_records_authority`, `sole_client_disconnect_clears_registry_without_touching_playback`, `cold_ctrl_player_command_keeps_connection_as_driver`

#### Scenario: Queue operation tests isolated

- **WHEN** a developer wants to find or modify queue operation tests
- **THEN** those tests SHALL be in `daemon_tests_queue.rs` and contain exactly the tests: `adopt_queue_rejection_sends_authoritative_state_to_sole_client`, `ctrl_queue_move_updates_authoritative_queue_and_broadcasts_state`, `ctrl_queue_append_updates_authoritative_queue_and_broadcasts_state`, `stale_ctrl_queue_move_is_rejected_and_resyncs_sender`

#### Scenario: WebSocket tests isolated

- **WHEN** a developer wants to find or modify WebSocket-related tests
- **THEN** those tests SHALL be in `daemon_tests_ws.rs` and contain exactly the tests: `cold_websocket_noop_does_not_evict_ctrl_driver`, `websocket_takeover_helper_records_emby_remote_authority`

### Requirement: Shared test helpers extracted

Shared helper functions used across multiple daemon test files SHALL be defined in `daemon_tests_helpers.rs` and included before all test files.

#### Scenario: Helpers accessible to all test files

- **WHEN** any daemon test file calls `item()`, `connect_client()`, `shared_queue_state()`, `cold_player()`, `recv_event()`, or `assert_close()`
- **THEN** the call SHALL resolve without additional imports or module-qualified paths

### Requirement: All existing tests pass unchanged

The refactor SHALL preserve every existing test with identical assertions. No test logic, expected values, or test names SHALL change.

#### Scenario: Full test suite passes

- **WHEN** `cargo test` runs on `mbv-core`
- **THEN** all 18 daemon tests SHALL pass with the same results as before the refactor

### Requirement: All files under line limit

Every new test file SHALL be under 800 lines.

#### Scenario: Line check passes

- **WHEN** `./scripts/check-code-file-lines.sh` runs
- **THEN** all new daemon test files SHALL be reported as within the 800-line limit
