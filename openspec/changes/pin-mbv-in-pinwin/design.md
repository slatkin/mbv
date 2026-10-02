# Design

## Context

- **Launch.** `main()` runs `pre_config_startup()`, `applog::init`, `load_config`, then
  `run_local_instance`. Every local launch is a Client; a fresh launch spawns the Owner process
  (`local_daemon::spawn_detached`, `mbv --__local-daemon`) and attaches over the local ctrl socket.
  The tray lives in the Owner and is unchanged by this change.
- **Config and F2.** A bool setting is a field on `Config` (`mbv-config`), parsed in `parse.rs`,
  saved in `save.rs`, listed as a `SettingKey` (`mbv-ui-model/src/settings.rs`), toggled in
  `src/app/dispatch/settings.rs` and documented in `dist/config.toml`. `show_systray_icon` is the
  worked example.
- **pinwin today** (imported in the first unit of this change, `slatkin/pinwin@eaefd7f`): a Zig
  `main()` over libghostty-vt (a statically linked Zig dependency) plus C glue on GTK4,
  gtk4-layer-shell, pango and gio. Terminal state is module-level. `glue_start()` runs the GTK loop
  and `pty.c` runs `forkpty` for one command. It also has a control socket, its own tray, a GTK
  options window, a GKeyFile config and `COLS`/`GUTTER` env vars. Its pure layout core
  (`options.c`: side, columns, four gutters, strict parsing, checked geometry validation) has no
  GTK dependency.
- **Crate graph.** `mbvd` depends only on `mbv-core` and `mbv-daemon`; `mbv-desktop` is a leaf
  pulled in by the root `mbv` crate only.

## Goals / Non-Goals

**Goals:**
- Pinning is a feature of mbv: an F2 / `config.toml` setting, no separate program, package,
  launcher or config for the user.
- GTK never enters `mbv-core`, `mbv-daemon` or `mbvd`; the TUI binary gets it only behind a cargo
  feature.
- pinwin is first-party code, so surfaces that existed only because it was a separate program are
  removed, not bridged.

**Non-Goals:**
- No tray, ctrl or Owner changes. The Owner's tray stays as it is, and `mbvd` is unaffected.
- No pinwin command line, environment variables, socket protocol or standalone options window.
- No panel on non-Wayland or non-layer-shell sessions: they get the plain TUI.

## Decisions

