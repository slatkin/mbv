## 1. Classify tests in each render test file

- [x] 1.1 Audit `tests.rs` (13 tests) — classify each as cosmetic or behavioral per the design decision tree
- [x] 1.2 Audit `tests_album_focus.rs` (5 tests) — classify each
- [x] 1.3 Audit `tests_album_listing.rs` (3 tests) — classify each
- [x] 1.4 Audit `tests_album_detail.rs` (4 tests) — classify each
- [x] 1.5 Audit `tests_music_groups.rs` (5 tests) — classify each
- [x] 1.6 Audit `tests_music_detail.rs` (3 tests) — classify each
- [x] 1.7 Audit `tests_non_music.rs` (4 tests) — classify each
- [x] 1.8 Audit `tests_panel.rs` (6 tests) — classify each
- [x] 1.9 Audit `tests_queue.rs` (14 tests) — classify each
- [x] 1.10 Audit `tests_scroll_pills.rs` (8 tests) — classify each
- [x] 1.11 Audit `home_tests.rs` (8 tests) — classify each
- [x] 1.12 Audit `detail_tests.rs` (7 tests) — classify each
- [x] 1.13 Audit `list_tests.rs` (8 tests) — classify each

## 2. Extract shared test helpers

- [x] 2.1 Create `src/app/render/test_helpers.rs` with all helpers duplicated across 3+ files: `buffer_to_string`, `render_sidebar_scrollbar_column`, `render_power_scrollbar_column`, `render_power_scrollbar_column_with_viewport`, `render_pill_bar_hitboxes`, `render_power_library_to_terminal`, `render_power_library_to_terminal_focused`, `render_power_library_to_string`, `render_power_view_to_terminal`, `render_power_view`, `make_power_movie_app`, `make_power_queue_app`, `make_power_remote_queue_app`, `make_power_music_group_app`, `make_power_home_video_app`, `make_power_large_movie_library_app`
- [x] 2.2 Add `#[cfg(test)] #[path = "test_helpers.rs"] mod test_helpers;` to `render/mod.rs` before all other test file includes
- [x] 2.3 Remove duplicated helper functions from each test file that now sources them from `test_helpers.rs`

## 3. Remove cosmetic tests

- [x] 3.1 Delete cosmetic tests from `tests.rs`
- [x] 3.2 Delete cosmetic tests from `tests_album_focus.rs`
- [x] 3.3 Delete cosmetic tests from `tests_album_listing.rs`
- [x] 3.4 Delete cosmetic tests from `tests_album_detail.rs`
- [x] 3.5 Delete cosmetic tests from `tests_music_groups.rs`
- [x] 3.6 Delete cosmetic tests from `tests_music_detail.rs`
- [x] 3.7 Delete cosmetic tests from `tests_non_music.rs`
- [x] 3.8 Delete cosmetic tests from `tests_panel.rs`
- [x] 3.9 Delete cosmetic tests from `tests_queue.rs`
- [x] 3.10 Delete cosmetic tests from `tests_scroll_pills.rs`
- [x] 3.11 Delete cosmetic tests from `home_tests.rs`
- [x] 3.12 Delete cosmetic tests from `detail_tests.rs`
- [x] 3.13 Delete cosmetic tests from `list_tests.rs`

## 4. Clean up

- [x] 4.1 Remove unused imports from test files where cosmetic test removal left dead imports
- [x] 4.2 Remove file-specific helpers that are no longer referenced after cosmetic test removal
- [x] 4.3 Delete any test files that have zero remaining tests after the audit

## 5. Verify

- [x] 5.1 Run `cargo test` — all remaining tests pass
- [x] 5.2 Run `cargo check` — no compilation warnings from dead code in test files
