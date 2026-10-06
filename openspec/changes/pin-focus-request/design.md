# Design

## Context

See proposal.md for the motivation. The current state:

- `pin::start` (`src/pin.rs`) starts the panel in `Keyboard::OnDemand`, so `request_focus()` always does real work for mbv. `main` calls it after `load_config` and passes `Option<PinnedPanel>` down to `App.pinned_panel`.
- `PinnedPanel` is a type alias for `pinwin::Panel`. `Panel` is not `Clone`. Its methods are safe to call from any thread, and each one blocks for at most pinwin's `APPLY_WAIT` (5 s).
- `src/single_instance.rs` already holds the Owner lock (`$XDG_RUNTIME_DIR/mbv.lock`) with a non-blocking `flock` (ADR 0006). The kernel releases it on any process death.
- The pinwin socket code (`ipc.rs`) lives in the pinwin binary, not in the library. mbv cannot reuse it.
- The shell loop runs `drain_iteration_work` (`src/app/shell/run.rs`) once per pass. A pass waits at most 50 ms for terminal input, or 8 ms with the visualizer.

## Goals / Non-Goals

**Goals:**

- Make "at most one pinned launch per user" a rule that the type system and the kernel enforce, not a convention.
- Serve the focus request with std Unix sockets only, with no new dependency and no new thread.

**Non-Goals:**

