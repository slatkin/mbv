---
status: accepted
---

# pinwin Is an External App Host

## Context

mbv linked the pinwin 0.3.4 crate. A `--pin` launch called `pinwin::Panel`,
opened a pty, and moved its own stdio onto the panel's pty. `App` held the
`Panel` handle and called `apply_layout_animated` for `x`, `Ctrl+e`, and the
F2 Panel rows. An instance socket served `--toggle` and the duplicate `--pin`.

pinwin 0.4 changes from a terminal library into a layer-shell app host for
any Wayland program. It removes the library API, the pty hand-over, and the
instance socket, so the 0.3.4 link cannot move forward. pinwin 0.4.1 adds
what mbv needs to drive its own panel: `PINWIN_PANEL` and `PINWIN_SOCKET` in
the app environment, size and reserve mode in one request,
`pinwin run --show-existing`, and gutters as run options and as a live
`set-gutters` command.

The full design and its alternatives are in
`openspec/changes/pinwin-app-host/design.md` (decisions D1 to D4).

## Decision

pinwin is an external program that mbv runs. mbv links no panel library, and
`src/pin.rs` is the only pinwin boundary.

### D1. A terminal hosts the TUI in the panel

`--pin` runs `pinwin run … -- <terminal argv> <mbv exe>`. The terminal argv
comes from the `[panel] pin_terminal` prefix, else `TERMINAL` followed by
`-e`, else the launch fails with a reason that names both. The terminal must
open its window in the process that `pinwin run` starts. That is why
`pin_terminal` is separate from the Unpin `[panel] terminal` prefix, which can
give the launch to a running copy. The mbv executable follows the existing
`launch_exe` rule, so the launcher's own build runs even when the file on disk
was replaced.

### D2. `--pin` spawns `pinwin run` and waits for it

The launcher first checks what needs no Wayland code: `WAYLAND_DISPLAY` is
set, a terminal argv resolves, and `pinwin` starts. A failure there takes the
start-failure path: fall back to the current terminal when stdin is a
terminal, else notify and exit non-zero.

Then it spawns `pinwin run` as a child, waits, and exits with the child's
status. A non-zero status is logged, and reported with a notification when
stdin is not a terminal, because a desktop launch has no stderr to read. After
the spawn there is no terminal fallback: the status cannot tell a pinwin
failure from a terminal that ended with an error after a long session.

### D3. Pinned means `PINWIN_PANEL` is set

At start, on the main thread and before any thread exists, mbv reads
`PINWIN_PANEL` and `PINWIN_SOCKET` into a `OnceLock` and removes both from its
environment, the same way it claims `MBV_SWAP_TOKEN`. A non-empty
`PINWIN_PANEL` means pinned, and its value is the panel name for every
request.

The Owner process, mpv, and every other child therefore do not inherit the
panel identity. A stay-alive Owner cannot think that it is pinned. A terminal
that gives the launch to a running copy starts mbv without the variable, so
mbv runs as an unpinned Client, which matches what the user sees. The
`HANDED_OVER` flag, the pty code, `PanelEnv`, and the stderr redirect go away.

The `--pin` launcher sets `MBV_SWAP_TOKEN` on the `pinwin run` child from the
claimed token, so the Pin swap reaches the pinned Client.

### D4. Panel requests run the `pinwin` CLI

Each request is one synchronous `pinwin <command>` child with
`PINWIN_SOCKET` set from D3. Exit 0 is success. Exit 1 gives pinwin's stderr
line as the Warning toast text. Exit 2 or a spawn failure gives a fixed toast
that names the cause: usage mismatch or `pinwin` not installed. The argv
builders are pure functions with unit tests. The process call is the mocked
boundary.

## Considered options

- **Keep the pinwin 0.3.4 library link (rejected).** 0.4 removes the API,
  and 0.3.x gets no more work.
- **A native ratatui Wayland backend, or the 0.3.4 terminal stack moved into
  mbv (rejected).** Both bring a font, raster, and input stack into mbv, which
  pinwin itself just dropped.
- **`exec` into `pinwin run` (rejected).** It saves one idle process, but a
  desktop launch on a compositor without layer-shell then fails with no
  notification.
- **Write JSON lines to the daemon socket instead of the CLI (rejected).** It
  gives typed error kinds that mbv only shows as text, and it copies pinwin's
  wire format into mbv with no compiler check.

## Consequences

- mbv no longer links pinwin. The Wayland, font, and raster stack leaves the
  build.
- The `pinwin` program, 0.4.1 or later, becomes an optional runtime
  dependency. A missing program fails the launcher with a reason, and the
  start-failure paths still notify a desktop launch.
- pinwin CLI changes break mbv at runtime with no compiler check. One minimum
  version in `PKGBUILD` and `AGENTS.md`, argv builders that keep every flag in
  one file, and a manual pinned check at each pinwin release limit that risk.
- There is one panel per `--pin` launch, and it ends with that launch. A
  second `--pin` shows the running panel through `--show-existing`.
- Related decisions: [ADR 0030](0030-owner-process-is-the-only-local-player-owner.md)
  keeps the Owner process as the only local Player owner. The pinned launch
  selects where the TUI draws, not who owns the Player.
