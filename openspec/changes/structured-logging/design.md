# Design: Structured logging

## Context

See proposal.md (Why) and `specs/application-logging/spec.md` for the required behaviour.

Current state (`crates/mbv-core/src/applog.rs`, 206 lines):
- A `log::Log` impl. `GlobalLogger` formats `ts=HH:MM:SS level= source= msg="…"` and writes
  it to an optional file and, when stderr logging is on, to stderr with a `<prio>` prefix.
- `applog::Level` (error/warn/info/debug) is what `--log-level` parses into, in
  `src/main.rs` (`parse_log_level_arg`), `crates/mbvd/src/main.rs` (`parse_log_level`,
  `Action::Serve.log_level`) and `src/local_daemon.rs` (`local_daemon_args` passes it on
  with `Level::logfmt()`).
- mbv call sites use bare-word targets (`target: "player"`, `"api"`, `"ws"`, …). Any target
  containing `::` is treated as third-party and dropped below warn.
- Three sinks: the TUI writes `state_dir/mbv.log` (and stderr for a system instance), the
  local daemon writes `state_dir/local-daemon.log`, and `mbvd` writes its log path or, as
  the system instance, stderr only.
- About 415 `log::` call sites: `src/app` 159, `mbv-player` 103, `mbv-daemon` 40,
  `mbv-emby` 36, `mbv-audiobookshelf` 22, `mbv-ws` 15, `mbv-remote-player` 12,
  `src/main.rs`+`src/local_daemon.rs`+`src/config.rs` 13, and fewer than 5 each in
  config, desktop, visualizer, mbvd, net, images, feed and cast.
- `tracing` 0.1 is already in `Cargo.lock` as a transitive dependency. No crate depends on
  it directly.
- The ctrl protocol already carries correlation ids: `PlaybackIntent { request_id:
  PlaybackRequestId, generation }`, `QueueLoadRequestId`, `QueueSlotId`, `EmbySessionId`.

## Goals / Non-Goals

**Goals:**
- Keep one logging entry point (`mbv_core::applog::init`) for all three binaries.
- Convert every mbv call site in this change, so no crate is left with an ad-hoc format.

**Non-Goals:**
- OpenTelemetry export, JSON output, or any sink besides file and stderr.
- A new wire-level trace id. Correlation reuses ids the protocol already carries.
- Changing what gets logged or at which level, beyond what conversion needs. The level
  policy ("routine traffic is debug-only") stays as it is.
- Managing leftover `*.log.old` files from the old rotation scheme. They are left alone.

## Decisions

### D1. `tracing` over `log`'s `kv` feature
The issue asks for spans for correlation, and `log` has no spans. With `log` + `kv`, every
call site would have to repeat `slot=`/`request=` by hand, so the "nested line inherits
playback context" requirement could not be met. `tracing` is sync, needs no tokio, and is
already in the lockfile.

Dependencies added to `mbv-core`: `tracing`, `tracing-subscriber` (`default-features =
false`, features `registry`, `std`), and `tracing-log` (the `log` → `tracing` bridge for
third-party crates such as ureq and rustls, and for mbv crates not yet converted). No
`env-filter`: it pulls in regex, and D3 needs its own matching anyway. Other crates depend
on `tracing` directly. Once a crate is converted, its `log` dependency is removed.

### D2. A hand-written logfmt `Layer` on a `Registry`
`tracing-subscriber`'s `fmt` layer cannot produce this logfmt shape (key order, bare vs
quoted values, a trailing `msg`). A small custom `Layer`:
- `on_new_span`: format the span's fields to a logfmt fragment once and store it in the
  span's extensions. `on_record` appends to it.
- `on_event`: build the line: `ts`, `level`, `source` (the event target), `event` (the
  metadata name, when explicit — see D4), the event's fields, then the scope fragments
  from outermost to innermost, then `msg`. Write it to each sink.
- The visitor quotes a value when it contains space, `=`, `"` or a control character, and
  escapes `\`, `"`, `\n`, `\r` and `\t`.