- Instance names (pinwin's `--focus [name]`). One pinned launch per user means one target.
- Other socket commands. The protocol has one request.
- A runtime change of the keyboard mode.

## Decisions

### D1. Pin pinwin to tag `0.1.0`

Change `Cargo.toml:119` from `tag = "0.2.2"` to `tag = "0.1.0"` and refresh `Cargo.lock` with `cargo update -p pinwin`. Tag `0.1.0` is commit `236774b`. Between the current lock commit `b910591` and `236774b`, the pinwin public API only grows, so this step changes no mbv call site. The lower version number does not matter for a git dependency. A `rev =` pin was rejected because a tag is what the user asked for.

### D2. The lock is a per-user `flock`, separate from the Owner lock

The pin lock is `$XDG_RUNTIME_DIR/mbv-pin.lock`. It sits next to `mbv.lock` and uses the same `runtime_dir()` fallback. A non-blocking exclusive `flock` gives an answer with no race. The kernel releases the lock when the holder dies, so there is no stale case.

Alternatives that were rejected:

- The socket bind as the lock, as in pinwin: if two launches start together, the steps "connect, remove the stale file, bind" race. Both can see a refused connect, and the second remove deletes the first one's live socket.
- One lock per Wayland display, as in pinwin's socket path: the user requires that two pinned launches cannot run at once, so the lock is per user.
- Reuse of the Owner lock: the Owner lock belongs to the Player owner, and a pinned TUI is a Client. The two lifetimes differ.

The pin lock copies the pattern in `single_instance::resolve`: std `OpenOptions` opens the file, and `nix::fcntl::Flock` locks it. That file and the std `UnixListener` are close-on-exec, so the Owner process that a pinned launch spawns (`local_daemon::spawn_detached`) does not inherit the lock. If the Owner inherited it, a stay-alive Owner would hold the lock after the pinned TUI exits, and every later `--pin` would fail. `single_instance::runtime_dir` becomes `pub(crate)`, and the pin paths use it.

### D3. A panel without the lock cannot exist

A new type, `PinClaim`, holds the locked file and the bound, non-blocking `UnixListener`. `PinClaim::acquire()` returns `PinAcquire::Claimed(PinClaim)` or `PinAcquire::Busy`. Other I/O failures return an error.

`pin::start` takes the `PinClaim` by value. `PinnedPanel` changes from a type alias to a struct with private fields `panel: pinwin::Panel` and `claim: PinClaim`, in that order. Rust drops fields in declaration order, so the panel closes before the lock is released. The code cannot build a `PinnedPanel` without a claim, so the one-panel rule needs no comment to enforce it.

If `pin::start` fails, the claim drops inside it, so the lock is released before the terminal fallback runs. This is the "Pin flag" rule in the spec delta. The `PinStartError` stays as it is.

`pin::apply_layout` keeps its signature and reaches the panel through the struct. A new method `PinnedPanel::serve_focus_requests(&self) -> bool` serves pending requests (D5).

### D4. The socket

The socket path is `$XDG_RUNTIME_DIR/mbv-pin.sock`. `PinClaim::acquire` binds it right after it takes the lock. While the lock is held, no other process can own the path, so acquire removes any old file at that path first and then binds. The socket file mode is `0600`.

The protocol is pinwin's. The client sends `focus\n`. If `request_focus()` returns `Ok(())`, the server answers `ok\n`. For any other request or result, it answers `error\n`. The server logs a failed `request_focus()` with the `PinwinError`. mbv does not remove the socket file at exit. A file with no listener gives a refused connect, which the client reports as "no pinned mbv is running".

### D5. Serve requests in the shell loop, with no thread

`drain_iteration_work` gets one more drain, `self.app.drain_pin_focus_requests()`. If `pinned_panel` is `None`, it does nothing. Otherwise it calls `serve_focus_requests`, which accepts connections until `WouldBlock`. For each connection it sets a 100 ms read and write timeout, reads one line, answers, and closes. A focus request reaches the panel within one loop pass, which is at most 50 ms.

A listener thread was rejected. It needs `Arc<Panel>` or a new channel into the shell, and AGENTS.md prefers synchronous code and owned data. The loop already polls on a short period, so a thread gives no faster response.

The shell does not set a redraw for a focus request. Focus does not change what the TUI draws, so the drain returns `false`.

### D6. The client and the second `--pin` share one path

`pin::request_focus_from_running() -> Result<(), PinFocusError>` connects to the socket, writes `focus\n`, and reads one line with a 6 s read timeout. That is one second more than pinwin's 5 s `APPLY_WAIT`, so a slow but healthy GTK side still answers in time.

`PinFocusError` is a domain error enum with a `Display` text for each case:

- `NotRunning`: the connect failed with `ENOENT` or `ECONNREFUSED`. Text: "no pinned mbv is running".
- `Refused`: the server answered `error\n`. Text: "the pinned panel did not take focus".
- `NoAnswer(io::Error)`: the connect, write, or read failed in another way, or timed out. Text: "the pinned mbv did not answer: {error}".

`main` handles both entry points right after `applog::init`, before `migrate_legacy_emby_token` and `load_config`. A focus request then has no config side effects.

- `--focus` calls `run_focus_request()` and exits with its status.
- `--pin` calls `PinClaim::acquire()`. `Busy` calls `run_focus_request()` and exits with its status. `Claimed(claim)` keeps the claim and passes it to `pin::start` at the existing call site. An `acquire` I/O error goes to `pin::report_start_failure` as a new `PinStartError::Lock(io::Error)` variant, with the same terminal or notification path as any other start failure.

`run_focus_request()` maps `Ok` to status 0. It reports an `Err` with a log line, then the D5 rule of the original pin design: if stdin is a terminal, `eprintln!`, otherwise `notify`. It then returns status 1. The rule reuses `notify` and the stdin terminal check in `src/pin.rs`. It does not reuse `StartFailureAction`, because a focus failure exits with status 1 on both branches.

`--focus` is a runtime flag like `--pin`. It is parsed in `pre_config_startup` as `StartupArgs.focus_requested`. When both `--focus` and `--pin` are given, `--focus` wins, because it never starts a panel.

### D7. Module layout

The lock, socket, server answer, and client go in a new child module `src/pin/focus.rs`, with `pub(crate) mod focus;` in `src/pin.rs`, so `main` can name `PinClaim` and `PinAcquire`. `src/pin.rs` is 454 lines today and keeps the start path. The new module holds no start-up code.

### D8. Tests

There are two contracts and three tests in `src/pin/focus.rs`. The tests use a unique temp directory, as in `src/single_instance.rs` tests, and do not use a live panel. To allow this, `PinClaim::acquire_at(dir)` and `request_focus_at(socket)` take paths, and the public wrappers pass the runtime directory paths.

- Exclusive claim: a second `acquire_at` on the same directory returns `Busy` while the first claim lives, and `Claimed` after the first claim drops. This test guards the one-pinned-launch rule.
- Answer mapping: the server answer function takes the focus result as a closure parameter, `answer(stream, focus: impl FnOnce() -> Result<(), PinwinError>)`. A two-case `#[case]` table runs `answer` on a spawned thread against a bound listener, with `Ok(())`, which gives `Ok`, and with `Err(PinwinError::NotRunning)`, which gives `Refused`.
- No listener: `request_focus_at` on an empty directory gives `NotRunning`. This is a separate test, because it needs no server and a shared table would need a branch.

No test covers the shell drain, `main` flag routing, or the help text. These parts are wiring, and the user covers them with live testing.

## Risks / Trade-offs

- [A wedged GTK side blocks the TUI loop for up to 5 s per focus request.] → pinwin bounds the wait and reports `Internal`. A wedged panel already means a broken pinned launch, so the user has a bigger problem than a frozen frame.
- [A local process can hold a connection open without sending data.] → The 100 ms read timeout bounds the stall for each connection. The socket mode is `0600`, so only the same user can connect.
- [A focus request can arrive before the pinned launch serves it.] → The socket binds right after the lock, but the first loop pass runs only after `load_config`, `pin::start`, Owner spawn or attach (up to 10 s in `wait_for_owner_exit`), and App construction. A second launch in that window connects, waits the full 6 s, gets `NoAnswer`, reports "the pinned mbv did not answer", and exits with status 1. It never opens a second panel. The first loop pass still serves the queued request, so the panel takes focus a moment later. This is accepted.
- [A second `mbv --pin` over ssh focuses the panel on the desktop.] → This is accepted. The lock is per user, and the request is harmless.

## Migration Plan

There is no data migration. Users who bound a hotkey to `pinwin --focus` for mbv change it to `mbv --focus`. A pinned launch that runs an older mbv does not take the lock, so a new `mbv --pin` started beside it opens a second panel. A restart of the older pinned launch ends this.
