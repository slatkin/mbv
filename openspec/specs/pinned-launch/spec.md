# pinned-launch Specification

## Purpose

Defines mbv's pinned panel: `mbv --pin` runs mbv's TUI docked in a GTK layer-shell panel, the
desktop entry launches it that way, and the panel layout is editable in the settings screen. The
panel is part of mbv, not a separate program.

## Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in the pinned panel in the same process, with no other
program started by the user and no re-launch of mbv. Without `--pin`, mbv SHALL start in the
current terminal exactly as before; no config setting starts the panel. `--pin` SHALL combine with
`--log-level`.
`-h`, `-V`, `-q` and `--__local-daemon` SHALL behave as before and never start a panel. The panel
SHALL close when mbv exits. Each pinned launch SHALL open its own panel.

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
`cols` (integer 1 through 65535, default 40), `cols_expanded` (integer 1 through 65535, default 120),
`gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right` (integers in pixels, may be negative,
default 0), `accent` (boolean, default `true`), `accent_color` (`"#RRGGBB"` or `"RRGGBB"`, default
`"#dabc7f"`), `accent_width` (integer pixels 1 through 65535, default 1) and `cover` (boolean,
default `false`). An out-of-range or malformed value SHALL fall back to its default with a logged
warning, without changing the other keys. The F2 settings screen SHALL provide a Panel page with
one row per value except `cover`: `Side` cycles between left and right, `Accent` toggles on and
off, `Accent color` cycles through a fixed colour list that always includes the configured value,
and the numeric rows step down and up by 1, or by 10 with Shift, within their ranges. `cover` is
read from `config.toml` only. Layout semantics (docking, reservation, covering, negative gutters,
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
- **WHEN** `config.toml` sets `cover = "yes"`
- **THEN** `cover` falls back to `false` with a logged warning, and the other `[panel]` keys keep their values

### Requirement: GTK stays out of the daemon crates
GTK, gtk4-layer-shell and the pinwin library SHALL be linked only into the `mbv` TUI binary.
`mbv-core`, `mbv-config`, `mbv-daemon` and `mbvd` SHALL NOT depend on them, directly or
transitively.

#### Scenario: Daemon build
- **WHEN** `mbvd` is built
- **THEN** no GTK, gtk4-layer-shell or Zig dependency is involved

### Requirement: Pinned panel width toggle
mbv SHALL provide a configurable keybind action, `pinned_width_toggle` (default `Ctrl+e`, Global
section), that switches the running pinned panel between its collapsed width (`cols`) and its
expanded width (`cols_expanded`), keeping `side` and the gutters. The switch SHALL resize the
running panel without restarting mbv, and the space reserved beside tiled windows SHALL follow the
new width. Which width is active SHALL NOT be saved: every pinned launch SHALL start at the
collapsed width. If the panel rejects the other width, a warning toast SHALL name the reason and
the panel SHALL stay at its current width. When mbv is not running in a pinned panel, the action
SHALL show a neutral toast saying it needs a pinned launch and change nothing. The width toggle
SHALL change only the width: the stored panel mode SHALL be left exactly as it was, so the
width toggle and the pinned view toggle (`panel-mode`) are independent.

The panel SHALL animate between the two widths rather than snapping. A layout change that does
not alter the width SHALL apply in one step.

#### Scenario: Expand
- **WHEN** mbv runs in the panel at its collapsed width and the user presses `Ctrl+e`
- **THEN** the panel animates to `cols_expanded` columns and tiled windows reflow beside it

#### Scenario: Collapse
- **WHEN** mbv runs in the panel at its expanded width and the user presses `Ctrl+e`
- **THEN** the panel resizes to `cols` columns

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

### Requirement: Panel covering mode
`mbv --pin` SHALL build the panel layout in pinwin's covering mode when `[panel] cover` is true:
the panel draws over the tiled windows and the compositor reserves no space beside it. When
`cover` is false, the default, the layout SHALL use pinwin's pushing mode, so the compositor
reserves a strip and tiled windows move aside. The choice SHALL be made where the layout is
built; mbv SHALL provide no runtime toggle for it.

#### Scenario: Default pushes
- **WHEN** `mbv --pin` starts with no `cover` key in `config.toml`
- **THEN** the compositor reserves a strip beside the panel and tiled windows move aside

#### Scenario: Covering draws over tiles
- **WHEN** `mbv --pin` starts with `cover = true`
- **THEN** the panel draws over the tiled windows and the compositor reserves no strip

#### Scenario: Cover value survives a save
- **WHEN** mbv saves its settings while `cover` is `true`
- **THEN** the saved `[panel]` section keeps `cover = true`
