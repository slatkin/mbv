# Spec Delta

## Purpose

Defines mbv's pinned panel: `mbv --pin` runs mbv's TUI docked in a GTK layer-shell panel, the
desktop entry launches it that way, and the panel layout is editable in the settings screen. The
panel is part of mbv, not a separate program.

## ADDED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in the pinned panel in the same process, with no other
program started by the user and no re-launch of mbv. Without `--pin`, mbv SHALL start in the
current terminal exactly as before; no config setting starts the panel. `--pin` SHALL combine with
`--log-level` and with remote-client launches (`--connect-daemon` or `daemon_client_endpoint`).
`-h`, `-V`, `-q` and `--__local-daemon` SHALL behave as before and never start a panel. The panel
SHALL close when mbv exits. Each pinned launch SHALL open its own panel. A build without pinning SHALL reject `--pin` with an error message and
a non-zero exit.

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
`contrib/mbv.desktop` SHALL launch `mbv --pin` with `Terminal=false`. The `.deb`, whose binary is
built without pinning, SHALL NOT install a desktop entry.

#### Scenario: Launcher entry
- **WHEN** the tarball or an Arch package is installed
- **THEN** the mbv desktop entry runs `mbv --pin` without opening a terminal window

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`cols` (integer 1 through 65535, default 40) and `gutter_top`, `gutter_bottom`, `gutter_left`,
`gutter_right` (integers in pixels, may be negative, default 0). An out-of-range or malformed value
SHALL fall back to its default with a logged warning, without changing the other keys. In builds with pinning, the F2 settings
screen SHALL provide a Panel page with one row per value: `Side` cycles between left and right, and
the numeric rows step down and up by 1, or by 10 with Shift, within their ranges. Layout semantics
(docking, reservation, negative gutters, validation) are those of the `pinwin-panel` capability.

#### Scenario: Live change while pinned
- **WHEN** mbv runs in the panel and the user steps the width in F2's Panel page
- **THEN** the panel resizes without restarting mbv, and the value is saved

#### Scenario: Change while not pinned
- **WHEN** mbv runs in a terminal and the user changes a Panel row
- **THEN** the value is saved and used the next time `mbv --pin` starts

#### Scenario: Rejected layout while pinned
- **WHEN** mbv runs in the panel and a step would leave the panel's output with no width
- **THEN** a warning toast names the reason, the row keeps its previous value, and neither the panel nor `config.toml` changes

### Requirement: GTK stays out of the daemon crates
GTK, gtk4-layer-shell and the pinwin library SHALL be linked only into the `mbv` TUI binary and only
when the `pinning` cargo feature is enabled. `mbv-core`, `mbv-config`, `mbv-daemon` and `mbvd` SHALL
NOT depend on them, directly or transitively. mbv built without the feature SHALL NOT require Zig
or any GUI toolkit to build or run.

#### Scenario: Daemon and .deb builds
- **WHEN** `mbvd` or the mbv `.deb` is built
- **THEN** no GTK, gtk4-layer-shell or Zig dependency is involved, and the `.deb`'s Depends name no GTK package

#### Scenario: Feature off
- **WHEN** mbv is built without the `pinning` feature
- **THEN** it builds and runs without Zig or GTK, shows no Panel page, and rejects `--pin`
