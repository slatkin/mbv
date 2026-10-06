# Spec Delta

## MODIFIED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in a pinned panel hosted by kitty's `kitten panel`. The
`mbv --pin` process SHALL start the panel with a second mbv process inside it and SHALL wait until
the panel closes. Without `--pin`, mbv SHALL start in the current terminal exactly as before. No
configuration key starts the panel. `--pin` SHALL combine with `--log-level`, which SHALL apply to
the mbv process in the panel. `-h`, `-V`, `-q` and `--__local-daemon` SHALL behave as before and
never start a panel. If the mbv process in the panel exits, the panel SHALL close. The `mbv --pin`
process SHALL then exit with the kitty exit status. Each pinned launch SHALL open its own panel.

#### Scenario: Pinned launch from the desktop
- **WHEN** the user starts mbv from its desktop entry on a Wayland session with layer-shell and kitty installed
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell and kitty installed
- **THEN** the TUI appears in the panel, and the terminal waits until mbv exits

#### Scenario: Plain launch
- **WHEN** the user types `mbv` in a terminal, including over ssh or inside tmux
- **THEN** mbv starts in that terminal as before, whatever `WAYLAND_DISPLAY` is

### Requirement: Panel start failure
A pre-start failure is one that mbv finds before it starts kitty: `WAYLAND_DISPLAY` unset,
`XDG_RUNTIME_DIR` unset, or `kitten` not on `PATH`. For a pre-start failure, mbv SHALL record the
reason in its log. If stdin is a terminal, mbv SHALL print one line naming the reason to stderr and
run in that terminal. If stdin is not a terminal, mbv SHALL send a desktop notification naming the
reason and exit with status 1. If kitty exits with a non-zero status before the panel mbv starts,
mbv SHALL log the status and exit with status 1. If stdin is not a terminal, mbv SHALL also send a
desktop notification. A fatal start-up error in the mbv process inside the panel SHALL be logged
and sent as a desktop notification.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal and `kitten` is not on `PATH`
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel]` layout that leaves the output no width
- **THEN** mbv does not check the layout before it starts kitty, and the panel opens with the size the compositor gives it

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME, so kitty cannot open a layer-shell panel
- **THEN** a notification says the panel did not open, and mbv exits with status 1

#### Scenario: Desktop launch without kitty
- **WHEN** the desktop entry is launched and `kitten` is not on `PATH`
- **THEN** a notification says the panel did not open, and mbv exits with status 1

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold these keys:

- `side`: `"left"` or `"right"`, default `"left"`.
- `cols`: integer 1 through 65535, default 40.
- `cols_expanded`: integer 1 through 65535, default 120.
- `gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right`: integers in pixels, negative allowed, default 0.
- `cover`: boolean, default `false`.

An out-of-range or malformed value SHALL fall back to its default with a logged warning, without
changing the other keys. mbv SHALL ignore the keys `accent`, `accent_color` and `accent_width` on
read and SHALL NOT write them. The F2 screen SHALL provide a Panel page with one row per key except
`cover`. `Side` cycles between left and right. The numeric rows step down and up by 1, or by 10 with
Shift, within their ranges. mbv reads `cover` from `config.toml` only. The panel SHALL dock to
`side`, span the output height less the top and bottom gutters, and be `cols` or `cols_expanded`
cells wide. In push mode the compositor SHALL reserve the docking-edge gutter, the panel width and
the far gutter.

While pinned, a change to `Cols` SHALL switch the panel to its collapsed width. A change to
`Expanded cols` SHALL switch it to its expanded width. A change to a side or gutter row SHALL apply
at the current panel width. If kitty rejects a change, a warning toast SHALL name the reason and
the row SHALL keep its previous value.

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
- **WHEN** mbv runs in the panel and kitty remote control returns an error for a step
- **THEN** a warning toast names the reason, the row keeps its previous value, and `config.toml` does not change

#### Scenario: Malformed accent colour
- **WHEN** `config.toml` sets `accent_color = "orange"`
- **THEN** mbv logs no warning for that key, and the other `[panel]` keys keep their values

#### Scenario: Custom colour survives cycling
- **WHEN** `config.toml` sets `accent_color = "#123456"` and mbv saves its configuration
- **THEN** the saved `[panel]` section has no `accent_color` key, and the other `[panel]` keys keep their values

#### Scenario: Malformed cover value
- **WHEN** `config.toml` sets `cover = "yes"`
- **THEN** `cover` falls back to `false` with a logged warning, and the other `[panel]` keys keep their values

### Requirement: Pinned panel width toggle
mbv SHALL provide a configurable keybind action, `pinned_width_toggle` (default `Ctrl+e`, Global
section). It switches the running pinned panel between its collapsed width (`cols`) and its
expanded width (`cols_expanded`). It keeps `side` and the gutters. The switch SHALL resize the
running panel in one step without restarting mbv. The space reserved beside tiled windows SHALL
follow the new width. mbv SHALL NOT save which width is active, and every pinned launch SHALL start
at the collapsed width. If kitty rejects the other width, a warning toast SHALL name the reason and
the panel SHALL stay at its current width. When mbv is not running in a pinned panel, the action
SHALL show a neutral toast saying it needs a pinned launch and change nothing. The width toggle
SHALL change only the width. It SHALL leave the stored panel mode exactly as it was, so the width
toggle and the pinned view toggle (`panel-mode`) are independent.

#### Scenario: Expand
- **WHEN** mbv runs in the panel at its collapsed width and the user presses `Ctrl+e`
- **THEN** the panel resizes to `cols_expanded` columns and tiled windows reflow beside it

#### Scenario: Collapse
- **WHEN** mbv runs in the panel at its expanded width and the user presses `Ctrl+e`
- **THEN** the panel resizes to `cols` columns

#### Scenario: Launch starts collapsed
- **WHEN** the user quits mbv while expanded and starts `mbv --pin` again
- **THEN** the panel opens at `cols` columns

#### Scenario: Expanded width does not fit
- **WHEN** the user presses `Ctrl+e` and kitty remote control returns an error
- **THEN** a warning toast names the reason and the panel keeps its collapsed width

#### Scenario: Not pinned
- **WHEN** mbv runs in a terminal and the user presses `Ctrl+e`
- **THEN** a neutral toast says the toggle needs a pinned launch, and nothing else changes

#### Scenario: Width toggle leaves the panel mode alone
- **WHEN** mbv runs in the pinned panel showing library-only and the user presses `Ctrl+e` to collapse
- **THEN** the panel collapses to `cols` columns and the stored panel mode is unchanged. The shown
  panel follows the usual width-driven derivation.

### Requirement: Panel covering mode
`mbv --pin` SHALL open the panel in covering mode when `[panel] cover` is true. The panel then draws
over the tiled windows and the compositor reserves no space beside it. When `cover` is false, the
default, the compositor SHALL reserve a strip and tiled windows move aside. At panel start and at
every layout change, mbv SHALL apply the choice. mbv SHALL provide no runtime toggle for it.

#### Scenario: Default pushes
- **WHEN** `mbv --pin` starts with no `cover` key in `config.toml`
- **THEN** the compositor reserves a strip beside the panel and tiled windows move aside

#### Scenario: Covering draws over tiles
- **WHEN** `mbv --pin` starts with `cover = true`
- **THEN** the panel draws over the tiled windows and the compositor reserves no strip

#### Scenario: Cover value survives a save
- **WHEN** mbv saves its configuration while `cover` is `true`
- **THEN** the saved `[panel]` section keeps `cover = true`

## ADDED Requirements

### Requirement: Remote control stays private
The remote-control socket of the pinned panel SHALL be a file in `$XDG_RUNTIME_DIR` with a name
unique to the launch. It SHALL NOT be an abstract socket. At start, the mbv process in the panel SHALL
remove the pinned-launch environment variables from its own environment. Processes that it starts
then do not inherit them.

#### Scenario: Socket location
- **WHEN** `mbv --pin` starts the panel
- **THEN** the panel listens on a socket file under `$XDG_RUNTIME_DIR`, and no abstract socket is opened

#### Scenario: Owner process environment
- **WHEN** the mbv process in the panel starts the Owner process
- **THEN** the Owner process environment holds neither `MBV_PINNED` nor `KITTY_LISTEN_ON`

## REMOVED Requirements

### Requirement: GTK stays out of the daemon crates
**Reason**: mbv no longer links GTK, gtk4-layer-shell or pinwin in any crate, so the requirement has nothing to constrain.
**Migration**: None. `mbvd` builds as before, and the `mbv` binary no longer needs GTK or Zig.

### Requirement: Focus accent applies at launch
**Reason**: kitty draws no border around a single-window panel, so mbv has no accent to pass to it.
**Migration**: Remove `accent`, `accent_color` and `accent_width` from `[panel]`. mbv ignores them and drops them on the next save. The kitty `kitty.conf` controls the panel look.
