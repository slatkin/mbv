## 1. Baseline And Guardrails

- [x] 1.1 Before creating an isolated application-code worktree from `origin/main`, record and read the absolute planning change path in the current checkout; keep that checkout accessible as the source of truth because untracked OpenSpec artifacts will not appear in a revision-based worktree. Inspect both worktrees' `git status` and preserve unrelated changes without staging or modifying them.
- [x] 1.2 Capture the pre-move affected test inventory with `cargo test -p mbv --bin mbv -- --list | perl -pe 's/app::tests_(?:library_position|feed_podcast|queue_mutation|session_connect|lifecycle)::/app::MOVED::/' | LC_ALL=C sort > /tmp/mbv-369-tests.before` and confirm the four source modules contribute 29, 21, 28, and 30 tests respectively.
- [x] 1.3 Reconfirm every test function in the design's exact move manifest exists once in the current source; use function names rather than recorded line numbers as move boundaries.

## 2. Library Position And Panel Focus Lane

- [x] 2.1 Retain only the 12 manifest-assigned position model, persistence, reset, and home-navigation tests in `src/app/tests_library_position.rs`.
- [x] 2.2 Create `src/app/tests_library_position_restore.rs` and move its 10 manifest-assigned runtime restore tests as complete unchanged items, including the local networking/atomic imports inside `restoring_library_position_does_not_eagerly_prefetch_all_items`.
- [x] 2.3 Create `src/app/tests_panel_focus.rs` and move its 6 manifest-assigned Panel focus tests as complete unchanged items.
- [x] 2.4 Remove `queue_restore_uses_saved_cursor_when_last_played_is_missing` from `tests_library_position.rs` only after confirming the daemon-bootstrap lane contains the unchanged item.

## 3. Feed And Podcast Lane

- [x] 3.1 Create `src/app/tests_feed_group_nav.rs` and move the 10 manifest-assigned feed eligibility, state, selection, cache, navigation, clamping, and refresh-target tests unchanged.
- [x] 3.2 Create `src/app/tests_feed_group_loading.rs` and move the 4 manifest-assigned pagination, aggregation/filtering, and refreshed-event reconciliation tests unchanged.
- [x] 3.3 Create `src/app/tests_podcast.rs` and move the 7 manifest-assigned podcast detection and context-menu tests unchanged.
- [x] 3.4 Delete `src/app/tests_feed_podcast.rs` after confirming all 21 source tests exist exactly once across the three destinations.

## 4. Queue Mutation And Reorder Lane

- [x] 4.1 Retain the 12 manifest-assigned enqueue, clear, remove, context-menu, and scope-isolation tests in `src/app/tests_queue_mutation.rs`, including its crossterm key imports and test-local `item_ids` closures.
- [x] 4.2 Create `src/app/tests_queue_reorder.rs` and move the 16 manifest-assigned move, undo, Queue slot, and remote-reconciliation tests unchanged; do not add the queue-mutation-only crossterm import.
- [x] 4.3 Confirm all 28 source tests exist exactly once across the two queue destinations and no helper or assertion was consolidated or rewritten.

## 5. Bootstrap, Session, Reconnect, And Route Lane

- [x] 5.1 Create `src/app/tests_daemon_bootstrap.rs` with the 8 manifest-assigned bootstrap/adoption tests from `tests_session_connect.rs` and the unchanged `queue_restore_uses_saved_cursor_when_last_played_is_missing` item from `tests_library_position.rs`.
- [x] 5.2 Retain the 7 manifest-assigned endpoint/direct-connect tests in `src/app/tests_session_connect.rs`, keeping every nested `direct_success`/`direct_failure` helper with its owning test.
- [x] 5.3 Create `src/app/tests_auto_reconnect.rs` and move the 6 manifest-assigned reconnect policy tests unchanged, including each nested session/route override helper.
- [x] 5.4 Create `src/app/tests_library_route.rs` and move the 8 manifest-assigned lazy-connect and route-application tests unchanged, including nested route helpers and the test-local `CALLS` static.
- [x] 5.5 Move `remote_position_extrapolation_does_not_round_up_partial_seconds` unchanged into existing `src/app/tests_lifecycle.rs`, beside the other runtime-state helper tests.
- [x] 5.6 Confirm all 30 session-connect source tests plus the one library-position queue-restore test exist exactly once across the four decomposed modules and `tests_lifecycle`, with lock guards and override installation/reset lifecycles unchanged.

## 6. Test Module Integration

- [x] 6.1 Update only the test declaration block in `src/app/mod.rs` to declare `tests_library_position`, `tests_library_position_restore`, `tests_panel_focus`, `tests_feed_group_nav`, `tests_feed_group_loading`, `tests_podcast`, `tests_queue_mutation`, `tests_queue_reorder`, `tests_daemon_bootstrap`, `tests_session_connect`, `tests_auto_reconnect`, and `tests_library_route` through the existing `#[cfg(test)]` plus `#[path = "..."]` pattern.
- [x] 6.2 Remove the obsolete `tests_feed_podcast` declaration and confirm no production declarations, imports, bodies, signatures, or visibility changed.
- [x] 6.3 Keep `src/app/tests.rs` and all shared fixture definitions/import paths unchanged; do not add dependencies, helper abstractions, or unrelated cleanup.

## 7. Preservation And Size Verification

- [x] 7.1 Run `cargo fmt --all`, then capture the post-move inventory with `cargo test -p mbv --bin mbv -- --list | perl -pe 's/app::tests_(?:library_position|library_position_restore|panel_focus|feed_group_nav|feed_group_loading|podcast|queue_mutation|queue_reorder|daemon_bootstrap|session_connect|auto_reconnect|library_route|lifecycle)::/app::MOVED::/' | LC_ALL=C sort > /tmp/mbv-369-tests.after`.
- [x] 7.2 Run `diff -u /tmp/mbv-369-tests.before /tmp/mbv-369-tests.after` and require no output; investigate any loss, duplication, or rename rather than updating the baseline.
- [x] 7.3 Run `wc -l` over all 12 decomposed modules plus `src/app/tests_lifecycle.rs` and require every formatted destination to remain below 800 lines.
- [x] 7.4 Review `git diff --color-moved=blocks --color-moved-ws=allow-indentation-change` and confirm every deleted test item reappears unchanged, `src/app/mod.rs` changed only in its test block, and no production or fixture file changed.

## 8. Repository Verification And Review

- [x] 8.1 Run each affected module filter with `cargo test -p mbv --bin mbv 'app::tests_<module>::'` for all 12 decomposed modules plus `tests_lifecycle` and require every targeted test to pass.
- [x] 8.2 Run `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings` successfully.
- [x] 8.3 Run `cargo test --workspace`, then CI-parity `cargo test --release` and `cargo build --release`, and require all commands to succeed.
- [x] 8.4 Obtain an independent code-review/verifier pass focused on test inventory parity, unchanged test bodies, sub-800-line destinations, scope containment, and the absence of production behavior changes; resolve all findings before completion.
