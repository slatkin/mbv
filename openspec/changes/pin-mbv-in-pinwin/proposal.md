# Proposal

Issue: #864

## Why

mbv is an app with pinning support. `mbv --pin` starts mbv with its TUI docked to a screen edge in
a GTK layer-shell panel, and the desktop entry launches it that way. There is no second program
for the user to install, launch or configure: pinwin is mbv's own code and becomes part of mbv.
Plain `mbv` in a terminal (including over ssh or in tmux) always runs in that terminal. The first
version of this change kept pinwin as an external program with its own launcher, package, tray,
config and control socket. That is not the product.

## What Changes

- **Flag**: `mbv --pin` runs the TUI in the pinned panel. There is no config setting that turns
  pinning on; only the flag does. A `Panel` settings page in F2 holds the panel layout: side,
  columns and four gutters, saved in `config.toml` and applied live while pinned.
- **Desktop entry**: `contrib/mbv.desktop` becomes `Exec=mbv --pin`, `Terminal=false`. Sessions
  without layer-shell (GNOME, X11) are not supported from the launcher: the panel fails to start,
  mbv logs it, sends a desktop notification and exits non-zero. Every package ships the same
  binary and the same desktop entry.
- **pinwin becomes a library, upstream-first**: the reshape of the `pinwin/` tree (Zig + C) into a
  static library with a C ABI is specified, reviewed and implemented in `slatkin/pinwin` (change
  `add-library-abi`). This change imports the resulting revision over `pinwin/` and links it into
  the `mbv` TUI binary. Upstream removes the executable entry, `COLS`/`GUTTER` env vars, command
  argv, `--no-tray`, control socket (`PINWIN_SOCKET`), own tray, own GTK options window and own
  config file; the library attaches to a pty that mbv supplies and exposes start, apply-layout and
  stop calls.
- **In-process panel**: with `--pin`, mbv opens a pty pair, runs the panel on a GTK thread
  against the master, and runs the TUI unchanged on the slave. One process; no fork, no re-exec,
  no socket. Typed in a terminal, `mbv --pin` behaves like a GUI app started from a shell: the
  panel opens and the shell waits until mbv exits; if the panel cannot start, mbv warns and runs
  in that terminal.
- **One build, runtime flag**: `mbv` always links the pinwin library; `crates/mbv-pinwin` is an
  ordinary dependency of the root crate, still a leaf not depended on by `mbv-core`, `mbv-config`,
  `mbv-daemon` or `mbvd`. `--pin` is a runtime flag and the F2 Panel page is always present. There
  is one build, one tarball binary and one `.deb`.
- **Reverted from the first version**: the `pinwin` package split and `optdepends`, the
  `mbv --desktop` launcher, the ctrl `DeclarePinned`/`pinned-panel` capability, the lazily started
  tray and the `Pin options...` tray item, and the ADR 0004 amendment.
- Repo hygiene stays: `pinwin/AGENTS.md`, the root `AGENTS.md` map line and `*.zig` under
  `check-code-file-lines`.
- Naming: the layer-shell surface is recorded in `CONTEXT.md` as the *pinned panel*, kept
  distinct from the existing in-TUI panel vocabulary (Library panel, playback panel).

## Capabilities

### New Capabilities
- `pinned-launch`: the `--pin` flag, the desktop entry, the Panel layout settings, in-process
  panel start-up, and the start-failure behaviour.

### Modified Capabilities
- `pinwin-panel`, `pinwin-tray-options`, `pinwin-control`: the copies in `openspec/specs/` are
  re-imported from the library-form pinwin revision (design D1), not edited here. Upstream's
  `add-library-abi` rewrites `pinwin-panel` for the library form and retires
  `pinwin-tray-options` and `pinwin-control`; the re-import lands the same result here.
- `ctrl-protocol` and `local-daemon-tray`: no change. Their deltas from the first version are
  dropped.

## Impact

- `pinwin/` (re-import of the library-form revision; the reshape itself is upstream work), new
  leaf crate `crates/mbv-pinwin` (build.rs, FFI), `mbv-config` (`[panel]` keys), `mbv-ui-model`
  and `src/app/dispatch/settings.rs` (F2 Panel page), `src/main.rs` (`--pin` and start-up before
  terminal init), `contrib/mbv.desktop`, `dist/config.toml`, `CONTEXT.md` (pinned-panel terms),
  root `Cargo.toml` (`.deb` assets).
- Removes the `PKGBUILD`/`PKGBUILD-git` split, the `build.yml` pinwin steps beyond the library
  build, `src/desktop_launch.rs`, and the ctrl, daemon and tray code added by the first version.
- Building needs Zig and the GTK4 / gtk4-layer-shell development libraries; the TUI binary links
  them, and the mbv `.deb`, tarball and PKGBUILDs carry the GTK4 / gtk4-layer-shell runtime
  dependency.
- Depends on `slatkin/pinwin` change `add-library-abi` landing at the `library-abi` tag, including
  the SIGWINCH-on-resize addition design D3 requires of it. Nothing else outside this repo:
  slatkin/pinwin#1 only supplied the code that is imported.
