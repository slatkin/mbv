# Design

## Context

- `src/pin.rs` is mbv's only pinwin boundary (see `AGENTS.md`). The flow today is `pin::start`:
  check `WAYLAND_DISPLAY`, apply `PanelEnv`, open the pty, `Panel::start(Startup { .. })` with a
  struct literal, then detach and hand over stdio. `main` calls it after `load_config`. Every
  `Err` goes to `report_start_failure`, which falls back to the terminal or notifies and exits 1.
- The pinned panel lives in the pinned TUI Client process, not the Owner. `mbv.lock` and
  `single_instance` are about the Owner and are not involved here.
- pinwin 0.3.0 provides the following:
  - `instance::InstanceSocket::bind(&InstanceName) -> Result<_, InstanceError>`, where the error
    is `Duplicate`, `Path(PathError)` or `Io`.
  - `Startup::new(fd, layout, keyboard, accent).with_instance(socket)`.
  - `Panel::toggle` and `Panel::show`.
  - `instance::send(&InstanceName, Request::{Toggle, Show}) -> Result<(), SendError>`. `SendError`
    is non-exhaustive: `Environment`, `NotListening`, `Refused` or `NoAnswer`.
- A panel started with a socket serves `toggle` and `show` on a detached thread until the `Panel`
  drops. The drop removes the socket file.
- In pinwin, a hide releases the reserved strip unless the layout is covering. An apply while
  hidden is stored for the next show. Neither hide nor show resizes the grid or the pty.

## Goals / Non-Goals

**Goals:**
- One pinned panel per display.
- A second `--pin` shows the running panel.
- `mbv --toggle` can be bound to a compositor key.
- All of this uses only pinwin's socket.

**Non-Goals:**
- mbv-side IPC, PID files, signals or tracking of the shown state.
- An in-TUI keybind that hides the panel.
- A "toggle launches mbv when none runs" mode. `--pin` is the only launcher (decision in #883).
- New `[panel]` settings.

## Decisions

### D1: One fixed instance name, `mbv`

`pin.rs` holds the name, `InstanceName::parse("mbv")`, in one private helper. The `expect` in
that helper is on a literal that is known to be valid. pinwin already scopes the socket path by
`WAYLAND_DISPLAY`, so the limit of one per display needs nothing more from mbv. A configurable
name was rejected because nothing asks for one.

### D2: Bind first, inside `pin::start`, before `PanelEnv::apply`

The order inside `pin::start` becomes:
1. The `WAYLAND_DISPLAY` check (unchanged).
2. `InstanceSocket::bind(name)`.
3. On `Err(Duplicate)`, `instance::send(name, Request::Show)` and return the new outcome.
4. On `Err(other)`, return `PinStartError::Instance(InstanceError)`.
5. On `Ok(socket)`, run `PanelEnv::apply`, then `start_panel(config, socket)`. That builds
   `Startup::new(..).with_instance(socket)`.

A duplicate leaves the environment, the pty and the terminal untouched.

`pin::start` returns `Result<PinLaunch, PinStartError>`, where:

```rust
pub(crate) enum PinLaunch {
    /// This process owns the panel.
    Started(PinnedPanel),
    /// A pinned mbv already runs on this display and was asked to show.
    ShownExisting,
}
```

`main` maps `Started` to `Some(panel)` as today. `ShownExisting` returns from `main`, so the
process exits 0 before `run_configured_startup`, with no Owner attach and no TUI. A failed `Show`
becomes `PinStartError::ShowExisting(SendError)`. It takes the existing `report_start_failure`
path: a terminal fallback when stdin is a TTY, otherwise a notification and exit 1. That reuses
the existing contract and adds no new failure policy.

Binding in `pre_config_startup` was rejected. The socket would have to travel in `StartupArgs`
past applog init and config loading. It would also split pin start-up across two sites, when
`pin.rs` is meant to be the only boundary.

### D3: `--toggle` is parsed with the early flags, beside `-q`

`pre_config_startup` handles `--toggle` like `-q`: before applog, config migration and
`load_config`. It calls `pin::toggle_running()` and then `return None`, and that function exits
non-zero on failure. `pin::toggle_running()` runs `instance::send(name, Request::Toggle)` and
reports as follows:
- `Ok`: exit 0 with no output.
- `Err(SendError::NotListening { .. })`: reports "mbv: no pinned mbv is running".
- Any other `Err(error)`: reports "mbv: cannot toggle the pinned panel: {error}". This is a plain
  `Err(error)` binding, not a wildcard over variants. `SendError` is pinwin's non-exhaustive
  error, so mbv uses its `Display` and doesn't re-classify it.

Both reports go through one helper. The helper prints to stderr when stdin is a terminal and
otherwise calls `notify`, because a compositor key has no terminal. It then exits 1. The helper
reuses `failure_action`'s TTY split rather than adding a second one.

`--toggle` does not check `WAYLAND_DISPLAY` itself. pinwin's socket path falls back to
`wayland-0`, and a missing `XDG_RUNTIME_DIR` comes back as `SendError::Environment`, which is
reported like any other error.

### D4: Pin by `rev`

`Cargo.toml` uses `rev = "8127ff81079d91f326dc5b8427ab03a1dc17992c"` (tag `0.3.0`), as `AGENTS.md`
prescribes, in place of the current `tag = "0.2.2"`.

### D5: Packaging follows pinwin's runtime libraries

pinwin 0.3.0 links no GTK. Its README says the system needs libwayland, libxkbcommon and
fontconfig. The Wayland client is the pure-Rust backend, so the dependency set is settled by
`ldd target/release/mbv` after the bump (task 3.1). It is not guessed. The expected changes are:
- Debian `depends`: drop `libgtk-4-1, libgtk4-layer-shell0` and add `libxkbcommon0,
  libfontconfig1`, plus `libwayland-client0` only if `ldd` shows it.
- `PKGBUILD` and `PKGBUILD-git`: swap `gtk4 gtk4-layer-shell` for `libxkbcommon fontconfig`, in
  both depends and makedepends.
- `build.yml`: change the same pacman packages. The two deb `Depends` greps switch to the new
  package names, which keeps the existing check's shape.

## Risks / Trade-offs

- **The `Panel` is not dropped on a `process::exit` path, so the socket file stays behind.**
  pinwin's bind probes the file. A dead owner refuses the connect, so the next `--pin` unlinks
  the file and rebinds. `--toggle` sees `NotListening` and reports that nothing is running.
- **The Owner process inherits the listener fd and keeps the name "live" after the Client dies.**
  std's `UnixListener` sets `SOCK_CLOEXEC`, and the Owner is spawned by exec, so the fd doesn't
  survive. No mbv code is needed.
- **Focus on show depends on the compositor.** mbv keeps `Keyboard::OnDemand`. niri focuses a
  newly mapped on-demand surface (pinwin README). Other compositors may only show the panel. The
  live test checks this on niri.
- **BREAKING: a second `--pin` no longer opens a second panel.** One panel per display is the
  decision made in #883, so this is intended.

## Migration Plan

Users who bound a compositor key to a pinwin focus or launch command rebind it to `mbv --toggle`.
Nothing persisted changes. To roll back, revert the commit.
