# Proposal

## Why

The GUI Client cannot dock beside tiled windows. The TUI can, but only through a chain of
programs: `mbv-tui --pin` runs `pinwin run`, which runs a terminal, which runs the TUI, and each
width change is one more `pinwin` child process. The GUI needs a pin that lives inside the
running window, that the mouse drives, and that lets the window shrink to its collapsed rail
(issue #1066). A tiled window cannot do that, because the tiling window manager sets its size.

## What Changes

- New: the GUI window has a Pin control in the global header. A press docks the running window
  at the left or the right output edge as a Wayland layer-shell surface. The control then shows
  as Unpin, and a press puts the window back as a normal tiled window. Pin and Unpin keep the
  window's state: the destination, the picked item, the scroll positions, the open overlays, and
  playback. There is no new process and no Owner reattach.
- New: the GUI links pinwin as a library and hosts pinwin's Wayland proxy on a thread in its own
  process. On a Wayland session the GUI window always connects to the compositor through that
  proxy. While the window is not pinned, the proxy passes every request through.
- New: a pinned window has two views. The Queue view is the Queue panel alone, at the Queue
  column's width. The Full view is the Library and the Queue panel. Neither view has a
  configured width. The Queue view follows the Queue column width (the 72 px rail included).
  The Full view starts at the natural Wide width and remembers the last width the user dragged
  it to.
- New: the mouse moves between the views. A destination press in the header opens the Full
  view. A press on the active destination returns to the Queue view. A play from the Library and
  a keyboard focus loss also return to the Queue view. The rail's expand control and the
  footer's collapse control move between the Queue view and the rail. A drag on the window's
  inner edge resizes the current view.
- New: the Queue view pushes the tiled windows aside while `[panel] reserve` is on and covers
  them while it is off. The Full view always covers them, so a view change moves no tiled
  window.
- New: the GUI saves at an orderly exit whether it was pinned, and starts pinned the next time.
- Changed: in a pinned Queue view, the collapse shrinks the window to the rail, and the footer
  shows the collapse control (issue #1066). An unpinned queue-only window keeps today's rule.
- Changed: in a pinned window, a destination press in Mini opens the Full view and does not
  swap the Library into the Queue column's place.
- Changed: the GUI Settings screen gets a Panel section with Side, Reserve space, the four
  gutters, and the accent settings. These are the same `[panel]` keys that the TUI uses. Width
  and Expanded width stay TUI-only.
- Changed: the shipped `mbv` binary links pinwin, which is GPL-3.0-or-later, so the binary is
  distributed as a GPL-3.0-or-later combined work. The mbv source stays MIT. The packages ship
  the GPL text.
- Prerequisite, outside this repository: pinwin publishes an `embed` library feature in a
  `v0.5.0` tag. That is a pinwin OpenSpec change that only the user authorizes in that
  repository. This change states the API it needs in `design.md`.
- Not changed: the TUI pinned panel, `mbv-tui --pin`, `--toggle`, the Pin swap, and the Tray.
  The GUI never advertises `pinned-surface`, takes no part in Pin swap, and a Tray left click
  does not hide or show the GUI window.

## Capabilities

### New Capabilities

- `gui-pin-window`: the GUI's Pin and Unpin, the live re-dock, the Queue and Full views and
  their widths, the mouse transitions between them, the covering rule, the docked edge, the
  pinned state at start, the live apply of the Panel settings while pinned, and the
  availability of the Pin control.

### Modified Capabilities

- `gui-client`: "The panel changes state with its own size" lets a pinned Queue view collapse
  to the rail. "The top bar shows the destination tabs and Search" makes a destination press in
  a pinned Mini window open the Full view.
- `gui-settings-screen`: "Settings with no effect on the GUI are hidden" shows the Panel
  settings that apply to a pinned GUI window. "The sidebar lists the sections in a fixed order"
  adds the Panel section.

## Impact

- `crates/mbv-gui`: the proxy start in `main.rs` before any thread spawns, the window re-create
  in `window/launch.rs`, a new `window/pin` module (the panel host seam, the views, the edge
  drag, the drawer rule), the header's Pin control, the `Panel` component and `QueueFooter`
  collapse rule, the Settings Panel section, and the `pinwin` dependency.
- `crates/mbv-client-core`: a neutral user-play counter that the GUI reads, the Settings scope
  of the Panel keys, a pure next-value function for a Panel row step, and `WAYLAND_SOCKET`
  removal on the Owner spawn.
- `crates/mbv-ui-model`: the `SettingScope` of the Panel keys.
- `crates/mbv-config`: `GuiLaunchState.pinned` and `GuiPanelWidths.full_view`.
- Packaging: `LICENSES/GPL-3.0.txt`, `PKGBUILD`, `PKGBUILD-git`, the `.deb` assets in
  `crates/mbv-tui/Cargo.toml`, the release tarball step and its check in
  `.github/workflows/build.yml`, and the `Makefile` install.
- Docs: a new ADR that amends ADR 0033 for the GUI, `CONTEXT.md` terms, and
  `crates/mbv-gui/AGENTS.md`.
- Runtime: the `pinwin` program is not needed by the GUI. A compositor without layer-shell, an
  X11 session, or a proxy start failure hides the Pin control.
