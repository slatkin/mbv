# pinned-launch Specification

## Purpose

Defines mbv's pinned panel: `mbv --pin` runs mbv's TUI docked in a Wayland layer-shell panel, the
desktop entry launches it that way, and the panel layout is editable in the settings screen. The
panel is part of mbv, not a separate program.

## Requirements

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

### Requirement: Desktop entry
`contrib/mbv.desktop` SHALL launch `mbv --pin` with `Terminal=false`.

#### Scenario: Launcher entry
- **WHEN** the tarball, the `.deb` or an Arch package is installed
- **THEN** the mbv desktop entry runs `mbv --pin` without opening a terminal window

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`cols` (integer 1 through 65535, default 40), `cols_expanded` (integer 1 through 65535, default 120),
`gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right` (integers in pixels, may be negative,
default 0), `accent` (boolean, default `true`), `accent_color` (`"#RRGGBB"` or `"RRGGBB"`, default
`"#dabc7f"`) and `accent_width` (integer pixels 1 through 65535, default 1). An out-of-range or
malformed value SHALL fall back to its default with a logged warning, without changing the other
keys. A `cover` key left in the section by an earlier version SHALL be ignored without a warning.
The F2 settings screen SHALL provide a Panel page with one row per value: `Side` cycles between
left and right, `Accent` toggles on and off, `Accent color` cycles through a fixed colour list that
always includes the configured value, and the numeric rows step down and up by 1, or by 10 with
Shift, within their ranges. Layout semantics (docking, reservation, covering, negative gutters,
validation) are those of the `pinwin-panel` capability.

While pinned, a change to `Cols` SHALL switch the panel to its collapsed width and a change to
`Expanded cols` SHALL switch it to its expanded width, so the edited value is the width the panel
validates; a change to a side or gutter row SHALL apply at the panel's current width. A change to
an accent row SHALL NOT touch the running panel.

#### Scenario: Live change while pinned
- **WHEN** mbv runs in the panel and the user steps the width in F2's Panel page
- **THEN** the panel resizes without restarting mbv, and the value is saved

#### Scenario: Editing the expanded width while collapsed
- **WHEN** mbv runs in the panel at its collapsed width and the user steps `Expanded cols`
- **THEN** the panel switches to its expanded width using the new value, and the value is saved

#### Scenario: Gutter change while expanded
- **WHEN** mbv runs in the panel at its expanded width and the user steps a gutter
- **THEN** the panel stays at its expanded width with the new gutter, and the gutter is saved

#### Scenario: Change while not pinned
- **WHEN** mbv runs in a terminal and the user changes a Panel row
- **THEN** the value is saved and used the next time `mbv --pin` starts

#### Scenario: Rejected layout while pinned
- **WHEN** mbv runs in the panel and a step would leave the panel's output with no width
- **THEN** a warning toast names the reason, the row keeps its previous value, and neither the panel nor `config.toml` changes

#### Scenario: Malformed accent colour
- **WHEN** `config.toml` sets `accent_color = "orange"`
- **THEN** the colour falls back to `#dabc7f` with a logged warning, and the other `[panel]` keys keep their values

#### Scenario: Custom colour survives cycling
- **WHEN** `accent_color` is `#123456`, which is not in the fixed list, and the user opens the `Accent color` row
- **THEN** `#123456` is one of the values the row cycles through

#### Scenario: Malformed cover value
- **WHEN** `config.toml` still sets `cover = "yes"` or `cover = true` under `[panel]`
- **THEN** the key is ignored without a warning, and the other `[panel]` keys keep their values

### Requirement: Panel libraries stay out of the daemon crates
The pinwin library and its Wayland, xkbcommon and fontconfig stack SHALL be linked only into the
`mbv` TUI binary. `mbv-core`, `mbv-config`, `mbv-daemon` and `mbvd` SHALL NOT depend on them,
directly or transitively. No mbv binary SHALL link GTK or gtk4-layer-shell.

#### Scenario: Daemon build
- **WHEN** `mbvd` is built
- **THEN** no pinwin, Wayland client, xkbcommon, fontconfig or Zig dependency is involved

