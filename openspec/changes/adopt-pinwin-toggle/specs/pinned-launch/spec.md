# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: GTK stays out of the daemon crates`
- TO: `### Requirement: Panel libraries stay out of the daemon crates`

## MODIFIED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in the pinned panel in the same process, with no other
program started by the user and no re-launch of mbv. Without `--pin`, mbv SHALL start in the
current terminal exactly as before; no config setting starts the panel. `--pin` SHALL combine with
`--log-level`.
`-h`, `-V`, `-q`, `--toggle` and `--__local-daemon` SHALL never start a panel. The panel
SHALL close when mbv exits. At most one pinned panel SHALL run per Wayland display. When a pinned
mbv is already running on the display, `--pin` SHALL ask that panel to show, leave a shown panel
unchanged, open no second panel and start no TUI, and exit with status 0.

#### Scenario: Pinned launch from the desktop
- **WHEN** the user starts mbv from its desktop entry on a Wayland session with layer-shell
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell
- **THEN** the TUI appears in the panel, and the terminal waits until mbv exits

#### Scenario: Plain launch
- **WHEN** the user types `mbv` in a terminal, including over ssh or inside tmux
- **THEN** mbv starts in that terminal as before, whatever `WAYLAND_DISPLAY` is

#### Scenario: Second pinned launch while hidden
- **WHEN** a pinned mbv is running with its panel hidden and the user starts `mbv --pin` again
- **THEN** the running panel is shown, and the second mbv exits with status 0 without opening a panel

#### Scenario: Second pinned launch while shown
- **WHEN** a pinned mbv is running with its panel shown and the user starts `mbv --pin` again
- **THEN** the running panel is unchanged, and the second mbv exits with status 0

### Requirement: Panel start failure
When `--pin` is given and the panel cannot start (`WAYLAND_DISPLAY` unset, no layer-shell, the
instance socket cannot be bound, a saved layout that does not fit the output, the terminal
hand-over failing, or a running pinned mbv not answering the show request), mbv SHALL record the
reason in its log. If stdin is a terminal, mbv SHALL print one line naming the reason to stderr and
run in that terminal. If stdin is not a terminal, mbv SHALL send a desktop notification naming the
reason and exit with status 1. A fatal start-up error after the panel has opened SHALL be logged
and sent as a desktop notification.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal on a compositor without layer-shell
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel]` layout that leaves the output no width
- **THEN** the panel does not open, and mbv takes the same terminal or notification path

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME or X11
- **THEN** a notification says the panel could not open, and mbv exits with status 1

### Requirement: Panel libraries stay out of the daemon crates
The pinwin library and its Wayland, xkbcommon and fontconfig stack SHALL be linked only into the
`mbv` TUI binary. `mbv-core`, `mbv-config`, `mbv-daemon` and `mbvd` SHALL NOT depend on them,
directly or transitively. No mbv binary SHALL link GTK or gtk4-layer-shell.

#### Scenario: Daemon build
- **WHEN** `mbvd` is built
- **THEN** no pinwin, Wayland client, xkbcommon, fontconfig or Zig dependency is involved

### Requirement: Panel covering mode
`mbv --pin` SHALL build the panel layout in pinwin's covering mode when `[panel] cover` is true:
the panel draws over the tiled windows and the compositor reserves no space beside it. When
`cover` is false, the default, the layout SHALL use pinwin's pushing mode, so the compositor
reserves a strip and tiled windows move aside. The choice SHALL be made where the layout is
built; mbv SHALL provide no runtime toggle for it. The same setting SHALL govern a hidden panel,
with no setting of its own: a hidden pushing panel releases its strip and a shown one takes it
back, while hiding or showing a covering panel moves no tiled window.

#### Scenario: Default pushes
- **WHEN** `mbv --pin` starts with no `cover` key in `config.toml`
- **THEN** the compositor reserves a strip beside the panel and tiled windows move aside

#### Scenario: Covering draws over tiles
- **WHEN** `mbv --pin` starts with `cover = true`
- **THEN** the panel draws over the tiled windows and the compositor reserves no strip

#### Scenario: Cover value survives a save
- **WHEN** mbv saves its settings while `cover` is `true`
- **THEN** the saved `[panel]` section keeps `cover = true`

#### Scenario: Hiding a pushing panel
- **WHEN** the panel runs with `cover` false and the user hides it
- **THEN** the reserved strip is released and tiled windows take the space back

#### Scenario: Hiding a covering panel
- **WHEN** the panel runs with `cover = true` and the user hides or shows it
- **THEN** no tiled window moves

## ADDED Requirements

### Requirement: Panel toggle
mbv SHALL accept `--toggle`, which asks the pinned mbv running on the current Wayland display to
hide its panel if shown, or to show it if hidden, and exits with status 0. It SHALL NOT start
mbv, a TUI or a panel. When no pinned mbv is running or the request fails, it SHALL report why
(stderr on a terminal, otherwise a desktop notification) and exit with status 1. Hiding and
showing SHALL NOT resize the TUI or change its width state.

#### Scenario: Hide from a compositor key
- **WHEN** the user presses a compositor key bound to `mbv --toggle` while the pinned panel is shown
- **THEN** the panel hides, playback continues, and the `mbv --toggle` process exits with status 0

#### Scenario: Show from a compositor key
- **WHEN** the user presses the same key while the panel is hidden
- **THEN** the panel shows at the same width with the TUI as it was, and takes keyboard focus on niri

#### Scenario: Nothing pinned
- **WHEN** the user runs `mbv --toggle` in a terminal with no pinned mbv running
- **THEN** one line says no pinned mbv is running, and the process exits with status 1

#### Scenario: Nothing pinned from a key
- **WHEN** a compositor key runs `mbv --toggle` with no pinned mbv running
- **THEN** a desktop notification says no pinned mbv is running, and no mbv starts
