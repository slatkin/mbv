# Tasks

Conventions for every conversion task are in design.md D4 (macro form, fields, event
names) and D6 (credential-named fields are removed). Each task ends with `cargo fmt`,
`cargo clippy -p <crates touched> --all-targets -- -D warnings`, and `cargo nextest run -p
<crates touched>` passing.

## 1. Logging core (`mbv-core`)

- [x] 1.1 Add `tracing = "0.1.44"` to `[workspace.dependencies]`. Add `tracing`,
  `tracing-subscriber` (`default-features = false`, features `registry`, `std`) and
  `tracing-log` to `crates/mbv-core/Cargo.toml`. Add the `formatting`, `local-offset` and
  `macros` features to the workspace `time`, add `time` to `mbv-core`, and remove `libc`
  from `mbv-core` (D7). Verify with `cargo check -p mbv-core` and confirm
  `cargo tree -p mbv-core -e normal | rg -c 'tokio|regex'` finds nothing new.
- [x] 1.2 Add `applog/spec.rs`: `LogSpec`, `LogSpecError`, `parse`, `Display`
  round-trip, and the target filter (design D3). Verify with the `#[case]` parse tests and
  the filter tests named in design.md "Tests".
- [ ] 1.3 Add `applog/time.rs` (`format_ts` over `time::OffsetDateTime`, `now_local` with a
  UTC fallback, D7; the old `unsafe` `now_ts` is deleted) and `applog/line.rs` (logfmt line
  formatter, value quoting/escaping, D6 URL/bearer redaction). Verify with the timestamp,
  line and redaction unit tests from design.md, including the unlisted `?password=` and
  `user:pass@` cases.
- [x] 1.4 Add `applog/sink.rs`: `FileSink` with size rotation (3 generations), and the
  one-time stderr warning on open, rotate or create-dir failure (D8). Keep the stderr
  `<prio>` sink (D9). Verify with the temp-directory rotation test and the existing
  `stderr_line_has_systemd_priority_prefix` test, extended with a trace case.
- [x] 1.5 Rewrite `applog.rs`: the logfmt `Layer` (span fragments in extensions, D2), the
  `Registry` + filter, the `tracing-log` `LogTracer` bridge, and
  `init(stderr, log_path, &LogSpec)`, `carry_context(f)` (carries the current span and
  dispatcher) and `carry_dispatcher(f)` (carries only the dispatcher; the thread starts
  with no span), per design D5's thread-spawn rule. Delete `applog::Level`, `LogEntry`,
  `GlobalLogger`. Verify with the span-inheritance test (in-memory sink, `with_default`),
  the `carry_context`/`carry_dispatcher` cross-thread tests, and the rewritten
  `init_at_info_disables_debug_records` test.

## 2. CLI plumbing

- [x] 2.1 Switch `--log-level` parsing to `LogSpec`: `parse_log_level_arg` in
  `src/main.rs`, `parse_log_level` and `Action::Serve.log_level` in
  `crates/mbvd/src/main.rs`, `local_daemon_args` and `run_local_daemon_main` in
  `src/local_daemon.rs` (pass `spec.to_string()`), and the three `applog::init` callers.
  Update the usage text to show the directive form. Extend the existing parse tests in
  `src/main.rs` and `crates/mbvd/src/tests.rs` with one directive case and one malformed
  case. Verify: `cargo nextest run -p mbv -p mbvd` passes, and `cargo run --bin mbvd --
  --log-level info,player=` prints usage and exits non-zero.

## 3. Correlation spans

- [ ] 3.1 Ctrl connections, playback intent and queue load (D5).
  - `mbv-remote-player`: the `ctrl.connected` event (`peer` = own pid on a Unix endpoint,
    `TcpStream::local_addr` on a TCP endpoint), and the `ctrl.intent.sent` and
    `queue.load.sent` events.
  - `mbv-daemon`:
    - `ctrl.client.connected` (`client`, and `peer` = `SO_PEERCRED` pid, or
      `TcpStream::peer_addr`) where `ClientRegistry` registers a connection.
    - The `ctrl.intent` span (`client`, `request`, `generation`) around the handler that
      feeds `PlaybackIntentState::accept`.
    - The thread-spawn rule (`carry_context`) in `spawn_item_lookup`.
    - The rejoin rule in `handle_playback_resolved`.
    - The `queue.load` span (`client`, `queue_request`) around `handle_queue_load_idle`.
    - The new `client_id` field on `PendingIdleQueueLoad`.
    - The rejoin rule in `complete_pending_idle_queue_load`.

  Verify: the rejoin-correlation test from design.md "Tests" passes in the
  `src/tests/loop.rs` harness, and clippy/nextest pass for both crates.
