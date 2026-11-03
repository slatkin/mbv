# Proposal

## Why

The explicit daemon endpoint mode starts the TUI as a thin client of one endpoint with no local
Owner process. It breaks the rule in `AGENTS.md` that every local TUI is a Client of the per-user
Owner process, and it forces special cases into unrelated work (for example, the
`queue-handoff-and-take` D6 gate on `home_is_local_daemon`). F3 direct connect now does the same
control job through the local Owner, and nothing — no script, packaging file, or unit file — calls
the flag. Removing the mode deletes a whole launch path and its launch-mode state
(`launched_as_remote`, `home_is_local_daemon`).

## What Changes

- **BREAKING**: remove the `--connect-daemon <endpoint>` CLI flag (`src/main.rs`): the parser, its
  tests, and its `--help` line. An invocation that still passes the flag exits with an error that
  names the removal and the replacement paths (the `-d` precedent).
- **BREAKING**: remove the `[mbvd.client] endpoint` config key (`daemon_client_endpoint`) and its
  save path. A config file that still sets it is ignored with a one-time warning, following the
  removed `[panel]` width-key precedent.
- Remove the thin-client startup branch in `run_configured_startup`; every local launch runs
  `run_local_instance` and attaches to the per-user Owner (`DaemonEndpoint::Local`).
- Remove the `launched_as_remote` and `home_is_local_daemon` App fields and the special cases that
  exist only for the removed mode: the teardown auto-reconnect persistence skip, the shutdown-policy
  conjunct, the auto-reconnect restore conjuncts, and the Hand off/Take home gate. Call sites reduce
  to `is_local_daemon()` or drop the conjunct.
- Remove the `home_is_local_daemon` field from `SessionTransferGates` (`mbv-ui-model`) and its
  projection; Hand off/Take gate only on `local_owner_plays` and the selected row's direct endpoint.
- Update ADRs 0006, 0010, 0015, and 0016, which name the mode as a separate mechanism; ADR 0016 gets
  an amendment covering the removal of `home_is_local_daemon`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `daemon-lifecycle`: "Stay Alive is the sole configuration policy" loses the "launched without an
  explicit daemon endpoint" qualifier; gains a scenario rejecting the removed `--connect-daemon`
  flag, like the existing `-d` scenario.
- `local-daemon-stay-alive`: "Stay-alive hosts playback in a local daemon" no longer qualifies the
  Player-owner rule with "launched without an explicit daemon endpoint"; the disabled-stay-alive
  scenario drops the same qualifier.
- `local-daemon-single-instance`: "Clients take no lock" drops the explicit-endpoint clause and its
  "Explicit-endpoint client starts" scenario.
- `local-daemon-thin-client`: "Local clients persist launch-scoped reconnect state" no longer
  distinguishes launch modes; every Client persists per the `auto_reconnect` setting, and the
  "Remote-daemon client exits" skip scenario is removed.
- `non-audio-fall-through`: "Local Player preparation precedes ending the attachment" rewords the
  preparation source: every launch attaches to the local Owner, so no "launch without an explicit
  endpoint" comparison remains.
- `sessions-sidebar`: "F3 offers Hand off and Take on a peer target" drops the "home of the Client
  is the local Owner" condition; the home is always the local Owner.
- `remote-queue-disconnect`: the disconnect-returns-to-local requirement no longer names the removed
  `home_is_local_daemon` field; the return applies to every Client.

## Impact

- `src/main.rs` (flag parsing, help text, startup branch, `run_remote_app` inlining),
  `src/app/state/app_struct.rs`, `src/app/state/construct.rs`, `src/app/state/construct/remote.rs`,
  `src/app/dispatch/run_loop/teardown.rs`, `src/app/dispatch/session/{connect,switch}.rs`,
  `src/app/shell/overlays/sidebars.rs`, `src/app/state/projection/chrome_status.rs`, and the
  touched test files under `src/app/tests/`.
- `crates/mbv-config` (parse, save, types, tests), `crates/mbv-ui-model/src/panel_targets.rs`,
  `crates/mbv-components/src/sessions.rs` and its tests.
- `docs/adr/0006`, `0010`, `0015`, `0016`.
- No ctrl-protocol, keybind-registry, or Service-capability change; the `h`/`t` keys keep their
  identities and only lose one gate conjunct.
