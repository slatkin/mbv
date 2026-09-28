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
  for numbers and bools. Field names: the D5 correlation keys where they apply, OTel
  semantic-convention names where one fits, as `M-LOG-STRUCTURED` requires
  (`http.request.method`, `http.response.status_code`, `url.path`, `file.path`,
  `error.message`), and plain snake_case otherwise.
- Never log request headers, request bodies, or `Debug` output of credential or config
  structs. Delete any field named like `token`, `key`, `password` or `credential`.
- Event names are `<target-ish component>.<operation>.<state>`, lowercase with `_` inside
  a segment.
- `Cargo.lock` has `tracing` 0.1.44, which supports `name:` in level macros. Pin the
  workspace requirement to `0.1.44`.

### D5. Correlation spans, carried across every handoff
Spans use the D4 target convention. A span entered only for a handler body would lose
context wherever an operation hands off to another thread or resumes later on the event
loop, so there are two propagation rules:
- **Thread spawn**: code in these paths wraps its thread body in
  `mbv_core::applog::carry_context(f)`. On the spawning thread, it captures both
  `tracing::Span::current()` and the current dispatcher
  (`tracing::dispatcher::get_default(Clone::clone)`). It returns a closure that runs `f`
  under `tracing::dispatcher::with_default` with the span entered. The dispatcher has to
  be carried too: an event goes to the default subscriber of the thread it's logged on,
  so a test's `with_default` subscriber would not see events from a worker thread (review
  N3). In production the global dispatcher is used either way.
- **Event-loop rejoin**: a handler that finishes a deferred operation rebuilds the span
  from the ids the rejoining event or parked state already carries. It does not store the
  `Span` itself, which would duplicate those ids.

Boundaries:
- **Ctrl connections** (B3 in the review): request ids are per connection, and a new
  `RemotePlayer` and a new TUI both restart their counters at 1. So:
  - The owner's ctrl spans always carry `client` (its `CtrlClientId`).
  - When `ClientRegistry` registers a connection (`crates/mbv-daemon/src/ctrl.rs`), it
    logs `ctrl.client.connected` with `client` and `peer`. For a Unix socket, `peer` is
    the process id from `SO_PEERCRED`, via `nix::sys::socket::getsockopt(…,
    PeerCredentials)`; the `socket` feature is already enabled. For TCP it is the peer
    address from `TcpStream::peer_addr` (`ip:port`).
  - Each time `RemotePlayer` connects (`crates/mbv-remote-player/src/connect.rs`, after
    the handshake), it logs `ctrl.connected` with a `peer` field that uses the owner's
    key and value format:
    - Unix endpoint: its own process id (`std::process::id()`).
    - TCP endpoint (`connect/endpoint.rs`, `SocketStream::Tcp`): its local address from
      `TcpStream::local_addr` (`ip:port`).
  - Matching is an exact `peer=` join between the TUI's `ctrl.connected` line and the
    owner's `ctrl.client.connected` line, which gives the owner's `client`. Then the
    `client`+`request` pair.
  - Scope: guaranteed for Unix sockets. For TCP it is guaranteed only when no address
    translation (NAT, proxy) sits between the TUI and the owner, because otherwise the
    owner sees a translated address. With translation, lines can still be matched by
    `request` and time, but that isn't guaranteed. No protocol change.
- **Playback intent**:
  - TUI: `RemotePlayer::send_playback_intent` (`crates/mbv-remote-player/src/lib.rs`)
    logs `ctrl.intent.sent` with `request`, `generation`.
  - Owner: the handler that feeds `PlaybackIntentState::accept` runs inside a
    `ctrl.intent` span with `client`, `request`, `generation`.
  - `spawn_item_lookup` (`crates/mbv-daemon/src/control/playback.rs`) follows the
    thread-spawn rule.
  - `handle_playback_resolved` (`crates/mbv-daemon/src/event_loop/control_events.rs`)
    follows the rejoin rule, using its `client_id`/`request_id`/`generation` arguments.
