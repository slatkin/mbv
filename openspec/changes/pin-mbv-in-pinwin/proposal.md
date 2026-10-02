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
- **pinwin becomes a library, upstream-first**: the reshape of the `pinwin/` tree (Zig + C) into a
  static library with a C ABI is specified, reviewed and implemented in `slatkin/pinwin` (change
  `add-library-abi`). This change imports the resulting revision over `pinwin/` and links it into
  the `mbv` TUI binary. Upstream removes the executable entry, `COLS`/`GUTTER` env vars, command
  argv, `--no-tray`, control socket (`PINWIN_SOCKET`), own tray, own GTK options window and own
  config file; the library attaches to a pty that mbv supplies and exposes start, apply-layout and
  stop calls.
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
- Naming: the layer-shell surface is recorded in `CONTEXT.md` as the *pinned panel*, kept
  distinct from the existing in-TUI panel vocabulary (Library panel, playback panel).

## Capabilities

### New Capabilities
- `pinned-launch`: the pin setting, the Panel layout settings, in-process panel start-up, and
  fallback to the plain TUI.

### Modified Capabilities
- `pinwin-panel`, `pinwin-tray-options`, `pinwin-control`: the copies in `openspec/specs/` are
  re-imported from the library-form pinwin revision (design D1), not edited here. Upstream's
  `add-library-abi` rewrites `pinwin-panel` for the library form and retires
  `pinwin-tray-options` and `pinwin-control`; the re-import lands the same result here.
- `ctrl-protocol` and `local-daemon-tray`: no change. Their deltas from the first version are
  dropped.

## Impact

- `pinwin/` (re-import of the library-form revision; the reshape itself is upstream work), new
  leaf crate `crates/mbv-pinwin` (build.rs, FFI), `mbv-config`
  (keys), `mbv-ui-model` and `src/app/dispatch/settings.rs` (F2 rows), `src/main.rs` (pin start-up
  before terminal init), `dist/config.toml`, `CONTEXT.md` (pinned-panel terms).
- Removes the `PKGBUILD`/`PKGBUILD-git` split, the `build.yml` pinwin steps beyond the library
  build, `src/desktop_launch.rs`, and the ctrl, daemon and tray code added by the first version.
- With the feature on, building needs Zig and the GTK4 / gtk4-layer-shell development libraries;
  the TUI binary links them. With it off, nothing changes.
- Depends on `slatkin/pinwin` change `add-library-abi` landing at a pinned revision (its own
  planning review and implementation round). Nothing else outside this repo: slatkin/pinwin#1
  only supplied the code that is imported.
