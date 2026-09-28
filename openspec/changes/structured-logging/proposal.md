# Proposal: Structured logging

Issue: #845 (split from #815; standard M-LOG-STRUCTURED).

## Why

The logs are hard to use when debugging. Each line is `ts=HH:MM:SS level=… source=… msg="…"`,
so all context (item and slot ids, request ids, URLs, durations) is buried inside `msg`.
You can't filter by key, the timestamp has no date or sub-second part, and nothing ties
together the TUI, local-daemon and `mbvd` lines for one playback or queue operation. Log
file failures are silently ignored, and a long-running daemon never rotates its file.

## What Changes

- Replace the custom `log` backend in `mbv-core` with a `tracing` subscriber that writes
  logfmt with named key-value fields. The existing `source=` key stays. `msg=` stays as
  the last key and is quoted only when its value needs it.
- Timestamps become RFC 3339 local time with milliseconds and a UTC offset
  (`2026-09-28T14:03:12.345+02:00`), produced by the `time` crate, which is already a
  dependency.
- Each converted event carries a dot-notation event name (`event=player.load.start`) and
  its context as fields (`slot=`, `item=`, `request=`, `http.response.status_code=`, …).
- Spans cover four scopes: ctrl requests (playback intent and queue load, keyed by the
  owner's client id plus the existing request ids), playback sessions (slot, item, Emby
  play session), Emby progress reporting, and HTTP requests. The context is carried
  across the worker threads and event-loop handoffs each operation passes through. The
  owner and the TUI each log a matching `peer=` for every connection: the TUI's pid on a
  local socket, or its `ip:port` over TCP when no address translation sits in between.
  That lets you match TUI and owner lines without any protocol change.
- `--log-level` accepts `trace` and per-target directives
  (`--log-level info,player=debug`). A plain level keeps working as it does today.
- A log file that can't be opened or rotated produces one message on stderr. The file
  is rotated by size while the process is running, not only at startup, and three older
  generations are kept.
- Credentials never appear in log output. Every URL is logged without its username/
  password, query string or fragment, and bearer tokens are replaced.
- All ~415 existing `log::` call sites are converted, crate by crate. Until each crate is
  converted, its `log::` calls keep reaching the same sink through the `log` bridge.
- **BREAKING (log format only)**: the timestamp format and key set change. Anything that
  parses `mbv.log` by the old `ts=HH:MM:SS` shape must be updated. No known consumers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `application-logging`: the log level flag gains `trace` and per-target directives. New
  requirements cover the logfmt line shape, full timestamps, named events with fields,
  correlation fields, redaction, reporting log file failures, and rotation.

## Impact

- `crates/mbv-core/src/applog.rs` (rewritten; split into submodules if needed), plus
  `mbv-core`'s `Cargo.toml`: add `tracing`, `tracing-subscriber` (`registry` and `std`
  features only, no `env-filter`/regex), `tracing-log` and `time`; drop `libc`. The
  workspace `time` gains the `formatting`, `local-offset` and `macros` features.
- `mbv_net::native_tls_agent` gains a required service argument and installs the HTTP
  logging middleware. Its callers in Emby, Audiobookshelf, Feeds and the TUI's image
  fetching must pass it.
- Ctrl owner (`mbv-daemon`): logs the peer when a client connects, and the parked queue
  load records its client id.
- Workspace `Cargo.toml`: `tracing` becomes a direct workspace dependency. It is already in
  the lockfile transitively. It does not bring in tokio.
- Every crate with log call sites: `src/`, `mbv-player`, `mbv-daemon`, `mbv-emby`,
  `mbv-audiobookshelf`, `mbv-ws`, `mbv-remote-player`, `mbv-config`, `mbv-desktop`,
  `mbv-visualizer`, `mbvd`, `mbv-net`, `mbv-images`, `mbv-feed`, `mbv-cast`.
- `--log-level` parsing in `src/main.rs`, `src/local_daemon.rs` and `crates/mbvd/src/main.rs`.
  The local daemon receives the whole directive string from its parent.
- No change to the ctrl protocol, config files, or any persisted state.
