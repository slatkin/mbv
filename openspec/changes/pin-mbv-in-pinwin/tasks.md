# Tasks

Rewritten around the in-app model (proposal, design D1–D8). Section 0 is done work that is kept;
section 2 removes done work that the rewrite supersedes.

## 0. Carried over (done in the first version)

- [x] 0.1 Import `~/Dev/pinwin/pinwin/` into `pinwin/` and the three pinwin specs into `openspec/specs/` (commit e20c10e6a, `slatkin/pinwin@eaefd7f`).
- [x] 0.2 `*.zig` governed by `check-code-file-lines` (e20c10e6a).
- [x] 0.3 `pinwin/AGENTS.md` and the root `AGENTS.md` map line (e20c10e6a); reworded for the library in 5.2.

## 1. Probe

- [ ] 1.1 Read how the TUI takes its terminal (crossterm/ratatui setup, any `/dev/tty`, stdin or stdout assumptions, image-protocol and mouse detection, embedded mpv output) and how `local_daemon::spawn_detached` and mpv/child processes inherit the controlling terminal. Record findings and any needed adjustment under D3 in `design.md` ("Probe result"). Verify: `design.md` D3 names the exact terminal-open site and states whether `setsid` + `TIOCSCTTY` + `dup2` onto 0/1/2 before it is sufficient.

## 2. Remove the first version's external-program wiring

Use `git revert --no-edit`, one revert commit per group, newest first, so history is preserved.

- [ ] 2.1 Revert the tray work: `89d27cf82`, `e37f6770a`, `ca03159c4` and the ADR 0004 amendment `a36fef682`. Verify: `git diff 8fb44d79c -- crates/mbv-daemon crates/mbv-desktop docs/adr/0004-tray-observer-and-non-takeover-commands.md Cargo.lock` is empty apart from changes made after 8fb44d79c elsewhere, and `cargo nextest run -p mbv-daemon -p mbv-desktop` passes.
- [ ] 2.2 Revert the ctrl capability and declaration: `83e476b7a`, `951ca97a7`, `89d58bf51`, `5690ab521`. Verify: `cargo nextest run -p mbv-ctrl -p mbv-daemon -p mbv-remote-player -p mbv` passes and `rg -n "DeclarePinned|pinned.panel|PINWIN_SOCKET" crates src` finds nothing.
- [ ] 2.3 Revert the launcher: `79a49de20`, `4bc2fdb6e`. Verify: `desktop-file-validate contrib/mbv.desktop` passes, `rg -n "\-\-desktop|desktop_launch" src README.md contrib` finds nothing, and `cargo nextest run -p mbv` passes.
- [ ] 2.4 Revert the packaging split: `91d68daf4`. Verify: `makepkg --printsrcinfo` lists the single `mbv` package for both PKGBUILDs and `aur.yml`'s `pkgver`/`sha256sums` rewrite still matches the file's lines.

## 3. pinwin as a library

- [ ] 3.1 Layout-core tests: write Zig unit tests (fresh, against `options.h`, no ported harness) under `zig build check` for the contracts: strict column parsing (1..=65535), strict gutter parsing (optional `-`, digits only), checked side geometry, and layout validation (overflow, negative reservation, no row, no width). Delete `tools/check_options.c`. Verify: `cd pinwin && zig build check` passes.
- [ ] 3.2 Remove what exists only because pinwin was a program: `control.c/h`, `tray.c/h`, the GTK options window and GKeyFile config in `options.c` (keep the pure layout core), `main()`, the `COLS`/`GUTTER`/`PINWIN_DEBUG` env parsing, command argv and `--no-tray`, and the gio / dbusmenu-glib link lines in `build.zig`. Verify: `cd pinwin && zig build check` passes and `rg -n "PINWIN_SOCKET|no_tray|dbusmenu|COLS" pinwin/src pinwin/build.zig` finds nothing.
- [ ] 3.3 Add `pinwin/src/pinwin_api.h` and its implementation (design D2): `pinwin_start(const PinwinStartup*)` (pty master fd, layout, keyboard mode; returns a result code, never `exit`; starts the GTK thread), `pinwin_apply_layout(const PinwinLayout*)` (validates with the layout core, posts to the GTK loop), `pinwin_stop()`. `pty.c` reads/writes the supplied master fd and applies window size to it instead of `forkpty`. `build.zig` builds `libpinwin.a` (libghostty-vt included) instead of an executable. Contract test (Zig, in `zig build check`): `pinwin_apply_layout` rejects an invalid layout with a distinct result code before touching GTK. Verify: `cd pinwin && zig build -Doptimize=ReleaseSafe && zig build check` passes and `zig-out/lib/libpinwin.a` exists.
- [ ] 3.4 Specs in `openspec/specs/` (design D1): rewrite `pinwin-panel` for the library form (no command/env/install requirements; docking, reservation, gutters, keyboard focus, terminal features, font, size reports, exit and layer-shell requirements kept; layout and validation rules from `pinwin-tray-options` folded in) and delete `pinwin-tray-options` and `pinwin-control`. Verify: `openspec validate --specs --strict` passes.

