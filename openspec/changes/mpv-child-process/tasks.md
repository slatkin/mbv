# Tasks

The libmpv-to-IPC swap lands in group 4. Groups 1 to 3 keep libmpv as the live backend, so each of them ends with a green gate: `cargo nextest run -p mbv-player`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings`. Do not run mpv in any test. Commit each group.

## 1. Domain `MpvError` (still on libmpv)

- [x] 1.1 Add the crate-private `MpvError` in `crates/mbv-player/src/mpv_error.rs` with the cases `Spawn`, `NotFound`, `Io`, `Reply(String)`, `Timeout`, `Closed`, `Decode`, and `Libmpv(String)` (carries `mpv_err_str` text until group 4 removes it). It implements `Display` and `std::error::Error`. Verify: `cargo check -p mbv-player` passes. (Done with `Libmpv` only: the other cases have no producer until group 3 and fail the `dead_code` gate. Add each in group 3 with its first producer.)
- [x] 1.2 Replace `libmpv2::Error` in `error.rs` (`PlayerErrorKind::MpvInit`, `set_audio_device`, the two `source:` fields) with `MpvError`. Convert at the existing libmpv call sites. Verify: `rg "libmpv2::Error" crates/mbv-player/src/error.rs` has no match, and `cargo nextest run -p mbv-player` passes.
- [x] 1.3 Run the gate and commit group 1.

## 2. Wrapper `Mpv` (still on libmpv)

- [ ] 2.1 Add `crates/mbv-player/src/mpv_session.rs` with the crate-private struct `Mpv` that wraps `libmpv2::Mpv`. Give it the calls in design D2: `set_property(name, impl Into<PropertyValue>)` with `From` for `i64`, `bool`, `f64`, `&str`, `String`; `get_f64`, `get_i64`, `get_bool`, `get_string`; `command`, `command_async`, `observe_property`, `request_log_messages`, `next_event`, and the wakeup hook. Each returns `Result<_, MpvError>`. Verify: `cargo check -p mbv-player` passes.
- [ ] 2.2 Move `lib.rs`, `runtime.rs`, `controller.rs`, `submit.rs` and `run.rs` to the wrapper (`init_mpv` returns the wrapper). Verify: `rg "libmpv2::Mpv|libmpv2::\{" crates/mbv-player/src/{lib,runtime,controller,submit,run}.rs` has no match, and `cargo nextest run -p mbv-player` passes.
- [ ] 2.3 Move `tracks.rs`, `run/commands.rs`, `run/queue_commands.rs`, `run/events/restart.rs` and `run/run_loop.rs` to the wrapper. Replace each `get_property::<T>` with the typed getter. Verify: `rg "libmpv2" crates/mbv-player/src -g '!mpv_session.rs' -g '!mpv_event*' -g '!mpv_error.rs' -g '!tests/**'` shows no use outside the wrapper, the event types, and `lib.rs` `mpv_err_str`. `cargo nextest run -p mbv-player` passes.
- [ ] 2.4 Replace the two raw FFI calls (`mpv_command_async` in `run_loop.rs`, `mpv_request_log_messages` in `runtime.rs`) with `command_async` and `request_log_messages`. Verify: `rg "libmpv2_sys|\.ctx" crates/mbv-player/src` matches only `mpv_session.rs`, `lib.rs` `mpv_err_str`, and `mpv_event*`.
- [ ] 2.5 Run the gate and commit group 2.

## 3. Child-process session (built and tested, not yet used by the run loop)

- [ ] 3.1 Read mpv's `player/client.c` (`mpv_create` and the client defaults) with `ketch code` or the mpv source. List each default that an embedded client has and the `mpv` program lacks, in a comment above `spawn_args`. Add `spawn_args(&MpvRunConfig) -> Vec<String>` in `mpv_session/spawn.rs` (design D4). Verify: one `#[case]` table test pins the argument list for the `use_mpv_config` / `no_scripts` combinations that `init_mpv` handles, and `cargo nextest run -p mbv-player` passes.
- [ ] 3.2 Add the socket session in `mpv_session/ipc.rs`: a non-blocking `UnixStream`, a line buffer, the event queue, `request` (reply matched by `request_id`, other lines decoded with `MpvEvent::from_ipc_line` and queued in order), `next_event`, and the timeout as a parameter (design D3). Verify: unit tests on a `UnixStream::pair()` peer prove that an event sent before the reply is still delivered, in order, and that a reply that does not come returns `MpvError::Timeout` with a zero timeout.
- [ ] 3.3 Add end-of-file handling and `quit_requested` (design D7): `next_event` returns `Shutdown` once for a close after a quit, and `Failed(ProcessExited)` for a close without one. Add the `ProcessExited` variant to `MpvFailure` (`is_clocked_audio_error` stays false for it). Cite scenario S8 in the test. Verify: two peer tests (close after quit, close without quit) pass.
- [ ] 3.4 Add `connect_with_retry(path, attempts, interval)`, the `Child` start in `mpv_session/spawn.rs` with null stdio and `PR_SET_PDEATHSIG` (one `unsafe` block with a SAFETY comment), and `Drop` (async `quit`, kill, reap). Verify: a test that connects to a missing path with a zero interval returns `MpvError::NotFound` or `Io` as the design says, and clippy is clean with no `allow`.
- [ ] 3.5 Run the gate and commit group 3.

