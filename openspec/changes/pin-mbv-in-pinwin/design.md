# Design

## Context

- **Launch.** `main()` runs `pre_config_startup()` (`-h`, `-V`, `-q` and `--__local-daemon` exit
  or divert there), `applog::init`, `load_config`, then `run_configured_startup`: an explicit
  daemon endpoint (`--connect-daemon` or config `daemon_client_endpoint`) runs a remote-client
  TUI (`run_remote_app`); otherwise `run_local_instance` makes this launch a Client, spawning the
  Owner process when needed (`local_daemon::spawn_detached`, which `setsid`s and nulls/pipes the
  child's stdio). The tray lives in the Owner (`mbv-desktop`); this change adds a
  `Panel options...` item to it for a pinned client (D9).
- **Terminal access.** mbv and ratatui-image never open `/dev/tty`. crossterm 0.29 reads input and
  sets raw mode on stdin when stdin is a tty (`tty_fd()`), but `window_size()` and the kitty
  keyboard probe open `/dev/tty` first and fall back to stdout only when that open fails. Resize
  events come from `SIGWINCH`, which the kernel sends only for a controlling terminal.
- **Config and F2.** Settings are fields on `Config` (`mbv-config`), parsed in `parse.rs`, saved
  in `save.rs`, listed as `SettingKey`s (`mbv-ui-model/src/settings.rs`) and handled in
  `src/app/dispatch/settings.rs`. Row kinds today are `Key`, `Boolean`, `Text` (cycled through a
  fixed set) and `Collection`; there is no numeric-entry kind. `mbv-config` is a dependency of
  `mbv-daemon` and `mbvd`.
- **Packaging.** CI runs one `cargo build --release`; the tarball and the mbv `.deb`
  (`cargo deb --no-build`) both package that `target/release/mbv`. The `.deb` ships
  `contrib/mbv.desktop` (root `Cargo.toml` deb assets).
- **pinwin today** (imported at `slatkin/pinwin@eaefd7f`, the program form; the library form is
  developed upstream in change `add-library-abi`): a Zig `main()` over libghostty-vt plus C glue
  on GTK4, gtk4-layer-shell, pango and gio, with a control socket, own tray, GTK options window,
  GKeyFile config and `COLS`/`GUTTER` env vars. Its pure layout core (`options.c`) has no GTK
  dependency.
- **Crate graph.** `mbvd` depends only on `mbv-core`, `mbv-daemon` and `mbv-config`;
  `mbv-desktop` is a leaf pulled in by the root `mbv` crate only.

## Goals / Non-Goals

**Goals:**
- Pinning is a feature of mbv: `mbv --pin` and the desktop entry; no separate program, package,
  launcher or config for the user.
- Plain `mbv` behaves exactly as today in every terminal, over ssh and in tmux.
- GTK never enters `mbv-core`, `mbv-daemon` or `mbvd`; the TUI binary gets it.
- pinwin is first-party code, so surfaces that existed only because it was a separate program are
  removed, not bridged.

**Non-Goals:**
- `mbvd` is unaffected; the Owner gains only the pinned-client tray item and its ctrl plumbing (D9).
- No pinwin command line, environment variables, control socket, own tray or own GTK options
  window: the panel's options UI is the F2 Panel page, reached from the Owner's tray (D9).
- No panel, and no working desktop launcher, on sessions without layer-shell (GNOME, X11).
- No config setting that turns pinning on.

## Decisions

**D1. The pinwin specs are re-imported, not authored here.**
- The library reshape and its spec edits are specified in `slatkin/pinwin` change `add-library-abi`
  (its own planning review round). When it lands, task 3.1 re-imports the rewritten `pinwin-panel`
  over `openspec/specs/pinwin-panel/` and deletes `openspec/specs/pinwin-tray-options/` and
  `pinwin-control/`, which upstream retires. This change never edits pinwin specs directly; the
  copies in `openspec/specs/` track the imported revision.

**D2. pinwin is a static library with a C ABI, owned upstream.**
- The reshape is designed and implemented in `slatkin/pinwin` change `add-library-abi`. This
  change depends on its contract:
  - `pinwin_start(const PinwinStartup*) -> int`: pty master fd, a full `PinwinLayout` (side, cols,
    four gutters), keyboard mode. Pure argument checks, then a synchronous start-up handshake with
    the GTK thread; `PINWIN_ERR_NO_DISPLAY` when GTK or layer-shell init fails.
  - `pinwin_apply_layout(const PinwinLayout*) -> int`: pure checks on the caller thread, then
    validation against live monitor metrics on the GTK thread; synchronous result code. Never
    called from the GTK thread.
  - `pinwin_stop(void)`: closes the panel and joins the thread. Hangup on the master only drops
    pinwin's pty source; it does not close the panel.
  - The library never calls `exit()`.
  - Build output: the consumer links `zig-out/lib/libpinwin.a` **and** `zig-out/lib/libghostty-vt.a`
    (upstream D7: the archives are not merged) against GTK4, gtk4-layer-shell and pango/cairo.
- **Required upstream additions** (not in `add-library-abi` yet; they must land there before the
  `library-abi` tag):
  1. After every `TIOCSWINSZ` on the master, pinwin raises `SIGWINCH` in its own process (D3).
  2. `pinwin_start` validates the startup layout against live monitor metrics during its
     handshake and returns `PINWIN_ERR_INVALID` when it does not fit (no row, no output width),
     instead of showing a degenerate panel.
  3. A `pinwin_start` that returns an error leaves no pinwin thread running.
- mbv treats any non-`PINWIN_OK` from `pinwin_start` as a start failure (D5).

**D3. One process: pty pair, stdio hand-over, no controlling-terminal takeover.**
- With `--pin` (D5), before any terminal setup mbv:
  1. opens a pty pair;
  2. calls `pinwin_start` on the master;
  3. on success, if the process has a controlling terminal, detaches from it with `TIOCNOTTY`
     (ignoring `SIGHUP` across the call: when mbv is the session leader, as when a terminal
     emulator runs it directly, `TIOCNOTTY` hangs up its own foreground group);
  4. `dup2`s the slave onto fds 0 and 1 only. fd 2 is redirected to the application log (the
     same `mbv.log` the app logger appends to), falling back to `/dev/null`: after the hand-over,
     native library output to stderr (GTK/GLib/layer-shell/Vulkan warnings; manual check 6.2 saw
     a GDK `vkAcquireNextImageKHR` `VK_ERROR_OUT_OF_DATE_KHR` message drawn over the TUI) must
     not draw over the panel. Fatal post-hand-over errors go through the log and notification
     (D5) instead.
- **Terminal environment.** The TUI inherits the launching terminal's environment, and
  ratatui-image reads `TERM`/`TERM_PROGRAM` to detect tmux and the iTerm2 protocol, so a `--pin`
  from inside tmux or WezTerm would pick the wrong image path on the panel. mbv sets
  `TERM=xterm-256color` and `COLORTERM=truecolor` and removes `TERM_PROGRAM` and
  `TERM_PROGRAM_VERSION` after the `WAYLAND_DISPLAY` check and before `pinwin_start`, and restores
  the saved values on any start failure. Both points run while no thread that reads the
  environment exists (the GTK thread is not started yet, or has exited per upstream addition 3);
  the `unsafe` `set_var`/`remove_var` calls carry that as their `SAFETY` comment.
- After this `/dev/tty` cannot be opened, so crossterm's size query and keyboard probe fall back
  to stdout, which is the slave; input and raw mode already use stdin. pinwin raising `SIGWINCH`
  after each resize (D2) delivers crossterm's resize events.
- No `setsid`/`TIOCSCTTY`: `setsid` fails with `EPERM` for a process-group leader, which is how a
  shell or terminal emulator starts mbv, and taking a new controlling terminal would need a fork.
- A hand-over failure after `pinwin_start` succeeded calls `pinwin_stop` and is handled as a start
  failure (D5).
- mbv exits when the TUI exits; process exit ends the GTK thread and removes the panel. Exit
  paths that return normally call `pinwin_stop` first.
- Typed in a terminal, `mbv --pin` leaves that terminal waiting until mbv exits. Ctrl-C there or
  closing that terminal still signals mbv's process group; that ends mbv as it would any
  foreground program. Accepted.
- Alternative: fork, with the child taking the slave as its controlling terminal. Kept as the
  fallback if the task 1.1 probe shows the no-takeover route fails (a dependency that needs a real
  controlling terminal); it would change only what the launching shell does (it would return at
  once).
- Alternative: re-exec mbv as the panel's pty child. Rejected: live layout changes from F2 would
  need an IPC channel back to the panel, which is the socket this design removes.
- Alternative: pinwin as a separate executable. Rejected by the product definition and by D4.
- **Probe result (task 1.1, 2026-10-02).** The route above works; the fork alternative is not
  needed. Probed from foot, from ghostty, and from a no-controlling-terminal `setsid` launch
  (both pty stdio and `/dev/null` stdio): `/dev/tty` open fails `ENXIO` after `TIOCNOTTY`;
  crossterm 0.29's `window_size` falls back to stdout, the slave, and returns the slave's size
  (ctty 82x69, slave 80x24, got 80x24); an in-process `raise(SIGWINCH)` after `TIOCSWINSZ`
  delivered `Event::Resize` in every run; and with the real ghostty-vt on the master, DA1, the
  kitty keyboard query (`supports_keyboard_enhancement` true), the mouse modes (DECRPM 1006/1002
  set) and the kitty graphics query all round-trip through the slave. `local_daemon::spawn_detached`
  (null stdio, `setsid`) and libmpv2 (`Mpv` defaults equal `--no-terminal`; mbv sets no terminal
  option) do not touch the launching terminal, and mbv never opens `/dev/tty` itself. Not probed,
  left to manual check 6.2: real keypress, click and drag-resize into the visible panel.

**D4. One build, the crate graph and packaging.**
- A new leaf crate `crates/mbv-pinwin` owns `build.rs` (runs `zig build` for the vendored
  `pinwin/`, links both archives from D2 plus GTK4, gtk4-layer-shell and pango/cairo) and the safe
  Rust wrapper over the C ABI. The root `mbv` crate depends on it unconditionally.
- `mbv-core`, `mbv-daemon`, `mbv-config`, `mbvd` and every other crate MUST NOT depend on
  `mbv-pinwin`, directly or transitively.
- There is one build: one `cargo build --release`, one tarball binary and one `.deb`, all carrying
  pinning. `--pin` is a runtime flag. The F2 Panel page is always present.
- The mbv `.deb`, tarball and PKGBUILDs carry the GTK4 / gtk4-layer-shell runtime dependency
  (Debian: `libgtk-4-1`, `libgtk4-layer-shell0`; Arch: `gtk4`, `gtk4-layer-shell`), and building
  needs Zig plus the GTK4 / gtk4-layer-shell development packages. The CI `.deb` assertion that
  its `Depends` name no GTK package is replaced by one that it names them (section 7.2).

**D5. When the panel is used, and what happens when it cannot start.**
- Only `--pin` asks for the panel. It combines with `--log-level` and with remote-client launches
  (`--connect-daemon`, config `daemon_client_endpoint`): pinning decides where the TUI draws, not
  who owns the Player. `-h`, `-V`, `-q` and `--__local-daemon` exit or divert before `--pin` is
  considered.
- The decision is made after `load_config` and before any terminal setup, including the
  remote-client "Connecting to daemon" line.
- Start failure (`WAYLAND_DISPLAY` unset, any non-`PINWIN_OK` from `pinwin_start`, or a D3
  hand-over failure) is logged with the reason, then:
  - stdin is a tty: print one line `mbv: cannot open the pinned panel: <reason>; running in this
    terminal` to stderr and continue in the terminal;
  - otherwise (desktop launcher, keybind): send a desktop notification with the same text
    (`notify-send`, as the TUI's system notifications do; a missing `notify-send` is only logged)
    and exit with status 1.
- Fatal start-up exits after the hand-over (single-instance failure, Owner start failure, daemon
  connect failure) would print to the panel and vanish with it, so they go through the same
  report as a start failure: logged, and sent as a notification. Messages printed after the
  hand-over that are not fatal (for example "Connecting to daemon…") appear on the panel.
- Each pinned launch opens its own panel; a second `mbv --pin` while one runs opens a second
  Client in a second panel, as a second terminal window does today. Intended.
- `should_pin` is a pure function over the parsed flag; the environment and
  `pinwin_start` result enter as the start-failure path above, not as decision inputs.

**D6. Panel layout settings.**
- A new `[panel]` section holds `side` (`"left"` / `"right"`, default `"left"`), `cols`
  (1..=65535, default 40) and `gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right`
  (integers in pixels, may be negative, default 0).
- `mbv-config` validates these with plain Rust range checks (side enum, cols range, gutters i32).
  An out-of-range or malformed value falls back to its default per key, with a logged warning;
  the other keys keep their values. A combination that does not fit the output is caught only by
  pinwin: while pinned by `pinwin_apply_layout`, at launch by `pinwin_start` (D2 addition 2),
  which is a start failure (D5). It never
  calls pinwin: `mbv-config` sits under `mbvd` (D4), and geometry checks need live monitor
  metrics.
- F2: a `Panel` destination (like `Services`/`Keys`) on the main page holds six rows. `Side` is a
  `Text` row cycling `left`/`right`. `Cols` and the four
  gutters are a new stepper row kind: the keys that cycle a `Text` row's value step the number
  down and up by 1, with Shift by 10; `Cols` stops at 1 and 65535, gutters at the i32 bounds. No
  free-text entry.
- Each step while pinned calls `pinwin_apply_layout` with the full new layout. `PINWIN_OK` saves
  the value; a rejection leaves both the panel and `config.toml` unchanged, keeps the previous
  value on the row, and shows the reason in a Warning toast (the existing F2 feedback path).
  When not pinned, each step is saved without geometry validation; the stepper bounds already
  keep the values in range.
- The keyboard mode is fixed to `on-demand` (the previous default), and `COLS`/`GUTTER`/
  `PINWIN_DEBUG`/`--no-tray` have no equivalent.
- Naming: the layer-shell surface is the *pinned panel*, recorded in `CONTEXT.md` beside the
  in-TUI panel vocabulary (Library panel, playback panel) so the two senses of "panel" stay
  distinct.

**D7. Reverts.** The first version's packaging split, launcher module, ctrl capability, daemon
tray hook changes, `Pin options...` item and ADR 0004 amendment were removed in dedicated tasks
(section 2) rather than left dormant. The launcher revert restores `contrib/mbv.desktop` to its
pre-change form; task 4.4 then sets it to `Exec=mbv --pin`, `Terminal=false`. The ctrl, tray and
ADR 0004 reverts turned out to remove wanted behaviour; section 8 re-applies them adapted (D9).
The packaging split, `mbv --desktop` launcher and `PINWIN_SOCKET` wiring stay removed.

**D9. Tray access to the panel options.** The first version's behaviour comes back, adapted to
the in-process panel. The Owner's tray (`mbv-desktop`, ksni) shows a `Panel options...` item
while at least one pinned client is connected.
1. *Declaration.* A pinned client sends the Owner a ctrl command declaring that it is pinned (the
   first version's `pinned-panel` capability and `DeclarePinned`, `5690ab521`, `83e476b7a`,
   `951ca97a7`, `89d58bf51`), with no socket path: the panel is in the client process. The
   remote-player connect path declares the same way when the Owner supports the capability.
2. *Tray lifecycle.* The Owner starts the tray lazily for a pinned client even when stay-alive is
   off (`ca03159c4`), and the ADR 0004 amendment (`a36fef682`) records it. Stay-alive on or off,
   the tray's existing items behave as before.
3. *Action.* Selecting `Panel options...` makes the Owner send the declared pinned client a ctrl
   request. The client opens the F2 settings screen on its Panel page, and brings its panel
   forward. The F2 page (D6) is the options UI: live apply, saved on success, rejection toast.
   With several pinned clients, the request goes to every pinned client.
4. *Not restored.* pinwin's own tray, the control socket and the staged GTK options window
   (`Options...` with Apply/Close) are not restored; the library has no tray or window.
This design does not author separate `ctrl-protocol` / `local-daemon-tray` spec deltas: the
user-visible behaviour is in `pinned-launch`, and the ctrl vocabulary lives with `mbv-ctrl`.
Authoring those deltas is left to `/opsx:continue` if wanted.

**D8. Tests.** The layout core's contracts are Zig unit tests under `zig build check` upstream
(owned by `add-library-abi`). This change adds one Rust contract test: `[panel]` values out of
range fall back to their defaults and a valid section round-trips through save. The `--pin`
decision, the F2 stepper (it reuses the `Text` row's keys and save path), panel rendering and the
pty hand-over are covered by the task 6.2 manual check.

## Risks / Trade-offs

- **Upstream dependency:** blocked on `slatkin/pinwin` `add-library-abi` landing at the
  `library-abi` tag with the D2 additions (task 3.1). Sections 2, 4.2 and 4.3 do not depend
  on it; 4.1's linking and 4.4's start-up need the imported library.
- **Build cost:** `cargo build` needs Zig 0.16 and GTK4 / gtk4-layer-shell dev
  libraries and fetches libghostty-vt (pinned by hash). CI builds one way, with pinning; every
  package now carries the GTK runtime dependency (D4). Zig 0.16 in the pinned Arch CI image is
  checked in task 7.2.
- **TUI on a pty it did not start with:** terminal queries, image protocol and mouse modes go
  through pinwin's terminal. pinwin already supports what mbv uses (kitty graphics, mouse, kitty
  keyboard), and task 1.1 proves the hand-over end to end before the rest is built.
- **Link order:** gtk4-layer-shell shims libwayland-client symbols and only works if the dynamic
  linker loads it before libwayland-client (gtk4-layer-shell `linking.md`). libmpv also pulls in
  libwayland-client. `build.rs` links gtk4-layer-shell directly and first among the GTK libraries;
  task 4.1 proves the load order with `LD_DEBUG=libs`, since a wrong order makes every pinned
  launch fail.
- **Desktop launcher on GNOME/X11:** fails with a notification. Accepted by product decision.