- Formatting a line is a pure function (level, target, name, fields, scope, message, time
  parts) → `String`. Tests cover it directly, without a subscriber.

Module layout: `applog.rs` plus `applog/` children (`spec.rs` for D3, `line.rs` for
formatting and redaction, `sink.rs` for the file writer and rotation, `time.rs`). No
`mod.rs`.

### D3. `LogSpec` replaces `applog::Level` at the CLI boundary
`LogSpec { default: LevelFilter, directives: Vec<(String, LevelFilter)> }` has
`LogSpec::parse(&str) -> Result<LogSpec, LogSpecError>` and a `Display` that round-trips,
so the local daemon gets its parent's exact string. Filtering is a `Filter` impl on the
layer. The directive whose target is the longest prefix match at a `::` or string boundary
wins. With no matching directive, a target containing `::` (third-party) is capped at
`min(default, WARN)`, which keeps today's third-party suppression. Other targets use
`default`. `applog::Level` is deleted. Its callers (`parse_log_level_arg`, mbvd's
`parse_log_level`, `Action::Serve`, `local_daemon_args`) take `LogSpec`. The existing CLI
parse tests change to `LogSpec`. `init` also sets `log::set_max_level` from the spec's
most verbose level, so the bridge doesn't drop records before the filter sees them.

Rejected: reusing `tracing_subscriber::filter::Targets`. It has no "cap third-party at warn
unless named" rule, and a hand-written ~40-line matcher is simpler than wrapping it.

### D4. Call-site conventions (what the conversion tasks follow)
- Macro form: `tracing::warn!(name: "player.load.failed", target: "player", slot = %slot,
  error = %e, "load failed")`. Keep each site's existing bare target and its level.
- Every value that was interpolated into the message becomes a field. The message is a
  short fixed phrase with no `{}`. Use `%` (Display) for ids and errors and bare values
  for numbers and bools. Field names: the D5 correlation keys where they apply, OTel names
  where one fits (`http.method`, `http.status`, `url.path`, `file.path`, `error`), and
  plain snake_case otherwise.
- Event names are `<target-ish component>.<operation>.<state>`, lowercase with `_` inside
  a segment.
- `Cargo.lock` has `tracing` 0.1.44, which supports `name:` in level macros. Pin the
  workspace requirement to `0.1.44`.

### D5. Correlation spans at four boundaries
Spans use the D4 target convention and are entered for the duration of the work, on the
thread doing it.
- **Playback intent**: TUI side — `RemotePlayer::send_playback_intent`
  (`crates/mbv-remote-player/src/lib.rs`) logs `ctrl.intent.sent` with `request`,
  `generation`. Daemon side — the ctrl handler that dispatches `CtrlCmd::PlaybackIntent`
  into `PlaybackIntentState::accept` (`crates/mbv-daemon/src/core.rs`) runs inside a
  `ctrl.intent` span with `request`, `generation`.
- **Queue load**: `RemotePlayer::load_queue_idle` logs `queue.load.sent` with
  `queue_request`. `handle_queue_load_idle` (`crates/mbv-daemon/src/control/queue_load.rs`)
  runs inside a `queue.load` span with `queue_request`.
- **Playback session**: the per-file playback loop in `mbv-player` (the `PlaybackRun`
  that owns the active slot, `crates/mbv-player/src/run/`) enters a `playback` span with
  `slot` and `item`, re-created on each slot change. `play_session` is recorded on the
  span with `Span::record` once the Emby session id is known. Declare it as
  `play_session = tracing::field::Empty` up front. Report/progress code called from inside
  inherits these fields.
- **HTTP**: a ureq 3 `Middleware` in `mbv-net`, installed wherever the Emby, Audiobookshelf
  and feed agents are built. It opens an `http.request` span with `service`,
  `http.method`, `url.path` (path only, no query) and logs one `http.request.done`
  (debug) or `http.request.failed` (warn) event with `http.status` and `duration_ms`.
  ureq 3.4.2 accepts a plain `Fn(Request, MiddlewareNext) -> Result<Response, Error>` as
  middleware (`ureq::middleware`), set on the agent config.

