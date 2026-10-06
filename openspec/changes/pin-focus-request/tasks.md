# Tasks

## 1. Pin pinwin 0.1.0

- [ ] 1.1 In `Cargo.toml:119`, change the pinwin dependency to `tag = "0.1.0"` and run `cargo update -p pinwin` (design D1). Verify that the `Cargo.lock` pinwin source ends in `?tag=0.1.0#236774bbeb4aee6a992b367415b27544acdc8b15` and that `cargo check -p mbv` passes with no mbv source change. Commit.

## 2. Lock, socket, and client module

- [ ] 2.1 Create `src/pin/focus.rs` and add `mod focus;` to `src/pin.rs` (design D7). Add `PinClaim` with `acquire_at(dir: &Path)` and the public `acquire()` that uses `$XDG_RUNTIME_DIR` with the same `/tmp` fallback as `single_instance::runtime_dir` (design D2, D3, D4). `acquire_at` takes a non-blocking exclusive `flock` on `mbv-pin.lock`. On `EWOULDBLOCK` it returns `Busy`. Otherwise it removes any old `mbv-pin.sock`, binds a non-blocking `UnixListener` there with mode `0600`, and returns `Claimed`. Verify with `cargo check -p mbv`.
- [ ] 2.2 Add the server answer `answer(stream, focus: impl FnOnce() -> Result<(), PinwinError>)` with a 100 ms read and write timeout. It answers `ok\n` only for a `focus\n` line whose closure returns `Ok(())`, and `error\n` otherwise. It logs a failed focus result. Add `PinFocusError` with the variants `NotRunning`, `Refused`, and `NoAnswer(io::Error)`. Implement `Display` and `std::error::Error` for it. Add `request_focus_at(socket: &Path)` and the public `request_focus_from_running()`, with a 6 s read timeout (design D4, D6). Use no `anyhow`, `thiserror`, or `eyre`. Verify with `cargo check -p mbv`.
- [ ] 2.3 Add the two contract tests in `src/pin/focus/tests.rs` (design D8). Each uses a unique temp directory, as in the `src/single_instance.rs` tests. The first test covers the exclusive claim: `Busy` while a claim lives, and `Claimed` after it drops. The second is a `#[case]` table of client results: `Ok`, `Refused`, and `NotRunning` for an empty directory. Use no sleeps. Verify that `cargo nextest run -p mbv pin::focus` passes, then commit.

## 3. Panel ownership and serving

- [ ] 3.1 In `src/pin.rs`, change `PinnedPanel` from a type alias to a struct (design D3). Its private fields are `panel: pinwin::Panel` and then `claim: focus::PinClaim`, in that order. `pin::start(config, claim)` takes the claim by value and drops it on every error path before it returns. Keep `pin::apply_layout`'s signature and reach the panel through the struct. Add `PinnedPanel::serve_focus_requests(&self) -> bool`. It accepts until `WouldBlock` and calls `focus::answer` with `|| self.panel.request_focus()` for each connection. It always returns `false` (design D5). Verify that `cargo check -p mbv` shows only the expected `main.rs` call-site error for `pin::start`.
- [ ] 3.2 Add `App::drain_pin_focus_requests(&self) -> bool`. If `pinned_panel` is `None`, it returns `false`. Otherwise it calls `serve_focus_requests`. Call it in `drain_iteration_work` (`src/app/shell/run.rs`) next to the other `self.app.drain_*` calls, as `*had_events |= ...` (design D5). Verify with `cargo check -p mbv` after task 4.1.

## 4. Start-up routing and documentation

- [ ] 4.1 In `src/main.rs`, add `focus_requested` to `StartupArgs`, parsed with `has_flag(&args, "--focus")` in `pre_config_startup` (design D6). In `main`, right after `applog::init` and before `migrate_legacy_emby_token`, add the routing. `--focus` runs `run_focus_request()` and exits with its status, and it wins over `--pin`. `--pin` calls `PinClaim::acquire()`. `Busy` runs `run_focus_request()` and exits with its status. `Claimed` keeps the claim for the existing `pin::start` call. An I/O error goes to `pin::report_start_failure` as a new `PinStartError::Lock(io::Error)` variant, and the launch continues as a failed pinned start. Put `run_focus_request()` in `src/pin.rs` and reuse `failure_action` and `notify` for the report. Verify that `cargo check -p mbv` passes.
- [ ] 4.2 Add `--focus` to `print_usage` in `src/main.rs`: "Give keyboard focus to the running pinned panel. Bind it to a compositor hotkey." Add a short README section, "Pinned panel focus hotkey". It says that a second `mbv --pin` focuses the running panel, and it gives a niri example, `Mod+P { spawn "mbv" "--focus"; }`. In `CONTEXT.md`, change the `--pin` entry to say "at most one pinned launch per user". It also says that a second `--pin` focuses the running panel. Add a `--focus` entry. Verify the text by reading it, then commit.

## 5. Integration checks

- [ ] 5.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`. Verify that all three pass with no lint suppression added. Then commit.
- [ ] 5.2 Ask the user for a live test. Name these exact commands: `mbv --pin`, a second `mbv --pin`, `mbv --focus` with the panel running and without it, and `mbv --focus` from a niri hotkey. Do not run them without a yes. If the user reports that the spec delta scenarios hold, the task is done.
