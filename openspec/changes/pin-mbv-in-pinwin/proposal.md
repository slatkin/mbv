# Proposal

Issue: #864

## Why

mbv is an app with pinning support. A user turns on "Pin app as UI panel" in F2 (or `config.toml`),
and the next time mbv launches, its TUI is docked to a screen edge in a GTK layer-shell panel. There
is no second program for the user to install, launch or configure: pinwin is mbv's own code and
becomes part of mbv. The first version of this change kept pinwin as an external program with its
own launcher, package, tray, config and control socket. That is not the product.

## What Changes

- **Setting**: `pin_as_panel` (default off) in `config.toml`, with an F2 toggle. It takes effect on
  the next launch. A `Panel` settings page holds the panel layout: side, columns and four gutters.
  Layout changes apply live while pinned.
- **pinwin becomes a library**: the `pinwin/` tree (Zig + C) is reshaped into a static library
  with a C ABI, linked into the `mbv` TUI binary. Its executable entry, `COLS`/`GUTTER` env
  vars, command argv, `--no-tray`, control socket (`PINWIN_SOCKET`), own tray and own GTK options
  window / config file are removed. It attaches to a pty that mbv supplies and exposes
  start, apply-layout and stop calls.
- **In-process panel**: with pinning on, mbv opens a pty pair, runs the panel on a GTK thread
  against the master, and runs the TUI unchanged on the slave. One process; no re-exec, no socket.
  mbv without pinning, off Wayland, or when panel start-up fails behaves as today.
- **Cargo feature gate**: pinning sits behind a cargo feature of the `mbv` TUI crate. `mbv-core`,
  `mbv-daemon` and `mbvd` MUST NOT depend on it, directly or transitively; `mbvd` and the `.deb`
  carry no GTK.
- **Reverted from the first version**: the `pinwin` package split and `optdepends`, the
  `mbv --desktop` launcher, the ctrl `DeclarePinned`/`pinned-panel` capability, the lazily started
  tray and the `Pin options...` tray item, and the ADR 0004 amendment. `contrib/mbv.desktop`
  returns to a plain `Exec=mbv`.
- Repo hygiene stays: `pinwin/AGENTS.md`, the root `AGENTS.md` map line and `*.zig` under
  `check-code-file-lines`.

## Capabilities

### New Capabilities
- `pinned-launch`: the pin setting, the Panel layout settings, in-process panel start-up, and
  fallback to the plain TUI.

### Modified Capabilities
- `pinwin-panel`, `pinwin-tray-options`, `pinwin-control`: these were copied into
  `openspec/specs/` as already-accepted behaviour (design D1) and are edited there directly, not as
  deltas. `pinwin-panel` is rewritten for the library form; `pinwin-tray-options` and
  `pinwin-control` are retired and their layout rules move into `pinwin-panel`.
- `ctrl-protocol` and `local-daemon-tray`: no change. Their deltas from the first version are
  dropped.

## Impact

- `pinwin/` (library reshape), new leaf crate `crates/mbv-pinwin` (build.rs, FFI), `mbv-config`
  (keys), `mbv-ui-model` and `src/app/dispatch/settings.rs` (F2 rows), `src/main.rs` (pin start-up
  before terminal init), `dist/config.toml`.
- Removes the `PKGBUILD`/`PKGBUILD-git` split, the `build.yml` pinwin steps beyond the library
  build, `src/desktop_launch.rs`, and the ctrl, daemon and tray code added by the first version.
- With the feature on, building needs Zig and the GTK4 / gtk4-layer-shell development libraries;
  the TUI binary links them. With it off, nothing changes.
- Depends on nothing outside this repo: slatkin/pinwin#1 only supplied the code that is imported.
