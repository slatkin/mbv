# Proposal

Issue: #864

## Why

mbv should open pinned to a screen edge from its one launcher entry, with exactly one tray icon
(mbv's). The panel program, pinwin (formerly penguin), lives in its own repo with its own tray. It
gains a control socket and `--no-tray` in its own change (slatkin/pinwin#1). This change brings
pinwin's code into this repo and connects it to mbv: one launcher, one tray, and a tray item that
drives the panel.

## What Changes

- **Import pinwin**: copy the pinwin program (Zig + C, its own `build.zig`) into `pinwin/` and its
  accepted specs into `openspec/specs/`. pinwin stays a standalone panel terminal that runs any
  command. Cargo never builds it, and mbv never links it.
- **Packaging**: pinwin ships as its own `pinwin` package (split from the same `PKGBUILD` /
  `PKGBUILD-git`) that owns its GTK4 / gtk4-layer-shell / libdbusmenu-glib dependencies. `mbv`
  lists it in `optdepends` and gains no GUI dependency. CI builds and checks pinwin with Zig 0.16
  and adds it to the release tarball. The `.deb` (mbvd-only) is unaffected.
- **One launcher**: `contrib/mbv.desktop` stays the only desktop entry and starts `mbv --desktop`
  with no terminal. `mbv --desktop` runs `pinwin --no-tray mbv` when pinwin is installed, or opens
  mbv in the user's terminal through `xdg-terminal-exec` when it is not.
- **Pinned Client declares itself**: a Client started inside pinwin (it sees `PINWIN_SOCKET`) tells
  its local Owner process over ctrl that it is pinned, with the socket path. This is an additive
  ctrl capability, not a protocol version bump.
- **One tray, mbv's icon**: the Owner's tray is shown when stay-alive is enabled, as today, **or**
  once a pinned Client has declared itself. Nothing in the tray menu is specific to stay-alive.
- **Pin item**: while a pinned Client is attached, the tray shows `Pin options...`, which asks
  that Client's pinwin to open its options window.
- Repo hygiene: a scoped `pinwin/AGENTS.md` (Zig/C conventions, `zig build check`), one line in
  the root `AGENTS.md` repository map, and `*.zig` governed by `check-code-file-lines`.

## Capabilities

### New Capabilities
- `pinned-launch`: the single mbv desktop launcher with its pinwin and terminal paths, how a
  Client detects it is pinned and declares it, and pinwin shipping as its own package.
- `pinwin-panel`, `pinwin-tray-options`, `pinwin-control`: copied from pinwin's main specs as
  part of the code import, minus the two requirements that only make sense in the pinwin repo
  ("Install from the checkout", "Coexists with pinwin"). They are not authored as deltas here; see
  design D1.

### Modified Capabilities
- `local-daemon-tray`: the tray belongs to the local daemon whether or not stay-alive is enabled,
  and starts when stay-alive is enabled or when a pinned Client declares itself. It gains the
  `Pin options...` item while a pinned Client is attached.
- `ctrl-protocol`: additive capability for a local Client to declare itself pinned with its pinwin
  socket path.

## Impact

- New top-level `pinwin/` tree (Zig/C). The Cargo workspace is untouched by it.
- `contrib/mbv.desktop`; `src/main.rs` plus a small launcher module (`--desktop`); the Client
  attach path; `crates/mbv-ctrl` (capability, command); `crates/mbv-daemon` (ctrl handling,
  per-client pinned state, lazy tray start); `src/local_daemon.rs` (tray hook);
  `crates/mbv-desktop/src/tray.rs` (menu item and socket request).
- `PKGBUILD`, `PKGBUILD-git`, `.github/workflows/build.yml`, `scripts/check-code-file-lines.sh`,
  `AGENTS.md`.
- Depends on slatkin/pinwin#1 (`PINWIN_SOCKET`, the `options` request, `--no-tray`) having landed
  in pinwin before the import.
- `mbvd` unaffected (still no tray). mbv without pinwin behaves as today, apart from the launcher
  opening the terminal through `xdg-terminal-exec`.