## 4. Swap the backend to the child process

- [ ] 4.1 Add the spawner thread owned by the player controller (design D5). It starts every child, for both `pre_warm` and the on-demand start in `run_player_thread`. Verify: a unit test with a stub start function shows that `pre_warm` and on-demand start both call the spawner (no mpv is started), and `cargo nextest run -p mbv-player` passes.
- [ ] 4.2 Change `init_mpv` to build the arguments, start the child through the spawner, connect, and run the existing post-init `set_property` calls on the new session. Make the wrapper's inside the child session and delete its libmpv2 path. Keep `prepare_ipc_socket`. A missing `mpv` program uses the `player.startup.failed` path with a message that names `mpv`. Verify: `cargo nextest run -p mbv-player` passes, and `rg "libmpv2" crates/mbv-player/src` matches only `mpv_event.rs` (`from_libmpv`) and tests.
- [ ] 4.3 Replace the libmpv wakeup callback and `wait_event(0.0)` in `run/run_loop.rs` with `poll` on the wakeup pipe and the socket, and `next_event`. Update the `WAKEUP_POLL_MS` comment (the lost-byte case is gone, the ceiling stays for timers). Verify: `cargo nextest run -p mbv-player` passes, and `rg "set_wakeup_callback|wait_event" crates/mbv-player/src` has no match.
- [ ] 4.4 Handle `Failed(ProcessExited)` in `on_mpv_error` (design D7): end the run with `PlayerEvent::Stopped { error: Some(..) }` and the last observed position, through the teardown that the audio-output failure uses. Give `PropertyValue::None` its arm in `on_property_change` and stop the `("video-params/h", change)` arm from logging it. Verify: a test that feeds `Failed(ProcessExited)` to the run loop on the existing test seam sees one `Stopped` event with an error. A second test shows that `PropertyValue::None` for `video-params/h` leaves the status unchanged.
- [ ] 4.5 Remove `#[cfg(test)]` from `mpv_event/ipc.rs` and from `PropertyValue::None`, and delete `MpvEvent::from_libmpv`, `PropertyValue::from_libmpv`, `end_file_from_libmpv`, `MpvError::Libmpv`, and `mpv_err_str`. Make the IPC observe calls use `observe_property_string` for `sid`, `aid`, and `audio-codec-name` (design D8). Verify: `rg "libmpv" crates/mbv-player/src` has no match outside comments, and the fixture tests still pass.
- [ ] 4.6 Record the decision. Add `docs/adr/0032-mpv-runs-in-a-child-process.md` (decision, the probe findings as evidence, the S8 behavior, mpv 0.41 as the tested version). Add one entry to `docs/invariants/`: the child never outlives the Owner process and never serves two runs. Update the `AGENTS.md` sentence "It embeds mpv." and the README mentions of embedding, and add the new terms to `CONTEXT.md` if the change introduces one. Verify: `rg "embeds mpv|embedded libmpv" AGENTS.md README.md docs CONTEXT.md` has no stale match.
- [ ] 4.7 Run the gate and commit group 4.

## 5. Dependencies and packaging

- [ ] 5.1 Remove `libmpv2` and `libmpv2-sys` from `crates/mbv-player/Cargo.toml`. Keep the workspace entries that `mbv-core` dev-dependencies use. Verify: `cargo tree -p mbv-player | rg libmpv` shows nothing, and `cargo check --workspace --all-targets` passes.
- [ ] 5.2 Change the Debian `depends` lines in `Cargo.toml` and `crates/mbvd/Cargo.toml` from `libmpv2 | libmpv1` to `mpv`. Check `PKGBUILD` and any release workflow for a libmpv build or link step. Verify: `rg "libmpv" Cargo.toml crates/mbvd/Cargo.toml PKGBUILD .github` matches only the `mbv-core` dev-dependency.
- [ ] 5.3 Run the gate and commit group 5.

## Workflow follow-up

- Archive the change after review, and sync the delta spec into `openspec/specs/`.
- Issue #936 (the active-file start-failure stop) is independent of this change.
