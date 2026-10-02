# Spec Delta

## Purpose

Defines mbv's pinned panel: `mbv --pin` runs mbv's TUI docked in a GTK layer-shell panel, the
desktop entry launches it that way, and the panel's size and side are changed from mbv's system
tray. The panel is part of mbv, not a separate program.

## ADDED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in the pinned panel in the same process, with no other
program started by the user and no re-launch of mbv. Without `--pin`, mbv SHALL start in the
current terminal exactly as before; no config setting starts the panel. `--pin` SHALL combine with
`--log-level`.
`-h`, `-V`, `-q` and `--__local-daemon` SHALL behave as before and never start a panel. The panel
SHALL close when mbv exits.

#### Scenario: Pinned launch from the desktop
- **WHEN** the user starts mbv from its desktop entry on a Wayland session with layer-shell
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell
- **THEN** the TUI appears in the panel, and the terminal waits until mbv exits

#### Scenario: Plain launch
- **WHEN** the user types `mbv` in a terminal, including over ssh or inside tmux
- **THEN** mbv starts in that terminal as before, whatever `WAYLAND_DISPLAY` is

### Requirement: Panel start failure
When `--pin` is given and the panel cannot start (`WAYLAND_DISPLAY` unset, no layer-shell, GTK
init failure, a saved layout that does not fit the output, or the terminal hand-over failing), mbv
SHALL record the reason in its log. If stdin
is a terminal, mbv SHALL print one line naming the reason to stderr and run in that terminal. If
stdin is not a terminal, mbv SHALL send a desktop notification naming the reason and exit with
status 1. A fatal start-up error after the panel has opened SHALL be logged and sent as a desktop
notification.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal on a compositor without layer-shell
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel]` layout that leaves the output no width
- **THEN** the panel does not open, and mbv takes the same terminal or notification path

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME or X11
- **THEN** a notification says the panel could not open, and mbv exits with status 1

### Requirement: Desktop entry
`contrib/mbv.desktop` SHALL launch `mbv --pin` with `Terminal=false`.

#### Scenario: Launcher entry
- **WHEN** the tarball, the `.deb` or an Arch package is installed
- **THEN** the mbv desktop entry runs `mbv --pin` without opening a terminal window

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`cols` (integer 1 through 65535, default 40) and `gutter_top`, `gutter_bottom`, `gutter_left`,
`gutter_right` (integers in pixels, may be negative, default 0). An out-of-range or malformed value
SHALL fall back to its default with a logged warning, without changing the other keys. The TUI
SHALL NOT present these values; they are changed only from the tray (below) or by editing
`config.toml`. Layout semantics (docking, reservation, negative gutters, validation) are those of
the `pinwin-panel` capability.

#### Scenario: Invalid saved value
- **WHEN** `config.toml` has `cols = 0` in `[panel]`
- **THEN** mbv logs a warning and uses 40 columns, keeping the other `[panel]` values

#### Scenario: No panel settings in the TUI
- **WHEN** the user opens the F2 settings screen
- **THEN** it has no Panel page

### Requirement: One pinned mbv at a time
At most one pinned mbv SHALL run per user. A `--pin` launch while one is pinned SHALL NOT open a
panel: if stdin is a terminal, mbv SHALL print `mbv: a pinned mbv is already running` and exit with
status 1; otherwise it SHALL exit with status 0 and no notification. Unpinned mbv launches SHALL
NOT be limited by this.

#### Scenario: Second pinned launch from the desktop
- **WHEN** a pinned mbv is running and the user starts the desktop entry again
- **THEN** nothing visible happens and the existing panel is unchanged

#### Scenario: Second pinned launch from a terminal
- **WHEN** a pinned mbv is running and the user types `mbv --pin`
- **THEN** one line says a pinned mbv is already running, and mbv exits with status 1

### Requirement: Panel options in the tray
mbv's system tray SHALL always show a `Resize` item, greyed out while no pinned mbv runs or while
the form is open; a pinned mbv starts the tray if Stay-alive is off. Choosing it SHALL open a small
form showing the current side, columns and four gutters. Edits SHALL take effect only on `Apply`,
which SHALL change the panel live and save the values to `config.toml`. Closing the form without
`Apply` SHALL discard the edits. A layout the panel rejects SHALL be reported in the form, leaving the panel, `config.toml` and the
edits unchanged. The tray's other items SHALL
behave as before.

#### Scenario: Resize from the tray
- **WHEN** a pinned mbv is running and the user opens `Resize`, sets `Columns` to 60 and presses `Apply`
- **THEN** the panel becomes 60 columns wide without restarting mbv, and 60 is saved

#### Scenario: Close without Apply
- **WHEN** the user edits a value and closes the form without `Apply`
- **THEN** the panel and `config.toml` are unchanged, and reopening the form shows the saved values

#### Scenario: Rejected layout
- **WHEN** the user applies a layout that leaves the panel's output no width
- **THEN** the form shows the reason and keeps the edits, and the panel and `config.toml` are unchanged

#### Scenario: No pinned mbv
- **WHEN** only unpinned mbv terminals are running
- **THEN** the tray's `Resize` item is greyed out

#### Scenario: Pinned mbv without Stay-alive
- **WHEN** Stay-alive is off and a pinned mbv starts
- **THEN** the tray appears with its usual items and `Resize`

### Requirement: GTK stays out of the daemon crates
GTK, gtk4-layer-shell and the pinwin library SHALL be linked only into the `mbv` TUI binary.
`mbv-core`, `mbv-config`, `mbv-daemon` and `mbvd` SHALL NOT depend on them, directly or
transitively.

#### Scenario: Daemon build
- **WHEN** `mbvd` is built
- **THEN** no GTK, gtk4-layer-shell or Zig dependency is involved
