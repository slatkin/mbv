# Design

## Context

- **Launch.** `main()` runs `pre_config_startup()` (CLI flags), then `applog::init`, then
  `load_config`, then `run_local_instance`. Every local launch is a Client: a fresh launch
  spawns the Owner process (`local_daemon::spawn_detached`, `mbv --__local-daemon`) and attaches
  over the local Unix ctrl socket (`attach_owner_process`, `DaemonEndpoint::Local`). With
  stay-alive off, the Owner serves one Client and exits with it.
- **Tray.** `mbv-daemon/src/run.rs::start_tray` calls the `on_tray_ready` hook once at startup,
  and only when `owner_settings().stay_alive`. The hook in `src/local_daemon.rs` returns `None`
  when `show_systray_icon` is off. Otherwise it takes the player handle and calls
  `mbv_desktop::tray::spawn`, which returns an opaque `Box<dyn Send>`. The tray (ksni) reads
  `Arc<Mutex<PlayerStatus>>` and sends `TransportCommand`s, and it is deliberately not a ctrl client.
- **Ctrl.** `CtrlHello::current()` lists capabilities, and the client copies the ones it uses
  into `CtrlCompatibility` (`mbv-remote-player/src/connect.rs`). The daemon's `ClientRegistry`
  (`mbv-daemon/src/ctrl.rs`) holds one `CtrlClient` per connection with its `transport`.
  `is_local_client` already gates `RequestShutdown`.
