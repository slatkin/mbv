## 1. Restore auto-reconnect on local-daemon attach

- [x] 1.1 In `src/app/construct.rs`, at the tail of `App::new_remote` (after the existing
      `handle_failed_local_daemon_adoption` check, before `app` is returned), add
      `if is_local_daemon { app.try_auto_reconnect(); }`, using the constructor's `is_local_daemon`
      argument rather than `app.is_local_daemon`.
- [x] 1.2 Confirm (read-through, no code change expected) that `switch_to_library_route` and
      `switch_to_direct_remote`'s `else` branch (the `self.player.is_remote()` true case) correctly
      handles being invoked from within `try_auto_reconnect()` when `self.player` is already the
      local-daemon `PlayerProxy::remote`, per design.md's "Composition with the rest of
      `new_remote`" section.

## 2. Fix the teardown persistence skip-gate

- [x] 2.1 In `src/app/run_loop_events.rs`, change `teardown`'s gate from
      `if self.launched_as_remote && !self.is_local_daemon` to
      `if self.launched_as_remote && !self.home_is_local_daemon`.
- [x] 2.2 Update the comment above that gate (currently starting "Gated on `launched_as_remote &&
      !is_local_daemon`") to explain the corrected reasoning: `is_local_daemon` can now flip during
      a local-daemon-launched session once task 1.1 makes reconnect-on-attach routine, so the gate
      must key off the immutable launch-time `home_is_local_daemon` instead.

## 3. Tests

- [x] 3.1 In `src/app/tests_auto_reconnect.rs`, add a test that constructs an `App` via
      `App::new_remote(..., is_local_daemon: true)` (mirroring the existing
      `try_auto_reconnect_restores_a_persisted_library_route` / `..._direct_session` tests' setup —
      saved `LastRemoteConnection`, `auto_reconnect = true`, route-connect/session-load overrides)
      and asserts the saved connection is restored during construction, without a separate manual
      `try_auto_reconnect()` call.
- [x] 3.2 Add a test asserting an explicit-remote `App::new_remote(..., is_local_daemon: false)`
      construction does NOT attempt auto-reconnect, even with a saved record and
      `auto_reconnect = true` (regression guard for design.md's Decision 1 gating rationale).
- [x] 3.3 Add a `teardown`-focused test: an app with `home_is_local_daemon = true`,
      `is_local_daemon` currently `false` (simulating a local-daemon launch that reconnected to a
      genuinely remote target), and a tracked connection (`active_route` or
      `connected_session_state` set) — assert `teardown` persists that connection rather than
      skipping (regression guard for design.md's Decision 2 walk-through, the case the old gate got
      wrong).
- [x] 3.4 Add or extend a `teardown` test for the existing explicit-remote skip case (
      `launched_as_remote = true`, `home_is_local_daemon = false`) to confirm it's unaffected by the
      gate's field change — e.g. adapt the removed-in-#416 style coverage or check
      `tests_lifecycle.rs` for a suitable existing test to extend rather than adding a near-duplicate.

## 4. Verify

- [x] 4.1 `cargo test` (targeted: `tests_auto_reconnect`, `tests_lifecycle`, and any new/changed
      `run_loop_events`/`construct` coverage) passes.
- [x] 4.2 `cargo clippy` and `cargo fmt --all -- --check` clean on the changed files.
- [ ] 4.3 Manual check against the reporting user's own environment: delete or inspect
      `~/.local/state/mbv/last_remote_connection.json` (currently
      `{"kind":"DirectSession","device_name":"music"}`), launch under stay-alive, and confirm the
      log shows `"auto-reconnect enabled; loading state"` followed by a resolved/attempted
      reconnect — where previously the local-daemon-attach launch logged nothing under the
      `auto_reconnect` target at all.
