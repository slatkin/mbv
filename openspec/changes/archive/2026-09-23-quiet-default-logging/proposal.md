# Proposal

## Why

The systemd `mbvd` wrote ~100k journal lines in two days (#758). Almost all of it is
routine debug chatter (per-message websocket logging, pings, progress reports) that
every binary logs unconditionally, and journald receives every line at one flat
priority, so real warnings and errors cannot be filtered. The TUI and local daemon log
files carry the same chatter.

## What Changes

- Every binary (`mbv` TUI, local daemon, `mbvd`) logs at `info` and above by default;
  debug output is opt-in.
- New `--log-level <error|warn|info|debug>` flag on `mbv` and `mbvd`; the local daemon
  inherits the level from the `mbv` that spawns it.
- Routine traffic chatter is logged at debug only: per-message websocket inbound,
  progress reports, pings, and the Capabilities request and its reply.
- Lines written to stderr carry the systemd priority prefix, so journald records each
  line at its real level.
- The systemd `mbvd` logs only to journald; it no longer writes its own log file.

Out of scope: the cause of the 81k-line burst on 2026-09-21 (separate issue).

## Capabilities

### New Capabilities
- `application-logging`: log level default and control, which messages are routine
  (debug) chatter, and how log lines reach journald.

### Modified Capabilities
<!-- none -->

## Impact

- `crates/mbv-core/src/applog.rs` (level gate, stderr prefix)
- `crates/mbv-core/src/ws.rs`, `crates/mbv-core/src/api_client_reporting.rs` (demotions)
- `crates/mbvd/src/main.rs`, `src/main.rs`, `src/local_daemon.rs` (flag, init)
- Deploy host: the `LogFilterPatterns=` workaround can be removed.
