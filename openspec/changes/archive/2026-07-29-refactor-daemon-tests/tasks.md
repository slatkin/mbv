## 1. Create split test files

- [ ] 1.1 Create `crates/mbv-core/src/daemon_tests_helpers.rs` with the shared helpers: `item`, `connect_client`, `shared_queue_state`, `cold_player`, `recv_event`, `assert_close`
- [ ] 1.2 Create `crates/mbv-core/src/daemon_tests_media_filter.rs` with the 5 media-type filter tests and required imports
- [ ] 1.3 Create `crates/mbv-core/src/daemon_tests_connection.rs` with the 7 connection/driver lifecycle tests and required imports
- [ ] 1.4 Create `crates/mbv-core/src/daemon_tests_queue.rs` with the 4 queue operation tests and required imports
- [ ] 1.5 Create `crates/mbv-core/src/daemon_tests_ws.rs` with the 2 WebSocket tests and required imports

## 2. Update daemon.rs wiring

- [ ] 2.1 Replace `include!("daemon_tests.rs")` in `daemon.rs` with individual includes for each new file, with `daemon_tests_helpers.rs` listed first
- [ ] 2.2 Delete `crates/mbv-core/src/daemon_tests.rs`

## 3. Verify

- [ ] 3.1 Run `cargo test -p mbv-core` — all 18 daemon tests pass
- [ ] 3.2 Run `./scripts/check-code-file-lines.sh` — all new files under 800 lines