**D1. The imported pinwin specs are edited in place.**
- `openspec/specs/pinwin-{panel,tray-options,control}` arrived as already-accepted behaviour (first
  version's D1) and are still edited directly, not through deltas. `pinwin-panel` is rewritten for
  the library form. `pinwin-tray-options` and `pinwin-control` are deleted, and their layout and
  validation rules fold into `pinwin-panel`.

**D2. pinwin is a static library with a C ABI.**
- `pinwin/build.zig` builds `libpinwin.a` (libghostty-vt included) instead of an executable, and
  keeps a unit-test step (`zig build check`). `src/main.zig`'s `main()` and env/argv parsing are
  removed.
- The ABI (`pinwin/src/pinwin_api.h`) is small and owned data only:
  - `pinwin_start(const PinwinStartup*) -> int`: startup struct = pty master fd, layout
    (`PinwinLayout`), keyboard mode. Returns a result code and never calls `exit`. Spawns the GTK
    thread.
  - `pinwin_apply_layout(const PinwinLayout*) -> int`: validates with the existing layout core and
    posts to the GTK loop; result code to the caller.
  - `pinwin_stop(void)`: closes the panel and joins the thread.
- `tray.c`, `control.c` and their libraries (gio tray publication,
  dbusmenu-glib) are deleted.

**D3. In-process panel on a pty pair.**
- When pinning applies (D5), before any terminal setup mbv opens a pty pair, `pinwin_start`s the
  panel on the master, and makes the slave the process's controlling terminal and stdio
  (`setsid`, `TIOCSCTTY`, `dup2` to 0/1/2). The TUI then runs exactly as in a terminal, with
  terminal size changes arriving as `SIGWINCH` from the pty.
- `pty.c` no longer forks: it reads and writes the supplied master fd and applies the window size
  to it. The panel closes when the master sees hangup or `pinwin_stop` is called; mbv exits when
  the TUI exits, so the panel disappears with it.
- Alternative: re-exec mbv as the panel's pty child. Rejected: live layout changes from F2 would
  then need an IPC channel from the TUI child to the panel in the parent, which is exactly the
  socket this design removes.
- Alternative: pinwin as a separate executable. Rejected by the product definition (no
  user-visible pinwin) and by the crate-graph constraint of D4.
- Risk: the Owner spawn and any child processes must not inherit the pty as their controlling
  terminal. `spawn_detached` already detaches; task 1.1 probes this against the real system.

**D4. Cargo feature gate and the crate graph.**
- A new leaf crate `crates/mbv-pinwin` owns `build.rs` (runs `zig build`, links `libpinwin.a`,
  GTK4, gtk4-layer-shell, pango/cairo, libghostty-vt's static archive) and the safe Rust wrapper
  over the C ABI. The root `mbv` crate depends on it behind the cargo feature `pinning`.
- `mbv-core`, `mbv-daemon`, `mbvd` and every other crate MUST NOT depend on `mbv-pinwin`, directly
  or transitively. Release builds for the desktop packages enable `pinning`; the `.deb` and
  `mbvd` do not.
- With the feature off, the F2 rows are absent and `pin_as_panel` in `config.toml` is read and
  preserved but ignored.

**D5. When the panel is used.**
- Pinning applies only on the interactive TUI launch path, decided after `load_config` and before
  terminal initialisation, when all of these hold: the `pinning` feature is built in,
  `pin_as_panel` is true, `WAYLAND_DISPLAY` is set, and `pinwin_start` succeeds.
- Otherwise the TUI starts in the current terminal as today. A `pinwin_start` failure (no
  layer-shell, GTK init error) is logged and falls back; it is never fatal and prints nothing on
  the terminal. The flags `-h`, `-V`, `-q`, `--__local-daemon` and `--connect-daemon` never
  pin.

**D6. Settings.**
- `[display]` gains `pin_as_panel = false`. A new `[panel]` section holds `side` (`"left"` /
  `"right"`), `cols` (1..=65535, default 40) and `gutter_top`, `gutter_bottom`, `gutter_left`,
  `gutter_right` (integers in pixels, may be negative, default 0). Invalid values are rejected by
  the pinwin layout core and fall back to defaults with a logged warning.
- F2: the main page gets a `PinAsPanel` row (toggle, "takes effect on next launch"); a `Panel`
  destination (like `Services`/`Keys`) holds the six layout rows. Both are shown only when the
  feature is built and `WAYLAND_DISPLAY` is set.
- Editing a layout row while pinned calls `pinwin_apply_layout` with the full new layout; a
  rejected layout is shown as an inline error and not saved. Outside the panel the values are only
  saved.
- The keyboard mode is fixed to `on-demand` (the previous default), and `COLS`/`GUTTER`/
  `PINWIN_DEBUG`/`--no-tray` have no equivalent.

**D7. Reverts.** The first version's packaging split, launcher module, ctrl capability, daemon
tray hook changes, `Pin options...` item and ADR 0004 amendment are removed in dedicated tasks
rather than left dormant; `contrib/mbv.desktop` returns to `Exec=mbv`.

**D8. Tests.** The layout core's contracts (strict parsing, checked geometry, validation) become
Zig unit tests under `zig build check`, replacing `tools/check_options.c`. The Rust side tests
config parse/save of the new keys and the F2 dispatch. Panel rendering and the pty hand-over are
manual checks.

## Risks / Trade-offs

- **Build cost:** with the feature on, `cargo build` needs Zig 0.16 and GTK4 / gtk4-layer-shell dev
  libraries and fetches libghostty-vt (pinned by hash). Accepted behind the feature; CI builds
  both ways.
- **TUI on a pty it did not start with:** terminal queries, image protocol and mouse modes now go
  through pinwin's terminal. pinwin already supports what mbv uses (kitty graphics, mouse, kitty
  keyboard), and task 1.1 proves it end to end before the rest is built.
- **Launching from a terminal with pinning on:** the panel opens, and the launching terminal stays
  attached to the foreground mbv process until it exits. Accepted; disabling the setting from F2
  inside the panel restores the old behaviour.
- **GNOME Wayland (no layer-shell):** start fails, falls back to the plain TUI, logged only.
