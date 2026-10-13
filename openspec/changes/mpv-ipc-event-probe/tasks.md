# Tasks

## 1. Domain event type on the libmpv path (no behavior change)

- [x] 1.1 Add the crate-private `crates/mbv-player/src/mpv_event.rs` with `MpvEvent`, `PropertyValue`, `MpvFailure` and a domain `EndFileReason` enum (`Eof`, `Stop`, `Quit`, `Redirect`), as in design D5. Add `MpvEvent::from_libmpv(Result<libmpv2::Event, libmpv2::Error>)`: `Err(Raw(AoInitFailed))` becomes `Failed(AudioOutputInit)`, any other `Err` becomes `Failed(Other(mpv_err_str(e)))`, and events the run loop does not act on become `Ignored`. Verify: `cargo check -p mbv-player` passes.
- [x] 1.2 Replace the libmpv2 `EndFileReason` alias and the `mpv_end_file_reason` constants with the domain enum at every use site (`lib.rs`, `proxy.rs`, `run.rs`, `run/queue.rs`, `run/events/termination.rs`, `run/events/event_classifiers.rs`, `run/events/queue_advance.rs`, `tests/session.rs`). Make `is_superseded_jump_end_file` take the domain enum. Verify: `cargo nextest run -p mbv-player` passes with no test body changes other than the reason type.
- [x] 1.3 In `run/run_loop.rs`, make `handle_mpv_event` take `MpvEvent` (convert with `from_libmpv` at the `wait_event` call site) and match it exhaustively. Remove the `_ => false` wildcard and make `Ignored` the documented no-op arm. Make `on_mpv_error` and `is_clocked_audio_error` take `&MpvFailure`. Verify: `cargo nextest run -p mbv-player` passes, and `rg "libmpv2::(Event|Error)" crates/mbv-player/src/run` matches only the conversion call site.
- [x] 1.4 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` and verify that both are clean. Commit group 1.

## 2. Dual-backend probe example

- [x] 2.1 Add `crates/mbv-core/examples/mpv_ipc_probe/` (design D1 implementation note) with two backends behind one scenario runner: in-process `libmpv2::Mpv`, and an `mpv` child process started with `--idle=yes --no-config --vo=null --ao=null --no-terminal --input-ipc-server=<scratch path>` (design D2). Both observe the same 11 properties and IDs as `runtime.rs` `observe_properties`. It uses the public `libmpv2` API, `serde_json` and std only, with no crate-private items. Verify: `cargo build -p mbv-core --example mpv_ipc_probe` succeeds.
- [x] 2.2 Add the normalizer and comparison from design D3 (JSON event shape, the 5 normalization rules, results `MATCH`, `MATCH-AFTER-NORMALIZATION` and `DIFF` with the first differing index). Each scenario runs 5 times per backend and every distinct sequence is reported. Verify: the example builds, and `--help` lists the scenarios and the output directory flag.
- [x] 2.3 Add scenarios S1 to S10 from design D4, using `av://lavfi:` media only. Write each IPC trace as raw lines to `crates/mbv-player/src/mpv_event/fixtures/<id>.jsonl`, with a first header line that records `mpv --version` and the date. Verify: the example builds and clippy is clean. Do not run it (see 3.1). Commit group 2.

## 3. Capture traces (the user runs the probe)

- [x] 3.1 Ask the user to run `cargo run -p mbv-core --example mpv_ipc_probe -- --out crates/mbv-player/src/mpv_event/fixtures`. Explain that it starts real mpv processes with synthetic media, `vo=null` and `ao=null`, plus one `ao` failure scenario. Do not run it without a yes. Verify: 10 fixture files exist, and the user has pasted or saved the probe's comparison output.
- [x] 3.2 Commit the fixture files. Verify: `git status` is clean.

## 4. IPC decoder and tests on real traces

- [ ] 4.1 Add `MpvEvent::from_ipc_line(&str) -> Result<MpvEvent, DecodeError>` with a domain `DecodeError` type (design D5). Type property values by property name, decode `end-file` with `reason:"error"` to `Failed`, and compare `file_error` with the runtime `mpv_error_string` for `AoInitFailed`. Known events the run loop does not act on become `Ignored`. Skip the fixture header line. Verify: `cargo check -p mbv-player` passes.
- [ ] 4.2 Add `crates/mbv-player/src/mpv_event/tests.rs` with the `decode_fixture` helper and the tests from design D6: S2 debris classification, S6 `AudioOutputInit` with `is_clocked_audio_error` true, S5 `Failed(Other(_))` with `is_clocked_audio_error` false, S1 `EndFile(Eof)`, and one malformed-line `DecodeError` test. Each test cites its scenario ID in a comment. Verify: `cargo nextest run -p mbv-player` passes.
- [ ] 4.3 Run `cargo fmt` and clippy with `-D warnings` and verify that both are clean. Commit group 4.

## 5. Findings

- [ ] 5.1 Fill the Findings section of `design.md` from the probe output: the result for each scenario, the normalizations used, any nondeterminism, the IPC-only fields seen (`playlist_entry_id` and others), and the 5 synchronous `get_property` reads as migration input. End with a recommendation: move mpv out of process, stay in process, or block on a named gap. Verify: every scenario S1 to S10 has a row. Commit.

## Workflow follow-up

- If the recommendation is to move, start a separate change for the out-of-process migration with `/opsx:propose`.
- Archive this change after review.
