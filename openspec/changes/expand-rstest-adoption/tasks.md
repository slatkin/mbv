## 1. Baseline and mbv-core parser/status batch

- [x] 1.1 Freeze a `baseline.md` from `candidates.md` before edits: record every selected old test name/count by file, package totals from `cargo nextest list -p mbv-core` and `-p mbv`, and all seven fixture-consumer mappings; verify the ledger names resolve in the live source and defer any drifted candidate rather than expanding scope.
- [x] 1.2 Convert the six `ws.rs` is-none tests from the ledger to one named `#[rstest]` case table, preserving each input and `parse_msg(..).is_none()` assertion; run `cargo fmt --all`, `cargo clippy -p mbv-core --all-targets -- -D warnings`, `cargo nextest run -p mbv-core ws`, and commit only `crates/mbv-core/src/ws.rs`.
- [x] 1.3 Convert the four selected `api_tests_parsing.rs` families (15 tests) to named case tables, retaining the existing `json!` input shape and every expectation; verify 15 generated named cases with `cargo nextest list -p mbv-core`, run the mbv-core per-file gates, and commit only that file.
- [x] 1.4 Convert `next_idx_*` and `previous_idx_*` in `player_tests_status.rs` (6 tests) to named case tables; verify six generated named cases, run the mbv-core per-file gates, and commit only that file.

## 2. mbv-core feed and persistence batch

- [x] 2.1 Convert `feed_queue_item_*` (3 tests) plus the three eligible same-file helpers in `player_tests_session_feed.rs` to named cases/`#[fixture]`s, keeping consumer-specific state and assertions explicit; verify the 3→3 table mapping and all fixture consumer names in `baseline.md`, run mbv-core per-file gates, and commit only that file.
- [x] 2.2 Convert the two feed families in `playback_queue_tests_feed.rs` (6 tests) to named case tables, preserving the primary-source and full media-kind assertion sets; verify six named generated cases, run mbv-core per-file gates, and commit only that file.
- [x] 2.3 Convert the three selected queue-item deserialization tests in `playback_queue_tests_persistence.rs` to a named table without weakening the variant and ID assertions; verify 3→3 named case parity, run mbv-core per-file gates, and commit only that file.

## 3. Root-package interaction and state batch

- [ ] 3.1 Convert the 14 selected `help.rs` key-to-message and scroll tests to named case tables; verify each generated case maps to a ledger test name, run `cargo fmt --all`, `cargo clippy -p mbv --all-targets -- -D warnings`, `cargo nextest run -p mbv help`, and commit only that file.
- [ ] 3.2 Convert the seven selected `tests_lifecycle.rs` transport and render-interval tests to named case tables; verify 7→7 parity, run root-package per-file gates, and commit only that file.
- [ ] 3.3 Convert the four `move_queue_item_*` tests and the eligible `make_feed_entry("f1")` fixture in `tests_queue_reorder.rs`; retain order, cursor, and undo assertions, verify case/fixture consumer mappings, run root-package per-file gates, and commit only that file.
- [ ] 3.4 Convert the two letter-pill tests in `actions_tests_letter.rs` to named cases; verify 2→2 parity, run root-package per-file gates, and commit only that file.
- [ ] 3.5 Convert the two `set_content_*` tests in `components/daemon_lost.rs` to named cases; verify 2→2 parity, run root-package per-file gates, and commit only that file.
- [ ] 3.6 Convert the three `remote_seek_*` tests in `actions_tests_queue.rs` to named cases; verify 3→3 parity, run root-package per-file gates, and commit only that file.
- [ ] 3.7 Convert the four selected search-sidebar key tests in `components/search_sidebar.rs` to named cases; verify 4→4 parity, run root-package per-file gates, and commit only that file.
- [ ] 3.8 Convert the two selected watched/unwatched filter tests in `components/feeds_component_tests.rs` to named cases; verify 2→2 parity, run root-package per-file gates, and commit only that file.

## 4. Root-package fixture batch

- [ ] 4.1 Convert `make_queue_items(3)` in `actions_tests_queue_state.rs` to a same-file `#[fixture]` used by its five recorded cursor tests; retain each test’s assertion and explicit scenario data, run root-package per-file gates, and commit only that file.
- [ ] 4.2 Convert `make_socket_merge_ready_app()` in `tests_podcast_playback.rs` to a same-file fixture used by its four recorded socket-progress tests; verify exactly those consumers remain, run root-package per-file gates, and commit only that file.
- [ ] 4.3 Convert `make_home_video_app()` in `tests_feed_group_loading.rs` to a same-file fixture used by its three recorded feed-home-video tests; verify exactly those consumers remain, run root-package per-file gates, and commit only that file.

## 5. Whole-change verification and disposition

- [ ] 5.1 Verify every converted table is named and preserves the baseline’s one-to-one test count: read each file diff, compare its cases to `baseline.md`, and verify `cargo nextest list -p mbv-core` and `-p mbv` totals remain unchanged; record selected/deferred results in `candidates.md` without changing deferred source.
- [ ] 5.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --release --test-threads=4`; verify all succeed, `cargo tree -p rstest --target all` still contains no async runtime or timeout path, and `git diff --stat` contains only test modules, their test-only helpers, and this change’s artifacts.
- [ ] 5.3 Commit the baseline, candidate disposition, and task-state docs separately from source commits; comment on #710 with the resulting family/case/fixture counts and explicitly note every deferred category, without closing the issue.
