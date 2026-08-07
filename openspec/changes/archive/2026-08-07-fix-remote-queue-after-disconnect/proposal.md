## Why

Disconnecting from a remote mbvd does not turn off remote queue management. `has_remote_queue()`
reads exactly one field — `remote_player_tab.is_some()` — and `restore_local_mode`'s
local-daemon reconnect tail (stay-alive clients only) unconditionally repopulates that field and
sets the scope to `Remote` when the daemon holds items, so the aqua queue-scope pill stays lit and
the queue panel keeps offering remote queue control after `d`. This breaks the invariant
`App::new_remote` establishes for the same state: a local daemon presents one unified queue with no
separate remote tab and no scope pill.

## What Changes

- `restore_local_mode`'s `home_is_local_daemon` reconnect branch restores the reconnected local
  daemon items into the unified `player_tab` exactly as `App::new_remote` does via
  `local_daemon_bootstrap` (construct.rs), instead of stuffing them into `remote_player_tab`.
- `remote_player_tab` is set to `None` and queue scope to `Local` on every disconnect path
  (explicit `d`, `RemoteDisconnected`, unannounced daemon loss, `DaemonShutdownAnnounced`) for
  stay-alive clients, matching the bare-mode arm that already does this.
- The now-redundant `has_initial_items`-based queue-scope branch in the reconnect tail is removed.
- Regression coverage asserting `remote_slot_state()` is `LocalDaemon` (not `DirectRemote`) after
  `restore_local_mode` on a `home_is_local_daemon` app, plus that the daemon's items remain visible.

## Capabilities

### New Capabilities

- `remote-queue-disconnect`: after disconnecting from a remote mbvd, the client returns to the
  plain local-daemon presentation — one unified queue, no remote queue tab, no scope pill — with
  the daemon's queue still shown.

### Modified Capabilities

None — no existing spec covers queue-scope/remote-queue presentation behavior.

## Impact

- Code: `src/app/session_connect.rs` (`restore_local_mode`, the `reconnected_local_daemon` tail),
  cross-checked against `src/app/construct.rs` (`local_daemon_bootstrap`) and
  `src/app/queue_scope.rs` (`has_remote_queue`).
- Tests: `src/app/tests_route_state.rs` gains the `home_is_local_daemon` regression case.
- No protocol, wire-format, config, or dependency changes.
