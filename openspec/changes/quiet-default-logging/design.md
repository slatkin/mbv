# Design

## Context

`applog` is one shared logger initialised once per process (`applog::init(stderr, path)`).
It hardcodes `log::set_max_level(Debug)` and `enabled()` returns true, so nothing is
gated. Only the system-instance `mbvd` writes to stderr. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:** one process-wide level, set at init; standard journald priority labelling.

**Non-Goals:** per-sink levels, a config-file key, rate limiting or aggregation of
repeated messages, writing to the journal socket directly.

## Decisions

- **Level passed to `applog::init` and applied via `log::set_max_level`.** The `log`
  macros already skip records above the max level, so no check in `GlobalLogger` is
  needed. Alternative: a config key — rejected, the flag is enough and systemd sets it
  on `ExecStart`.
- **sd-daemon `<N>` prefix on stderr only.** This is the documented systemd convention
  (`sd-daemon(3)`); journald parses it by default (`SyslogLevelPrefix=yes`). No new
  dependency. Alternative: native journal protocol — needs a socket client for no gain.
- **Local daemon inherits via argv.** `mbv` already spawns `mbv --__local-daemon`; it
  appends `--log-level <level>` when one was given. No new IPC.
- **Demote by editing the call sites** (`ws.rs` per-message inbound, Capabilities
  request/reply in `api_client_reporting.rs`). Progress and ping sites are already debug;
  they become silent through the default level alone.
- **System `mbvd` passes `None` for the log path**; `log_path()` stays for other instances.

## Risks / Trade-offs

- [Debug detail missing when a bug happens] → rerun with `--log-level debug`.
- [An existing stay-alive local daemon keeps its old level] → it takes the new level on
  its next spawn; acceptable.