- **Queue load**:
  - TUI: `RemotePlayer::load_queue_idle` logs `queue.load.sent` with `queue_request`.
  - Owner: `handle_queue_load_idle` (`crates/mbv-daemon/src/control/queue_load.rs`) runs
    inside a `queue.load` span with `client`, `queue_request`.
  - `PendingIdleQueueLoad` gains `client_id: CtrlClientId`, set from `ctx.client_id` when
    the load is parked.
  - `complete_pending_idle_queue_load` follows the rejoin rule, using the parked
    `client_id`/`request_id`.
- **Playback session**:
  - The per-file playback loop in `mbv-player` (the `PlaybackRun` that owns the active
    slot, `crates/mbv-player/src/run/`) enters a `playback` span with `slot`, `item` and
    `play_session = tracing::field::Empty`. The span is re-created on each slot change.
    `play_session` is filled in with `Span::record` once known.
  - The reporting threads are longer-lived than one slot and don't know about slots:
    - the progress reporter (`runtime.rs` `spawn_progress_reporter`)
    - the progress worker (`run/run_loop.rs`)
    - the report worker (`report_worker.rs` `SessionReporter::new`)

    So they don't inherit the playback span. Instead they get a `playback.report` span
    (`item`, `play_session`), built per call or per job. The rule differs by case,
    because the shared `ids` can still name the previous session while a deferred Start
    is resolving:
    - `SessionReporter::report_progress` / `report_ping`: built from the shared `ids` at
      call time. These are exactly the ids the report sends, so the label matches the
      request even inside the deferred window.
    - `ReportJob::Stopped(data)` and the `stopped` data in `ProgressJoinThenStopped`: from
      `data.id` / `data.sid`.
    - `ReportJob::Start` with `StartIds::Resolved`: from `item.id` and the resolved
      `session_id`.
    - `ReportJob::Start` with `StartIds::Deferred`: `item` comes from the new `item.id`,
      and `play_session = Empty`. The shared `ids` are never read at job entry.
      `play_session` is recorded with `Span::record` right after `get_playback_info`
      returns (next to the existing `locked.2 = info.session_id` update in
      `report_worker.rs`).

    `item`/`play_session` link these lines to the `playback` span's lines.
- **HTTP**: every production agent is built by `mbv_net::native_tls_agent`. Its callers
  are Emby `emby_agent`, Audiobookshelf `AudiobookshelfClient::new`, Feeds `tls_agent`,
  and two TUI sites that both call Emby: `image_fetch/protocol.rs` `fetch_url` (Emby
  item images and child lookups) and `image_fetch/fetch/level_warmup.rs` (Emby
  album-artist items).
  - Split it into `agent_config(service: HttpService, connect, global) -> ureq::config::Config`
    plus the agent build. `HttpService` is a new enum with one variant per `CONTEXT.md`
    Service: `Emby`, `Audiobookshelf`, `Feeds`. It is logged as `service=emby`,
    `service=audiobookshelf` or `service=feeds`. Both TUI sites pass `HttpService::Emby`.
  - `agent_config` installs the logging middleware. ureq 3.4.2 accepts a plain
    `Fn(Request, MiddlewareNext) -> Result<Response, Error>` (`ureq::middleware`).
  - The `service` argument is required, so no provider agent can be built without
    naming its service and getting the middleware.
  - The middleware runs inside an `http.request` span with `service`,
    `http.request.method` and `url.path` (path only). It logs `http.request.done` (debug)
    or `http.request.failed` (warn) with `http.response.status_code` (when there is a
    response) and `duration_ms`.
  - `MockHttp` gains `agent_for(service)`, which builds from the same `agent_config`, so
    tests go through the production configuration.

### D6. URL and bearer redaction in the formatter
A list of credential parameter names can't cover every shape. Feed enclosure URLs are
arbitrary third-party URLs (`?password=…`, signed-URL parameters, `user:pass@host`). So
the line formatter rewrites every URL-shaped substring (`<scheme>://…` up to whitespace
or a quote) in every field value and in the message:
- It drops the username/password part, the query string and the fragment, keeping
  scheme, host, port and path.
- It replaces `Bearer <token>` with `Bearer REDACTED`.

