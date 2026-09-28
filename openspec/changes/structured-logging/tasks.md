# Tasks

Conventions for every conversion task are in design.md D4 (macro form, fields, event
names) and D6 (credential-named fields are removed). Each task ends with `cargo fmt`,
`cargo clippy -p <crates touched> --all-targets -- -D warnings`, and `cargo nextest run -p
<crates touched>` passing.

## 1. Logging core (`mbv-core`)

- [ ] 1.1 Add `tracing = "0.1.44"` to `[workspace.dependencies]`. Add `tracing`,
  `tracing-subscriber` (`default-features = false`, features `registry`, `std`) and
  `tracing-log` to `crates/mbv-core/Cargo.toml`. Verify with `cargo check -p mbv-core` and
  confirm `cargo tree -p mbv-core -e normal | rg -c 'tokio|regex'` finds nothing new.
- [ ] 1.2 Add `applog/spec.rs`: `LogSpec`, `LogSpecError`, `parse`, `Display`
  round-trip, and the target filter (design D3). Verify with the `#[case]` parse tests and
  the filter tests named in design.md "Tests".
- [ ] 1.3 Add `applog/time.rs` (`localtime_r` wrapper returning parts, plus a pure RFC 3339
  ms formatter, D7) and `applog/line.rs` (logfmt line formatter, value quoting/escaping,
  D6 redaction). Verify with the timestamp, line and redaction unit tests from design.md.
- [ ] 1.4 Add `applog/sink.rs`: `FileSink` with size rotation (3 generations), and the
  one-time stderr warning on open, rotate or create-dir failure (D8). Keep the stderr
  `<prio>` sink (D9). Verify with the temp-directory rotation test and the existing
  `stderr_line_has_systemd_priority_prefix` test, extended with a trace case.
- [ ] 1.5 Rewrite `applog.rs`: the logfmt `Layer` (span fragments in extensions, D2), the
  `Registry` + filter, the `tracing-log` `LogTracer` bridge, and
  `init(stderr, log_path, &LogSpec)`. Delete `applog::Level`, `LogEntry`, `GlobalLogger`.
  Verify with the span-inheritance test (in-memory sink, `with_default`) and the rewritten
  `init_at_info_disables_debug_records` test.

## 2. CLI plumbing

- [ ] 2.1 Switch `--log-level` parsing to `LogSpec`: `parse_log_level_arg` in
  `src/main.rs`, `parse_log_level` and `Action::Serve.log_level` in
  `crates/mbvd/src/main.rs`, `local_daemon_args` and `run_local_daemon_main` in
  `src/local_daemon.rs` (pass `spec.to_string()`), and the three `applog::init` callers.
  Update the usage text to show the directive form. Extend the existing parse tests in
  `src/main.rs` and `crates/mbvd/src/tests.rs` with one directive case and one malformed
  case. Verify: `cargo nextest run -p mbv -p mbvd` passes, and `cargo run --bin mbvd --
  --log-level info,player=` prints usage and exits non-zero.

## 3. Correlation spans

- [ ] 3.1 Playback intent and queue load (D5). In `mbv-remote-player`, add the
  `ctrl.intent.sent` and `queue.load.sent` events. In `mbv-daemon`, add the `ctrl.intent`
  span around the handler that feeds `PlaybackIntentState::accept`, and the `queue.load`
  span around `handle_queue_load_idle`. Verify: clippy/nextest for both crates, and by
  reading the code, each span is entered for the whole handler body.
- [ ] 3.2 Playback session span in `mbv-player` (D5): the `playback` span, created per
  active slot with `slot`, `item` and `play_session = Empty`. Record `play_session` when the
  Emby session id is assigned. Verify: clippy/nextest for `mbv-player`, and by reading the
  code, the span is re-created on each slot change and entered on the playback thread.
- [ ] 3.3 HTTP middleware in `mbv-net` (D5), installed on the Emby, Audiobookshelf and feed
  agents. It logs `http.request.done`/`http.request.failed` with `service`, `http.method`,
  `url.path`, `http.status`, `duration_ms`. Verify: one unit test in `mbv-net` through its
  existing mock HTTP seam, asserting the failed event carries `http.status`. clippy/nextest
  pass for `mbv-net`, `mbv-emby`, `mbv-audiobookshelf` and `mbv-feed`.

## 4. Call-site conversion (design D4; drop the crate's `log` dependency when it has no `log::` uses left)

- [ ] 4.1 `mbv-player` (~103 sites). Verify: `rg 'log::' crates/mbv-player/src` is empty and
  the crate's checks pass.
- [ ] 4.2 `mbv-daemon`, `mbvd`, `mbv-remote-player` (~54 sites). Verify: `rg 'log::'` is
  empty in all three and their checks pass.
- [ ] 4.3 `mbv-emby`, `mbv-audiobookshelf`, `mbv-ws`, `mbv-net`, `mbv-cast`, `mbv-feed`
  (~76 sites). Verify: `rg 'log::'` is empty in all six and their checks pass.
- [ ] 4.4 `mbv-config`, `mbv-desktop`, `mbv-visualizer`, `mbv-images`, and the TUI's
  `src/main.rs`, `src/local_daemon.rs`, `src/config.rs` (~23 sites). Verify: `rg 'log::'`
  is empty in those paths and their checks pass.
- [ ] 4.5 `src/app/dispatch/` (~119 sites). Verify: `rg 'log::' src/app/dispatch` is empty
  and `cargo nextest run -p mbv` passes.
- [ ] 4.6 The rest of `src/app/` (`shell/`, `state/`, `infra/`, ~40 sites). Verify: `rg 'log::' src` is empty, the root crate's `log`
  dependency is removed, and `cargo nextest run -p mbv` passes.

## 5. Wrap-up

- [ ] 5.1 Workspace gate: `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check` and `cargo nextest run --workspace` pass. `rg -l '"log"|log\.workspace'
  crates/*/Cargo.toml Cargo.toml` lists only `mbv-core`, which still needs `log` for the
  bridge's `set_max_level`.
- [ ] 5.2 Manual check (user): run `mbv --log-level info,player=debug`, play an Emby item,
  and confirm in `mbv.log` and `local-daemon.log` that the RFC 3339 `ts`, `event=`, and the
  shared `request=`/`slot=` values match up across the two files, and that no `api_key`
  value appears.
- [ ] 5.3 Before pushing, run `make check-code-file-lines`. Split any file over 800 lines
  along responsibility seams.
