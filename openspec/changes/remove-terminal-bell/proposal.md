## Why

The terminal bell (`\x07`) has proven annoying in practice. The toast classification change (commit 4c46bf2) already removed it from the toast path, but `notify_with_actions` in `notify_actions.rs` still rings the bell for interactive prompts (next-up, skip-intro, clear-queue). No code path should ring the terminal bell; desktop notifications with action buttons already provide sufficient attention.

## What Changes

- Remove the `ring_terminal_bell()` call from `notify_with_actions`
- Delete `ring_terminal_bell()` (both `#[cfg(not(test))]` and `#[cfg(test)]` variants)
- Delete the `TEST_BELL_LOG` thread-local and its `#[cfg(test)]` block
- Delete or update the test `notify_with_actions_rings_terminal_bell_even_without_system_notifications` in `actions_tests_routes.rs`
- Update the stale comment at `actions_tests_routes.rs:179-180` asserting that `flash_status`/`flash_status_high` ring the bell

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Terminal bell behavior was never spec'd; this is pure removal of an annoyance.

## Impact

- `src/app/notify_actions.rs` -- bell plumbing removed (~40 lines deleted)
- `src/app/actions_tests_routes.rs` -- bell-related test deleted, stale comment updated
- No API, protocol, or dependency changes
