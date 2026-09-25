# Tasks

## 1. Refetch the shelf on refresh

- [x] 1.1 In `audiobookshelf_refresh` (`src/app/dispatch/audiobookshelf/browse.rs`), after the existing `start_audiobookshelf_shows` call, call `crate::app::dispatch::session::service_startup::start_audiobookshelf_shelves` with the same `library_id`, `generation`, a cloned config, and `self.lib_tx.clone()` (design D1); do not touch `audiobookshelf_shelf_cache` (D2). Verify with `cargo check -p mbv`.

## 2. Hermetic tests

- [x] 2.1 Add an App-level test (next to the podcast refresh tests under `src/app/tests/`, using `super::podcast::audiobookshelf_app()` with a second podcast library pushed): select the first library's tab, call `audiobookshelf_refresh`, then drain `app.lib_rx` with `recv_timeout` (design D3) and assert exactly one `AudiobookshelfShelfFetched` arrives, for the refreshed library id and the current generation, and none for the other library. Verify the test passes and fails with task 1.1 reverted.
- [x] 2.2 Add a tick-integration test in `src/app/tests/tick_integration/podcast/latest.rs`: seed the shelf cache, select Latest (marker acknowledged), trigger the refresh, deliver a current-generation `AudiobookshelfShelfFetched` with a different episode, push content, and assert the pill is still `Latest`, the new episode is listed, and the Latest marker stays cleared; then advance the setup generation the way the existing stale-result tests do and deliver a result carrying the old generation, plus and an `Err` result and assert the list is unchanged. Verify with `cargo nextest run -p mbv latest`.

## 3. Gate

- [x] 3.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`; all pass.
