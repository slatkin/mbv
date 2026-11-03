# Design

## Context

See `proposal.md` (Why). Current facts that shape the approach:

- `main.rs` owns both triggers of the mode: `connect_daemon_arg` parses `--connect-daemon` (split
  and `=` forms), and `run_configured_startup` folds in `config.daemon_client_endpoint`. The
  explicit endpoint, when present, skips single-instance resolution entirely and calls
  `run_remote_app` with the parsed endpoint.
- `run_remote_app` is also the attach path for a normal local launch:
  `attach_owner_process` connects to `DaemonEndpoint::Local` and calls the same function. After the
  removal it is the only call site.
- `App::new_remote_optional_with_config` (`src/app/state/construct/remote.rs`) sets
  `home_is_local_daemon = endpoint.is_local()` and `launched_as_remote = true` at construction. With
  the mode gone, every construction is the local attach, so the launch-time snapshot is constant:
  `home_is_local_daemon == true` always.
- ADR 0016 split "current target" (`player_endpoint`) from "launch mode"
  (`home_is_local_daemon`) because a mid-session swap repoints the current target. The current-target
  half survives; only the launch-mode half collapses.
- Remaining `home_is_local_daemon` readers: `switch.rs` (4 sites of
  `home_is_local_daemon && is_local_daemon()`, plus one bare check in `restore_local_mode`),
  `connect.rs` (auto-reconnect library-route restore), `teardown.rs` (shutdown-policy conjunct and
  the persistence-skip branch), the Sessions transfer gates (`SessionTransferGates` across
  `mbv-ui-model`, the sidebars projection, and the component), `panel_targets.rs`, and tests.
- `launched_as_remote` has exactly one reader: the teardown persistence-skip branch.
- Config parsing is manual `toml::Value` extraction (`parse_mbvd_section`); an unknown key is
  silently ignored, so removing the field alone would drop the key silently.
- Precedent for removed config keys: `warn_old_panel_width_keys` warns once via `tracing::warn!`
  and keeps parsing the rest of the file.
- Precedent for removed CLI flags: `-d` exits with guidance before any startup side effect
  (`pre_config_startup`).
- The specs that name the mode or the fields: `daemon-lifecycle`, `local-daemon-stay-alive`,
  `local-daemon-single-instance`, `local-daemon-thin-client`, `non-audio-fall-through`,
  `sessions-sidebar`, `remote-queue-disconnect`.

## Goals / Non-Goals

**Goals:**

- One launch path: every `mbv` invocation attaches to the per-user Owner process.
- Delete the launch-mode state (`launched_as_remote`, `home_is_local_daemon`) and every special
  case that exists only for the removed mode.
- Keep `DaemonEndpoint` parsing, library routes, and F3 direct connect untouched.

**Non-Goals:**

- No new way to attach to a remote Owner at startup. F3 direct connect stays the path.
- No ctrl-protocol, keybind-registry, or Service-capability change.
- No behavior change for `mbvd`, stay-alive, or the local Owner lifecycle.

## Decisions

### D1. A `--connect-daemon` invocation exits with guidance, like `-d`

`pre_config_startup` gains a rejection before any startup side effect: any argument equal to
`--connect-daemon` or starting with `--connect-daemon=` exits 1 with a message naming the removal
and the replacements — the automatic local-Owner attach, and the Sessions panel (F3) for reaching a
remote Player owner.

Rejected alternative: silently ignore the flag. That would attach to the local Owner and put an
mpv on this machine when the user asked for playback on a remote daemon — a materially different
outcome, not a benign downgrade.

### D2. The config key warns once and is ignored

`parse_mbvd_section` stops reading `client_endpoint`, and a `warn_old_panel_width_keys`-style check
warns once (`config.mbvd.client_endpoint` event) when `[mbvd.client] endpoint` is still present.
Save never writes it again, so the key disappears from the file on the next settings flush.

### D3. Both launch-mode fields are deleted, not just their special cases

With the mode gone, `home_is_local_daemon` is constant true and `launched_as_remote` is constant
true at every construction site. Keeping a constant field invites new readers that reason about a
mode that no longer exists. The simplifications:

- `switch.rs`: `home_is_local_daemon && is_local_daemon()` reduces to `is_local_daemon()`;
  `restore_local_mode`'s bare `else if self.home_is_local_daemon` becomes the unconditional
  reconnect branch after the suspended-link case.
- `connect.rs`: the library-route local check drops the conjunct.
- `teardown.rs`: the shutdown-policy conjunct drops; `persist_auto_reconnect_target_on_teardown`
  keeps only the `auto_reconnect` gate.
- `sessions.rs`: `hand_off_available` / `take_available` gate on `local_owner_plays` and the
  selected row's direct endpoint; `SessionTransferGates` loses `home_is_local_daemon`.
- `chrome_status.rs`: `connected_player_endpoint_label` stays — it names the connected peer for F3
  direct connects and library routes — but the regression test stops planting
  `daemon_client_endpoint` in the config, since the field no longer exists.
- `construct/remote.rs`: the constructor keeps its `endpoint.is_local()` guard around
  `try_auto_reconnect()`. Production always passes `Local`; the guard keeps test constructions on
  non-local endpoints from attempting real connections.

The ADR 0016 two-state split survives as one state: `player_endpoint` remains the sole record of
the current Player-owner target, and `is_local_daemon()` / `player_owner_is_on_this_machine()` stay
the predicates to use.

### D4. One startup path in `main.rs`

`run_configured_startup` collapses: `main` calls `run_local_instance` directly after the panel
launch, keeping the `startup.application.started` event. `run_remote_app` is inlined into
`attach_owner_process`, its only remaining caller. `StartupArgs` loses `cli_daemon_endpoint`.

### D5. ADR updates are amendments in place

Following the repo's ADR convention, each of 0006, 0010, 0015, and 0016 gets an amendment block
dated with the change rather than silent rewrites: 0006 drops the thin-client bullet's mode
wording, 0010 marks the startup-only path as removed, 0015 rewords the shared client path, and
0016 records that the launch-mode half of its two-state split was removed with the mode, leaving
`player_endpoint` as the single target record.

## Risks / Trade-offs

- A user who relied on `--connect-daemon` or `[mbvd.client] endpoint` gets a hard error or a
  warning instead of the old behavior. That is the point of the change; the messages name the
  replacement paths.
- The Sessions transfer gates lose one conjunct. Today the conjunct is always true in production,
  so no observable gate change remains — the spec delta records the simpler rule.

## Migration Plan

1. Land the code and spec deltas together; the removed flag/key were already unused by scripts,
   packaging, and unit files (verified in the issue).
2. Existing config files keep parsing: the stale key warns once and everything else loads.

## Open Questions

None.
