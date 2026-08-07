# Tasks

## 1. Fix `restore_local_mode` reconnect tail

- [x] 1.1 In `src/app/session_connect.rs`, in the `home_is_local_daemon` branch of
  `restore_local_mode`, build the reconnected local-daemon queue as
  `PlayerTab::new(initial_items, initial_cursor)` and assign it to `self.player_tab` instead of
  `self.remote_player_tab`
- [x] 1.2 Mirror D2: set `self.queue_source` from the reconnected remote's `queue_source` (the
  non-empty-daemon path of `bootstrap_local_daemon_queue`, matching `construct.rs:482`)
- [x] 1.3 Replace the tail's `has_initial_items`-gated scope branch with an unconditional
  `remote_player_tab = None; set_queue_scope(QueueScope::Local)` shared by both arms; drop the
  now-unused `has_initial_items` binding if nothing else reads it
- [x] 1.4 Confirm the changed tail compiles with no unused-variable/unused-binding warnings and
  no lost `debug_assert_eq!(player.is_remote(), player_endpoint.is_some())` invariant

## 2. Regression test

- [x] 2.1 In `src/app/tests_route_state.rs`, add a test in the shape of
  `restore_local_mode_reconnects_local_daemon_when_no_suspended_local_player_exists`:
  `make_local_daemon_app_stub(make_items(2))`, route to a stub remote via
  `switch_to_library_route`, then `restore_local_mode` under `DAEMON_ROUTE_CONNECT_OVERRIDE`
  returning a stub `RemotePlayer` with non-empty items
- [x] 2.2 In that test, assert `remote_slot_state() == RemoteSlotState::LocalDaemon` (not
  `DirectRemote`), `app.remote_player_tab.is_none()`, and that `displayed_queue()` shows the
  reconnected daemon's items (queue not emptied)
- [x] 2.3 Assert `player.is_remote()` and `is_local_daemon()` remain true (still attached to the
  local daemon, not a bare-mode fallback)

## 3. Verification

- [x] 3.1 Run `rtk cargo check -p mbv-core` and `rtk cargo clippy --workspace --all-targets`
- [x] 3.2 Run `rtk cargo test -p mbv-core` and the app test for the new case (e.g.
  `rtk cargo test --bin mbv restore_local_mode`) — no failures
- [ ] 3.3 Manual smoke: stay-alive launch → connect to remote mbvd (pill lit) → `d` → pill gone,
  queue shows daemon items (per proposal's Reproduction)