- [ ] 3.2 Playback session and reporting spans in `mbv-player` (D5):
  - The `playback` span, created per active slot with `slot`, `item` and
    `play_session = Empty`, and `Span::record` when the Emby session id is assigned.
  - The `playback.report` span, following design D5's per-case rule:
    - `report_progress`/`report_ping` use the shared `ids` at call time.
    - `Stopped` uses the data's ids.
    - `Start`/`Resolved` uses the item and the resolved session.
    - `Start`/`Deferred` uses the new `item.id` with `play_session = Empty`, recorded after
      `get_playback_info`. It never reads the shared `ids` at job entry.

    Wrap the three reporter thread spawns (`spawn_progress_reporter`, the `run_loop.rs`
    progress worker, and `SessionReporter::new`) in `carry_dispatcher`, not
    `carry_context`. They outlive the slot, so they must not inherit its `playback` span.

  Verify: the reporter-correlation test from design.md "Tests" passes, and clippy/nextest
  pass for `mbv-player`.
- [ ] 3.3 HTTP logging in `mbv-net` (D5):
  - The `HttpService` enum and `agent_config(service, connect, global)`, which installs
    the middleware.
  - `native_tls_agent` gains the required `service` argument, with variants `Emby`,
    `Audiobookshelf`, `Feeds`. Update its callers:
    - Emby `emby_agent`
    - `AudiobookshelfClient::new`
    - Feeds `tls_agent`
    - the two TUI Emby sites, `image_fetch/protocol.rs` `fetch_url` and
      `image_fetch/fetch/level_warmup.rs`, both passing `HttpService::Emby`.
  - `MockHttp::agent_for(service)` built from `agent_config`.
  - Events `http.request.done`/`http.request.failed` with `service`,
    `http.request.method`, `url.path`, `http.response.status_code`, `duration_ms`.

  Verify: the `agent_for` HTTP test from design.md "Tests" passes, and
  `rg 'native_tls_agent\(' --type rust` shows every call passing an `HttpService`.
  clippy/nextest pass for `mbv-net`, `mbv-emby`, `mbv-audiobookshelf`, `mbv-feed` and `mbv`.

## 4. Call-site conversion (design D4; drop the crate's `log` dependency when it has no `log::` uses left)

- [ ] 4.1 `mbv-player` (~103 sites). Verify: `rg 'log::' crates/mbv-player/src` is empty and
  the crate's checks pass.
- [ ] 4.2 `mbv-daemon`, `mbvd`, `mbv-remote-player` (~54 sites). Verify: `rg 'log::'` is
  empty in all three and their checks pass.
- [ ] 4.3 `mbv-emby`, `mbv-audiobookshelf`, `mbv-ws`, `mbv-net`, `mbv-cast`, `mbv-feed`
  (~76 sites). Verify: `rg 'log::'` is empty in all six and their checks pass.
- [ ] 4.4 `mbv-config`, `mbv-desktop`, `mbv-visualizer`, `mbv-images`, and the TUI's
  `src/main.rs`, `src/local_daemon.rs`, `src/config.rs` (~23 sites). Also remove the unused
  `log.workspace = true` from `crates/mbv-ui-model/Cargo.toml`. Verify: `rg 'log::'` is
  empty in those paths, `mbv-ui-model` no longer lists `log`, and their checks pass.
- [ ] 4.5 `src/app/dispatch/` (~119 sites). Verify: `rg 'log::' src/app/dispatch` is empty
  and `cargo nextest run -p mbv` passes.
- [x] 4.6 The rest of `src/app/` (`shell/`, `state/`, `infra/`, ~40 sites). Verify: `rg 'log::' src` is empty, the root crate's `log`
  dependency is removed, and `cargo nextest run -p mbv` passes.

## 5. Wrap-up

- [ ] 5.1 Workspace gate: `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check` and `cargo nextest run --workspace` pass. `rg -l '"log"|log\.workspace'
  crates/*/Cargo.toml Cargo.toml` lists only `mbv-core`, which still needs `log` for the
  bridge's `set_max_level`.
- [ ] 5.2 Manual check (user): run `mbv --log-level info,player=debug`, play an Emby item,
  and confirm in `mbv.log` and `local-daemon.log` that the RFC 3339 `ts`, `event=`, and the
  `request=` values match up across the two files, with the TUI's `ctrl.connected` line and
  the local daemon's `ctrl.client.connected` line showing the same `peer=` (the TUI's
  pid). Confirm that no `api_key` value appears. If a TCP `mbvd` is available, repeat the
  connection and confirm both connect lines show the same `peer=` `ip:port`.
- [ ] 5.3 Before pushing, run `make check-code-file-lines`. Split any file over 800 lines
  along responsibility seams.