### Requirement: Pinned panel width toggle
mbv SHALL provide a configurable keybind action, `pinned_width_toggle` (default `Ctrl+e`, Global
section), that switches the running pinned panel between its collapsed width (`cols`) and its
expanded width (`cols_expanded`), keeping `side` and the gutters. The switch SHALL resize the
running panel without restarting mbv. Expanding SHALL draw the panel over the tiled windows
without moving them, and collapsing SHALL return to the reserved strip without moving them (see
Panel covering mode). Which width is active SHALL NOT be saved: every pinned launch SHALL start at
the collapsed width. If the panel rejects the other width, a warning toast SHALL name the reason and
the panel SHALL stay at its current width. When mbv is not running in a pinned panel, the action
SHALL show a neutral toast saying it needs a pinned launch and change nothing. The width toggle
SHALL change only the width: the stored panel mode SHALL be left exactly as it was, so the
width toggle and the pinned view toggle (`panel-mode`) are independent.

The panel SHALL animate between the two widths rather than snapping. A layout change that does
not alter the width SHALL apply in one step.

#### Scenario: Expand
- **WHEN** mbv runs in the panel at its collapsed width and the user presses `Ctrl+e`
- **THEN** the panel animates to `cols_expanded` columns over the tiled windows, and no tiled window moves

#### Scenario: Collapse
- **WHEN** mbv runs in the panel at its expanded width and the user presses `Ctrl+e`
- **THEN** the panel resizes to `cols` columns, and no tiled window moves

#### Scenario: Launch starts collapsed
- **WHEN** the user quits mbv while expanded and starts `mbv --pin` again
- **THEN** the panel opens at `cols` columns

#### Scenario: Expanded width does not fit
- **WHEN** the user presses `Ctrl+e` and `cols_expanded` would leave the output no width
- **THEN** a warning toast names the reason and the panel keeps its collapsed width

#### Scenario: Not pinned
- **WHEN** mbv runs in a terminal and the user presses `Ctrl+e`
- **THEN** a neutral toast says the toggle needs a pinned launch, and nothing else changes

#### Scenario: Width toggle leaves the panel mode alone
- **WHEN** mbv runs in the pinned panel showing library-only and the user presses `Ctrl+e` to collapse
- **THEN** the panel collapses to `cols` columns and the stored panel mode is unchanged; the shown
  panel follows the usual width-driven derivation

### Requirement: Focus accent applies at launch
`mbv --pin` SHALL start the panel with the focus accent from `[panel]`: on with `accent_color` and
`accent_width` when `accent` is true, off otherwise. Because the accent is fixed when the panel
starts, a change to an accent row while pinned SHALL be saved and announced with a neutral toast
saying it applies on the next `mbv --pin` launch.

#### Scenario: Default accent
- **WHEN** `mbv --pin` starts with no accent keys in `config.toml` and the user clicks into the panel
- **THEN** a 1 px `#dabc7f` stroke appears around the panel's border while it holds focus

#### Scenario: Accent off
- **WHEN** `accent = false` and the user clicks into the panel
- **THEN** no accent stroke is drawn

#### Scenario: Accent edit while pinned
- **WHEN** mbv runs in the panel and the user toggles `Accent` in F2
- **THEN** the value is saved, the panel is unchanged, and a neutral toast says the change applies on the next `mbv --pin` launch

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

### Requirement: Panel width decides pushing or covering
`mbv --pin` SHALL build the collapsed panel layout in pushing mode and the expanded panel layout
in covering mode, with no setting for either. A pushing panel makes the compositor reserve a strip
beside it, so tiled windows move aside. A covering panel draws over the tiled windows and holds
the strip already reserved, so expanding and collapsing move no tiled window. mbv SHALL provide no
runtime toggle for the choice.

#### Scenario: Collapsed panel pushes
- **WHEN** `mbv --pin` starts
- **THEN** the compositor reserves a strip beside the panel and tiled windows move aside

#### Scenario: Expanding leaves tiles in place
- **WHEN** mbv runs in the panel at its collapsed width and the user expands it
- **THEN** the panel draws over the tiled windows and no tiled window moves

#### Scenario: Collapsing leaves tiles in place
- **WHEN** mbv runs in the panel at its expanded width and the user collapses it
- **THEN** the panel shrinks to the collapsed strip and no tiled window moves

### Requirement: Hidden panel follows its width
A hidden collapsed panel SHALL release its reserved strip, and showing it SHALL take the strip
back. Hiding or showing an expanded panel SHALL move no tiled window.

#### Scenario: Hiding a collapsed panel
- **WHEN** the panel runs at its collapsed width and the user hides it
- **THEN** the reserved strip is released and tiled windows take the space back

#### Scenario: Hiding an expanded panel
- **WHEN** the panel runs at its expanded width and the user hides or shows it
- **THEN** no tiled window moves
