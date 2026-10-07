# Proposal

## Why

Issue #883. mbv pins pinwin at tag `0.2.2`, a numbering error that no longer exists upstream. The
pinned panel can't be hidden and shown again, so a compositor key has nothing to call. pinwin
0.3.0 (slatkin/pinwin#30, change `serve-instance-socket`) drops GTK and adds what mbv needs:
- `Panel::toggle` and `Panel::show`.
- A per-name instance socket the host binds before any surface opens.
- `instance::send`, so another process can ask a running panel to toggle or show.

## What Changes

- Bump `pinwin` to 0.3.0 (rev `8127ff81079d91f326dc5b8427ab03a1dc17992c`), and build `Startup`
  through `Startup::new`.
- `mbv --pin` binds the instance socket `mbv` before the panel opens. The bound socket is handed
  to the panel, which then serves `toggle` and `show` requests for as long as it runs.
- **BREAKING**: if a pinned mbv is already running on the display, a second `mbv --pin` asks it
  to show (a no-op when it is shown) and exits 0. Each `--pin` no longer opens its own panel.
- New `mbv --toggle`: hides the running pinned panel, or shows it if hidden, then exits. With no
  pinned mbv running it reports so and exits 1. It never launches mbv. This is the command to
  bind to a compositor key.
- A hidden panel follows the existing `[panel] cover` setting, with no new config. Pushing frees
  the reserved strip while hidden. Covering moves no tiled window.
- Packaging and CI drop GTK and gtk4-layer-shell, and take libxkbcommon and fontconfig instead.
- Remove the stale GTK wording from the spec, `CONTEXT.md`, `AGENTS.md` and the `src/pin.rs`
  comments.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pinned-launch`:
  - Pin flag: a second `--pin` shows the running panel.
  - Start failure: drop the GTK cause.
  - The daemon-isolation requirement no longer names GTK.
  - New panel toggle requirement for `--toggle`.
  - Covering mode: covers hidden-panel behaviour.

## Impact

- Code: `src/pin.rs` (socket bind, show-existing outcome, `Startup::new`, toggle client, comments)
  and `src/main.rs` (`--toggle` parsing, help text, the `--pin` outcome).
- Dependencies: `Cargo.toml` pinwin pin and deb `depends`, `PKGBUILD`, `PKGBUILD-git`, and
  `.github/workflows/build.yml` (pacman packages and the deb `Depends` check).
- Docs: `openspec/specs/pinned-launch/spec.md` (on sync), `CONTEXT.md` *Pinned panel*, and the
  pinwin entry in `AGENTS.md`.
- Runtime: the socket lives at `$XDG_RUNTIME_DIR/pinwin/$WAYLAND_DISPLAY-mbv.sock`, which pinwin
  owns and removes. No mbv IPC, PID file or shown-state tracking is added.
