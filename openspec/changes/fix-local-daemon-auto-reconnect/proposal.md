## Why

`local-daemon-thin-client` already requires that a same-host local-daemon client "perform the
same session-state persistence a bare-mode mbv performs, including the auto-reconnect record
written at teardown" — but the implementation only does the write half. `App::new()` (bare mode)
calls `try_auto_reconnect()` at startup; `App::new_remote()` (used for every stay-alive /
local-daemon-attached launch) never does. The result, confirmed live against a real user's log and
`last_remote_connection.json`: a manually-connected remote session (e.g. a Sessions-panel device
named "music") gets correctly persisted on quit, then the very next stay-alive launch attaches to
the local daemon and never even attempts to restore it — no `"auto-reconnect enabled; loading
state"` log line at all. The user sees no Local/Remote queue toggle (it only renders once a
reconnect actually populates `remote_player_tab`) and no reconnection, exactly as if auto-reconnect
had been silently disabled.

A second, compounding bug makes this worse than a one-time miss: `App::teardown()` does not skip
its persistence write for local-daemon sessions (by design — see the existing requirement above),
but because reconnect was never attempted, `active_route` / `connected_session_state` /
`direct_remote_label` are all `None` at teardown, so it computes `"clear"` and overwrites the
previously-good saved target with nothing. One local-daemon session that never happens to touch the
remote connection is enough to permanently erase a working saved reconnect target, even before this
bug is fixed.

## What Changes

- `App::new_remote()` calls `try_auto_reconnect()` when `is_local_daemon` is true (attaching to the
  local stay-alive daemon), mirroring `App::new()`'s bare-mode startup — restoring parity with
  pre-#416 behavior where the same underlying `App` construction path always ran this. Not called
  for a genuinely remote/explicit daemon endpoint (`is_local_daemon == false`), where the user has
  already stated an explicit target.
- `App::teardown()`'s persistence gate is changed so a local-daemon session that never attempted (or
  never got to attempt) a reconnect this run does not overwrite an existing saved
  `last_remote_connection.json` with a clear. Clearing remains correct when the session actually
  had and then lost/dropped a tracked remote connection (explicit disconnect, or a failed
  auto-reconnect attempt this run) — see design.md for the exact mechanism and alternatives
  considered.
- Extends `local-daemon-thin-client`'s existing persistence requirement to explicitly cover the
  restore half (auto-reconnect on startup), not just the write half, and tightens it against the
  overwrite-on-untried-session failure mode.

## Capabilities

### Modified Capabilities

- `local-daemon-thin-client`: the "Clients persist the state bare mode persists" requirement is
  extended to require restoring a saved auto-reconnect target on startup (not just persisting one
  at teardown), and to require that a local-daemon session which never attempts a reconnect this
  run must not erase an existing saved target.

  Note on sequencing: `local-daemon-thin-client` is itself still an unarchived delta spec, owned by
  `retire-pty-relay-for-local-daemon-stay-alive` (already shipped in code as of v0.15.5, not yet
  synced into `openspec/specs/`). This change's delta is written as a further MODIFIED pass over
  that change's own requirement text and assumes it lands in main specs first (or the two are
  reconciled together at archive time) — see design.md.

## Impact

- Code: `src/app/construct.rs` (`App::new_remote`), `src/app/session_connect.rs`
  (`try_auto_reconnect`), `src/app/run_loop_events.rs` (`teardown`).
- Tests: `src/app/tests_auto_reconnect.rs` gains coverage for the local-daemon-attach path; a new or
  extended test for `teardown`'s persistence-gate behavior.
- No protocol, wire-format, or config schema changes. No new user-facing settings — this restores
  existing `auto_reconnect` config behavior for a code path that currently ignores it.
- Depends on / should be sequenced relative to `retire-pty-relay-for-local-daemon-stay-alive`'s
  archival (see Capabilities note above).