That covers converted sites, unconverted `log` sites, third-party lines, and error
messages that embed URLs (ureq errors do), all in one place. The trade-off is that query
parameters are never visible in logs. Anything worth logging from a query gets its own
field at the call site. Credentials that aren't in URLs (login password bodies, the
control token in `CtrlHello`, the Audiobookshelf `Authorization` header) are kept out by
the D4 rule, which conversion tasks apply.

Rejected:
- A list of named query parameters: misses unknown shapes (review B2).
- A `RedactedUrl` wrapper at call sites: depends on every caller remembering it, and
  misses messages from the bridge.

### D7. Timestamps from `time` with `local-offset`
The workspace `time` 0.3.55 gets the `formatting`, `local-offset` and `macros` features.
Its Unix local-offset lookup calls `localtime_r` directly and has no multi-thread
restriction. `mbv-core` takes `time` as a dependency and drops `libc`, along with
`now_ts`'s `unsafe` block.
- `OffsetDateTime::now_local()`, falling back to `now_utc()` if it errors, formatted with
  a `format_description!` of
  `[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]`.
- The formatter is a pure `fn format_ts(OffsetDateTime) -> String`, so tests can pass
  fixed `datetime!` values.

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
- Timestamp formatter: fixed `datetime!` → exact string, including a negative offset.
- Redaction: `?api_key=` in a field value, an unlisted `?password=` query, `user:pass@`
  userinfo in the message, and a `Bearer` token.
- Rotation: temp directory with a tiny limit. Generations shift and are capped at 3.
- Span inheritance: an event inside a span carries the span's fields (a subscriber built
  on an in-memory sink, `tracing::subscriber::with_default`, no global init).
- Rejoin correlation (`mbv-daemon`, `src/tests/loop.rs` harness): a failed
  `PlaybackResolved` handled on the event loop logs a line carrying the intent's `client`
  and `request`. The test uses a thread-local capture subscriber.
- Reporter correlation (`mbv-player`, existing `src/tests/` reporter fixtures): a deferred
  `ReportJob::Start` for a new item, run with the shared `ids` still holding the previous
  session:
  - a line logged before `get_playback_info` returns carries the new `item` and no
    `play_session`;
  - a line logged after it returns carries the new `item` and the resolved
    `play_session`.

  The report worker runs on its own thread. The test sees its events because the spawn
  goes through `carry_context`, which carries the test's `with_default` dispatcher.
- `carry_context` (`mbv-core`): an event logged on a thread spawned through
  `carry_context` reaches the spawning thread's `with_default` subscriber and carries the
  spawner's span fields.
- HTTP (`mbv-net`): an agent from `MockHttp::agent_for(HttpService::Emby)` given a 500
  logs `http.request.failed` with `service=emby` and `http.response.status_code=500`.
- The connect lines have no unit test. The `peer` value comes straight from
  `peer_addr`/`local_addr`/`SO_PEERCRED` and is checked manually (task 5.2).
- CLI parse tests in `src/main.rs` and `crates/mbvd/src/tests.rs` switch to `LogSpec`
  (existing tests, extended with one directive case).
- Call-site conversion itself gets no new tests. It is mechanical, and clippy plus
  compiling are the check.

## Risks / Trade-offs

- [415-site conversion is large and tedious] → Split by crate into tasks each sized for one
  agent. The `log` bridge keeps unconverted crates working, so any order is safe.
- [Log volume grows with fields] → Fields replace interpolated text, so size stays about the
  same. The HTTP done-event is debug level.
- [Redaction hides query parameters that might help debugging] → Values worth logging get
  their own field at the call site. URL credentials can't leak through any other path.
- [Handoffs added later may not propagate context] → The two rules in D5 are stated as
  conventions. Code review catches a new `thread::spawn` or rejoin in these paths that
  doesn't follow them.
- [Log format change breaks external parsers] → No known consumers. It's called out as
  BREAKING in the proposal.

## Migration Plan

Ship in one release. No persisted state changes. Rolling back means reverting the change.
The old `.log.old` files stay where they are.