## 4. mbv wiring

- [ ] 4.1 New leaf crate `crates/mbv-pinwin`: `build.rs` runs `zig build` for `pinwin/` and links `libpinwin.a`, GTK4, gtk4-layer-shell and pango/cairo; a safe wrapper over the C ABI (owned types, result enum, no raw pointers in the public API). The root `mbv` crate depends on it behind the cargo feature `pinning`. Verify: `cargo check -p mbv`, `cargo check -p mbv --features pinning`, and `cargo tree -p mbvd -i mbv-pinwin` and `cargo tree -p mbv-core -i mbv-pinwin` both report that the package is not in the graph.
- [ ] 4.2 `crates/mbv-config`: add `pin_as_panel` (`[display]`, default false) and the `[panel]` keys (`side`, `cols`, `gutter_top/bottom/left/right`) with parse, save and `dist/config.toml` documentation. Contract test: invalid `[panel]` values fall back to their defaults and a valid file round-trips through save. Verify: `cargo nextest run -p mbv-config`.
- [ ] 4.3 F2: a `PinAsPanel` row (toggle, note "takes effect on next launch") on the main page and a `Panel` destination with the six layout rows, in `mbv-ui-model/src/settings.rs` and `src/app/dispatch/settings.rs`, shown only when the `pinning` feature is built and `WAYLAND_DISPLAY` is set. A rejected layout shows an inline error and is not saved. Contract test: toggling the row flips and saves the config value; rows are absent when gated off. Verify: `cargo nextest run -p mbv-ui-model -p mbv`.
- [ ] 4.4 Start-up in `src/main.rs` (design D3, D5): a pure `should_pin(...)` decision (flags, feature, setting, `WAYLAND_DISPLAY`) with a `#[case]` table, then, before terminal initialisation, the pty pair, controlling-terminal and stdio hand-over and `pinwin_start`; on failure log and continue in the terminal. Editing a Panel row while pinned calls `pinwin_apply_layout`. Verify: `cargo nextest run -p mbv` and `cargo check -p mbv --features pinning`.

## 5. Packaging, CI, docs

- [ ] 5.1 `build.yml`: build the desktop release binary with `--features pinning` after installing `zig gtk4 gtk4-layer-shell` in the pinned image, and keep a build without the feature; the tarball gains no new file. `PKGBUILD` and `PKGBUILD-git`: single package; `makedepends` gain `zig gtk4 gtk4-layer-shell`, `depends` gain the GTK4 runtime libraries, and the build uses `--features pinning`. The `.deb` and `mbvd` build are unchanged. Verify by reading the diffs: no pinwin package, no pinwin executable, and `makepkg --printsrcinfo` lists one package for each PKGBUILD.
- [ ] 5.2 Docs: document the setting and the `[panel]` keys in `README.md`; reword `pinwin/AGENTS.md` and the root `AGENTS.md` line for the library (no executable, `zig build check`). Verify: `rg -n "pinwin" README.md AGENTS.md pinwin/AGENTS.md` shows no mention of a user-run `pinwin` command.

## 6. Integration check

- [ ] 6.1 Run `cargo clippy --workspace --all-targets -- -D warnings` and again with `--features pinning` on the `mbv` package, `cargo fmt --all -- --check` and `make check-code-file-lines`, and verify they all pass.
- [ ] 6.2 Manual check under niri:
  - Setting off: mbv opens in the terminal as before.
  - Enable "Pin app as UI panel" in F2, quit, relaunch: mbv opens docked in the panel; posters, mouse and resize work; quitting removes the panel.
  - Change width, side and a gutter in F2's Panel page while pinned: the panel updates live and survives a relaunch.
  - Stay-alive on or off: the Owner's tray behaves as it did before this change.
  - A session without layer-shell, or `WAYLAND_DISPLAY` unset: no pinning rows in F2 and the terminal launch works.
  - `mbvd` and the `.deb` build without Zig or GTK.