The same key names in every process give cross-process correlation (`rg 'request=42'
mbv.log local-daemon.log`) without a protocol change.

### D6. Redaction in the formatter, not at call sites
The line formatter scrubs the values of `api_key=`, `X-Emby-Token=` and `token=`
(case-insensitive, up to the next `&`, whitespace or quote) in every field value and the
message, replacing them with `REDACTED`. That covers converted sites, unconverted `log`
sites and third-party lines in one place. Headers and credentials must not be logged at all.
Conversion tasks check for any field named like `token`, `key`, `password` or `credential`
and remove it.

Rejected: a `RedactedUrl` wrapper at call sites. It relies on every caller remembering it
and misses messages from the bridge.

### D7. Timestamps from `localtime_r`, not `time`'s `local-offset`
`time`'s `UtcOffset::current_local_offset` refuses to work in a multi-threaded process on
Unix, and all three binaries are multi-threaded. Keep the existing `libc::localtime_r`
call. Take milliseconds from `SystemTime` sub-seconds and the offset from `tm.tm_gmtoff`.
Split this into an `unsafe` wrapper returning plain parts and a pure formatter
(`parts → "YYYY-MM-DDTHH:MM:SS.mmm±HH:MM"`) that tests can call.

### D8. File sink with in-process rotation
`FileSink { path, file: Option<File>, len: u64 }` sits behind the layer's `Mutex`. Before
each write, if `len + line_len > 5_000_000`, it rotates: remove `.3`, rename `.2`→`.3`,
`.1`→`.2`, `log`→`.1`, then reopen. `init` does the same check at startup. Any
`create_dir_all`, open or rename error prints one `mbv: log file <path>: <error>` to stderr.
After an open failure the sink stays `None`. After a failed rotation, writing continues to
the current file and the warning is printed only once. The size limit is a constructor
argument so tests can use a tiny one in a temp directory.

### D9. Stderr sink
Stderr sink behaviour is unchanged: `<prio>` + line, trace maps to `<7>`. The system
`mbvd` and a system-instance TUI still enable it.

## Tests (contracts and owning layer — all `mbv-core` unit tests unless noted)
- `LogSpec::parse`: bare level, target directive, trace, and the rejected forms
  (`#[case]` table, only cases with different outcomes). Display round-trip.
- Filter: a longest-prefix directive wins, and third-party is capped at warn unless named.
- Line formatter: key order, quoting and escaping, span fragments in outer→inner order.
- Timestamp formatter: fixed parts → exact string, including a negative offset.
- Redaction: `api_key` in a field value and in the message.
- Rotation: temp directory with a tiny limit. Generations shift and are capped at 3.
- Span inheritance: an event inside a span carries the span's fields (a subscriber built
  on an in-memory sink, `tracing::subscriber::with_default`, no global init).
- CLI parse tests in `src/main.rs` and `crates/mbvd/src/tests.rs` switch to `LogSpec`
  (existing tests, extended with one directive case).
- Call-site conversion itself gets no new tests. It is mechanical, and clippy plus
  compiling are the check.

## Risks / Trade-offs

- [415-site conversion is large and tedious] → Split by crate into tasks each sized for one
  agent. The `log` bridge keeps unconverted crates working, so any order is safe.
- [Log volume grows with fields] → Fields replace interpolated text, so size stays about the
  same. The HTTP done-event is debug level.
- [Formatter-level redaction can miss new credential shapes] → The pattern list is in one
  place. Conversion tasks also remove credential-named fields.
- [Log format change breaks external parsers] → No known consumers. It's called out as
  BREAKING in the proposal.

## Migration Plan

Ship in one release. No persisted state changes. Rolling back means reverting the change.
The old `.log.old` files stay where they are.
