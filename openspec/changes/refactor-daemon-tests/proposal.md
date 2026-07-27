## Why

`crates/mbv-core/src/daemon_tests.rs` has grown to 896 lines, exceeding the 800-line limit enforced by `scripts/check-code-file-lines.sh`. The file mixes several distinct test concerns (media-type filtering, client connection/driver lifecycle, queue operations, and WebSocket handling) alongside shared test helpers, making it hard to navigate and maintain. Splitting it into focused modules keeps each file under the limit and makes test intent immediately clear from the file name.

## What Changes

- Split `daemon_tests.rs` into smaller, thematically coherent test modules under `crates/mbv-core/src/` (or a `tests/` submodule directory).
- Extract shared test helpers (`item`, `connect_client`, `shared_queue_state`, `cold_player`, `recv_event`, `assert_close`) into a shared test utilities module.
- Group tests by concern: media-type filtering, client/driver lifecycle, queue operations, and WebSocket handling.
- No production code changes; tests-only refactor.
- **No behavior changes** — all existing tests must continue to pass with identical assertions.

## Capabilities

### New Capabilities

- `daemon-test-modules`: Split the monolithic `daemon_tests.rs` into focused, sub-800-line test modules with shared helpers extracted to a common utility module.

### Modified Capabilities

(none — this is a tests-only refactor with no spec-level behavior changes)

## Impact

- **Code**: `crates/mbv-core/src/daemon_tests.rs` will be removed or reduced, replaced by 3–4 new test files/modules plus a shared helpers module.
- **Dependencies**: No new dependencies. Internal `use` imports will need updating.
- **CI**: `make check-code-file-lines` will pass again.
- **Risk**: Low — test-only refactor, no production logic changes.