- **pinwin.** After slatkin/pinwin#1, pinwin takes `--no-tray`, exports `PINWIN_SOCKET`, and
  answers `options\n` with `ok\n` (see that change's `pinwin-control` spec).
- **Packaging and CI.** `PKGBUILD` repackages the release tarball, and `aur.yml` rewrites only
  `pkgver`/`sha256sums`. CI runs in a pinned `archlinux:base-devel` container; Arch `extra` has
  `zig 0.16.0`, `gtk4`, `gtk4-layer-shell` and `libdbusmenu-glib`.
  `scripts/check-code-file-lines.sh` governs `*.c`/`*.h` but not `*.zig`.

## Goals / Non-Goals

**Goals:**
- One launcher entry and one tray icon, with no GUI dependency in the `mbv` package.
- The tray's pin item reaches the right panel even with several pinned Clients (stay-alive).

**Non-Goals:**
- No mbv settings for the panel. Width, side and gutters stay in pinwin's options window and
  config.
- No new pinwin requests beyond `options`, and no pinwin code changes. Those belong to
  slatkin/pinwin#1.
- No tray in `mbvd`, and no pinned declaration toward `mbvd` or any TCP endpoint.
- No live tear-down of a tray once started (see D4).

## Decisions

**D1. Import is a copy at a named pinwin commit.**
- After slatkin/pinwin#1 is archived in pinwin, copy the tracked contents of `pinwin/pinwin/` into
  `pinwin/` here, and the three main specs (`pinwin-panel`, `pinwin-tray-options`,
  `pinwin-control`) into `openspec/specs/`.
- While copying, drop "Install from the checkout" (`make install` in the pinwin repo; replaced by
  this change's packaging requirement) and "Coexists with pinwin" (the script stays in the pinwin
  repo).
- The commit message names the source commit (`slatkin/pinwin@<sha>`). The specs are copied
  rather than authored as deltas because they are already-accepted behaviour moving repos, not a
  behaviour change.
- Alternative: `git subtree add`. Rejected, because pinwin's handful of commits would mix an
  unrelated root history into mbv's log.

**D2. `mbv --desktop` is resolved before config.**
- `pre_config_startup` recognises `--desktop`. `main` handles it right after `applog::init`, so
  the failure reason reaches `mbv.log`, and before `load_config`.
- A new small module `src/desktop_launch.rs` holds:
  - a pure `fn desktop_command(on_path: impl Fn(&str) -> bool) -> Option<[&str; N]>`, which
    returns `pinwin --no-tray mbv`, then `xdg-terminal-exec mbv`, then `None`;
  - the `PATH` scan (std only);
  - `CommandExt::exec`.
- `contrib/mbv.desktop` becomes `Exec=mbv --desktop`, `Terminal=false`.
- Alternatives considered:
  - `Exec=sh -c '...'`: rejected as bespoke scripting, and untestable.
  - Shipping the desktop entry from the `pinwin` package: rejected, because it creates two
    entries or a file conflict.
  - Keeping `Terminal=true` and re-execing into pinwin: rejected, because a terminal window would
    flash open.

**D3. The pinned declaration is an additive ctrl capability.**
- New capability constant `pinned-panel`, added to `CtrlHello::current()`, plus
  `supports_pinned_panel()`, copied into `CtrlCompatibility`.
- New `CtrlCmd::DeclarePinned { socket: PathBuf }` with no reply.
- Client: `attach_owner_process` (the only Local-endpoint attach) reads `PINWIN_SOCKET` through a
  pure `pinned_socket(Option<OsString>) -> Option<PathBuf>` (non-empty, absolute). If the result is
  `Some` and the Owner supports `pinned-panel`, it calls a new
  `RemotePlayer::declare_pinned(path)` once, right after connecting. `--connect-daemon` paths never
  call it.
- Daemon: `CtrlClient` gains `pinned: Option<(u64 /* declaration seq */, PathBuf)>`. The command
  is handled like `RequestShutdown`'s local-only gate: from a non-local client it is logged and
  ignored. `ClientRegistry::latest_pinned()` returns the path with the highest sequence among
  connected clients, and `remove` drops it with the client.
- Its `OwnerGate` follows the existing classification for Local-only commands.
- Alternative: the Owner reads `PINWIN_SOCKET` from its own environment. Rejected: the Owner may
  have been started outside the panel and may outlive it, so the environment is stale or wrong.

**D4. The tray starts lazily, at most once, and receives its pin target by push.**
- `start_tray` keeps the hook instead of dropping it when stay-alive is off. The first accepted
  `DeclarePinned` invokes it. Since a stay-alive-off Owner lives exactly as long as its single
  Client, a tray started this way never needs tearing down, so no live removal path exists.
- The hook's return type changes from `Option<Box<dyn Send>>` to `Option<Box<dyn TrayPort>>`, a
  small trait in `mbv-daemon`:

  ```rust
  pub trait TrayPort: Send {
      fn set_pin_target(&self, socket: Option<PathBuf>);
  }
  ```

  `mbv-desktop` implements it with ksni's `Handle::update`, writing an owned
  `pin_target: Option<PathBuf>` field on `MbvTray`, which also refreshes the menu.
- The daemon calls `set_pin_target(registry.latest_pinned())` after every accepted declaration
  and every client removal, and right after a lazy start.
- The tray never reads the registry and stays off ctrl.
- `show_systray_icon = false` still wins: the hook returns `None`, and pinning does not force an
  icon.
- Alternative: a shared `Arc<Mutex<Option<PathBuf>>>` read by `menu()`. Rejected, in line with the
  repo's preference for owned data over new shared state; it also wouldn't refresh the menu on
  change.

**D5. The tray item does blocking I/O on the tray thread, with tight timeouts.**
- `Pin options...` connects a `UnixStream` to the target, sets 1-second read and write timeouts,
  writes `options\n` and reads one line.
- Anything but `ok` is logged under target `tray` and otherwise ignored.
- This runs on ksni's own thread, not on the daemon loop.

**D6. Packaging and CI.**
- `PKGBUILD` becomes `pkgbase=mbv`, `pkgname=(mbv pinwin)`.
  - `package_mbv` keeps today's contents and adds
    `optdepends=('pinwin: open pinned from the launcher' 'xdg-terminal-exec: open in a terminal from the launcher')`.
  - `package_pinwin` installs `/usr/bin/pinwin` and the license, with
    `depends=(gtk4 gtk4-layer-shell libdbusmenu-glib pango)`.
- `PKGBUILD-git` does the same, adds `zig gtk4 gtk4-layer-shell libdbusmenu-glib` to
  `makedepends`, and builds with `zig build -Doptimize=ReleaseSafe` in `pinwin/`.
- `build.yml` installs those packages from the pinned image, runs `zig build
  -Doptimize=ReleaseSafe` and `zig build check` in `pinwin/`, and copies `pinwin` into the
  tarball.
- `aur.yml` is unchanged, because it only rewrites version and checksum. The `.deb` is unchanged.
- `scripts/check-code-file-lines.sh` adds `*.zig` to the governed extensions, and its self-test
  gets the matching case.

## Risks / Trade-offs

- **Launcher on GNOME Wayland (no layer-shell) with pinwin installed:** pinwin exits and nothing
  opens. Accepted, since mbv's launcher targets layer-shell compositors and this is the Wayland-only
  decision. It is written down in the README.
- **A stale pin target if a ctrl connection dies without a clean close:** the existing registry
  removal on read error already covers it. D5's timeouts bound a dead socket.
- **The release CI fetches libghostty-vt from the network:** the commit is pinned by hash in
  `build.zig.zon`. A disappeared commit is fixed by re-pinning.
- **Two Rust crates gain small surface (`mbv-ctrl` command plus capability, `mbv-daemon` trait):**
  the change is additive, with no protocol version bump.

## Migration Plan

1. Land slatkin/pinwin#1 in pinwin and archive it.
2. Import (D1) together with packaging and CI (D6). At this point the launcher is unchanged.
3. Ctrl, daemon and tray (D3–D5), then the launcher (D2) last, so the desktop entry only switches
   once pinning works end to end.
4. Rollback: revert the launcher commit to restore `Terminal=true`. The other parts are inert
   without it.
