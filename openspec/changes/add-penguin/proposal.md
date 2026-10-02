# Proposal

## Why

penguin is a standalone Wayland layer-shell panel terminal that pins a command (typically mbv)
to a screen edge. It currently lives in the separate `pinwin` repository. mbv wants to integrate it
(one launcher, one tray, see `pin-mbv-in-penguin`), which requires penguin to ship from this repo
and to accept commands from outside its own tray. This change is the penguin-side prerequisite. It
adds no Rust code and no mbv behaviour.

## What Changes

- Import `pinwin/penguin/` (Zig + C, its own `build.zig`) into this repo as `penguin/`. penguin
  stays a standalone, general-purpose app that runs any command and knows nothing about mbv. The
  `pinwin` Bash script stays behind in the old repo.
- Bring penguin's accepted specs (`penguin-panel`, `penguin-tray-options`) into `openspec/specs/`.
  Prerequisite outside this repo: archive pinwin's completed `add-penguin-cols-option` first, so
  the imported specs are current.
- Split `penguin/src/glue.c` (1637 lines) and `penguin/src/main.zig` (952 lines) along
  responsibility seams to respect the 800-line cap. Add `*.zig` to the governed extensions of
  `check-code-file-lines`.
- Drop `penguin/tools/gen_nerd_tables.py` (bespoke script); keep the generated
  `nerd_font_tables.h`.
- **Control API**: each penguin instance listens on a Unix socket under
  `$XDG_RUNTIME_DIR/penguin/` and exports its path to the child as `PENGUIN_SOCKET`. The protocol
  is one request per line. The first and only request is `options`, which opens that instance's
  Options window exactly as the tray's `Options...` does.
- **`--no-tray`**: penguin skips registering its own StatusNotifierItem, for hosts that offer
  penguin's options from their own tray through the control API.
- With no command, penguin runs `$SHELL` instead of the hardcoded `mbv`.
- The tray icon is installed with penguin, no longer read from `$HOME/penguin.svg`.
- Wayland-only, by design. On a compositor without wlr-layer-shell (including GNOME), penguin
  exits with a diagnostic. There is no fallback window.
- Packaging: penguin ships as a separate `penguin` package (split package in `PKGBUILD` /
  `PKGBUILD-git`) that owns its GTK4 / gtk4-layer-shell / libdbusmenu-glib dependencies. The `mbv`
  package gains no GTK dependency. CI builds penguin with Zig 0.16 and adds it to the release
  tarball. The `.deb` (mbvd-only) is unaffected.
- Scoped `penguin/AGENTS.md` with the Zig/C conventions and penguin's check program, plus one
  line in the root `AGENTS.md` repository map.

## Capabilities

### New Capabilities
- `penguin-panel`: imported from pinwin. Delta: default command is `$SHELL`; Wayland/layer-shell
  is required, and its absence is a diagnosed exit.
- `penguin-tray-options`: imported from pinwin. Delta: `--no-tray` suppresses the tray; the icon
  comes from the installed location.
- `penguin-control`: the per-instance control socket, `PENGUIN_SOCKET` in the child environment,
  and the line protocol (`options`).

### Modified Capabilities
None. No existing mbv capability changes.

## Impact

- New top-level `penguin/` tree (Zig/C). Cargo is untouched.
- `scripts/check-code-file-lines.sh` (adds `*.zig`), `PKGBUILD`, `PKGBUILD-git`,
  `.github/workflows/build.yml` (Zig step, tarball), root `AGENTS.md` map.
- New runtime deps live only in the `penguin` package: gtk4, gtk4-layer-shell, libdbusmenu-glib,
  librsvg (tray icon).
- Unblocks `pin-mbv-in-penguin`.
