## 1. Remove bell from notify_with_actions

- [x] 1.1 Remove `Self::ring_terminal_bell()` call from `notify_with_actions` in `src/app/notify_actions.rs`

## 2. Delete bell plumbing

- [x] 2.1 Delete both `ring_terminal_bell()` method variants (`#[cfg(not(test))]` and `#[cfg(test)]`) from `src/app/notify_actions.rs`
- [x] 2.2 Delete the `TEST_BELL_LOG` thread-local and its `#[cfg(test)]` block from `src/app/notify_actions.rs`
- [x] 2.3 Remove `use std::io::Write;` import if no longer needed (only used by the deleted `#[cfg(not(test))]` variant)

## 3. Update tests

- [x] 3.1 Delete the test `notify_with_actions_rings_terminal_bell_even_without_system_notifications` from `src/app/actions_tests_routes.rs`
- [x] 3.2 Delete the `#286` comment block above that test (lines 179-181)
- [x] 3.3 Update the stale comment at `src/app/actions_tests_routes.rs:179-180` asserting `flash_status`/`flash_status_high` ring the bell (if not already resolved by the toast classification change)

## 4. Verify

- [x] 4.1 Run `cargo clippy --workspace --all-targets` -- no warnings
- [x] 4.2 Run `cargo nextest run -p mbv` -- all tests pass
