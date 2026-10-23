# pinned-launch Specification

## Purpose

Defines mbv's pinned panel: `mbv --pin` runs mbv's TUI docked in a Wayland layer-shell panel, the
desktop entry launches it that way, and the panel layout is editable in the settings screen. The
panel is part of mbv, not a separate program.

## Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in a pinwin panel named `mbv`: pinwin docks the pin
terminal as the panel, and that terminal runs mbv with the hidden `--in-panel` flag. The process
that ran `--pin` SHALL wait until the panel's terminal ends. Without `--pin`, mbv SHALL start in the current terminal exactly as
before; no config setting starts the panel. `--pin` SHALL combine with `--log-level`, and the mbv
in the panel SHALL run with that log level. `-h`, `-V`, `-q`, `--toggle` and `--__local-daemon`
SHALL never start a panel. The panel SHALL close when mbv in it exits. At most one panel named
`mbv` SHALL run per Wayland display. When it already runs, `--pin` SHALL show it, leave a shown
panel unchanged, open no second panel, start no TUI, and exit with status 0.

#### Scenario: Pinned launch from the desktop
- **WHEN** the user starts mbv from its desktop entry on a Wayland session with layer-shell
- **THEN** mbv's TUI appears docked in the panel, and quitting mbv removes the panel

#### Scenario: Pinned launch from a terminal
- **WHEN** the user types `mbv --pin` in a terminal on a Wayland session with layer-shell
- **THEN** the TUI appears in the panel, and the terminal waits until the panel ends

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
When `--pin` is given and the panel cannot start before pinwin is run (`WAYLAND_DISPLAY` unset,
no pin terminal, `pinwin` not installed), mbv SHALL record the reason in its log. If stdin is a
terminal, mbv SHALL print one line naming the reason to stderr and run in that terminal. If stdin
is not a terminal, mbv SHALL send a desktop notification naming the reason and exit with status 1.
When pinwin was run and ends with a failure status (no layer-shell, an invalid pinwin config, a
layout that does not fit the output, a command that cannot run), mbv SHALL log the status, send a
desktop notification that says the panel could not open and names the status when stdin is not a
terminal, and exit with that status; it SHALL NOT then run in the terminal. A fatal start-up
error of the mbv in the panel SHALL be logged and sent as a desktop notification that names the
error.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal with `WAYLAND_DISPLAY` unset
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel] width` that leaves the output no width
- **THEN** the panel does not open, and mbv exits with pinwin's failure status

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME or X11
- **THEN** a notification says the panel could not open, the status stays in the log, and mbv exits with a non-zero status

#### Scenario: Fatal start-up error in the panel
- **WHEN** mbv in the pinned panel hits a fatal start-up error
- **THEN** the error is logged and a desktop notification names it

### Requirement: Desktop entry
`contrib/mbv.desktop` SHALL launch `mbv --pin` with `Terminal=false`.

#### Scenario: Launcher entry
- **WHEN** the tarball, the `.deb` or an Arch package is installed
- **THEN** the mbv desktop entry runs `mbv --pin` without opening a terminal window

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`width` (integer logical pixels 1 through 65535, default 400), `width_expanded` (integer logical
pixels 1 through 65535, default 1100), `reserve` (boolean, default `true`), `gutter_top`,
`gutter_bottom`, `gutter_left`,
`gutter_right` (integers in pixels, may be negative, default 0), `accent` (boolean, default
`true`), `accent_color` (`"#RRGGBB"` or `"RRGGBB"`, default `"#dabc7f"`), `accent_width` (integer
pixels 1 through 65535, default 1), `pin_terminal` (argv prefix, default unset) and `terminal`
(argv prefix, default unset). An out-of-range or malformed value SHALL fall back to its default
with a logged warning, without changing the other keys. A `cover` key left in the section by an
earlier version SHALL be ignored without a warning. `cols` and `cols_expanded` left by an earlier
version SHALL be ignored with one logged warning that names them. The panel SHALL start with
mbv's four gutters, whatever pinwin's own configuration sets. The F2 settings screen SHALL
provide a Panel page with one row per value except the two argv prefixes: `Side` cycles between
left and right, `Accent` toggles on and off, `Reserve space` toggles on and off, `Accent color`
cycles through a fixed colour list
that always includes the configured value, the width rows step down and up by 10, or by 100 with
Shift, and the gutter rows and `Accent width` step by 1, or by 10 with Shift, within their
ranges. Whether a layout fits the output is pinwin's decision.

While pinned, a change to `Width` SHALL switch the panel to its collapsed width and a change to
`Expanded width` SHALL switch it to its expanded width, so the edited value is the width the panel
validates; a change to a side or gutter row SHALL apply at the panel's current width. A change to
`Reserve space` while the panel is collapsed SHALL switch the running panel's reserve mode:
enabling reserves the strip beside the panel, and disabling leaves the panel covering any strip it
already holds. A change to `Reserve space` while the panel is expanded SHALL send no request, and
the saved value decides the reserve mode the next collapse requests. A saved disable SHALL be
announced with a neutral toast saying a strip the panel already holds is released on the next
`mbv --pin` launch; a saved enable while expanded SHALL be announced with a neutral toast saying
the strip is reserved on the next collapse. A change to
an accent row SHALL NOT touch the running panel. A pinned panel request SHALL run off the TUI
thread, and at most one SHALL be in flight. A step that arrives while a request is pending SHALL
be ignored, with no queue. The outcome SHALL reach the same success or failure handling; only an
accepted request saves the value, and a refusal flashes pinwin's reason and saves nothing.

#### Scenario: Live change while pinned
- **WHEN** mbv runs in the panel and the user steps the width in F2's Panel page
- **THEN** the panel resizes without restarting mbv, and the value is saved

#### Scenario: Editing the expanded width while collapsed
- **WHEN** mbv runs in the panel at its collapsed width and the user steps `Expanded width`
- **THEN** the panel switches to its expanded width using the new value, and the value is saved

#### Scenario: Gutter change while expanded
- **WHEN** mbv runs in the panel at its expanded width and the user steps a gutter
- **THEN** the panel stays at its expanded width with the new gutter, and the gutter is saved

#### Scenario: Side change while expanded
- **WHEN** mbv runs in the panel at its expanded width and the user steps `Side`
- **THEN** the panel moves to the other edge at its expanded width, and the side is saved

#### Scenario: Enable reserve while pinned
- **WHEN** mbv runs in the panel at its collapsed width started with `reserve = false` and the user toggles `Reserve space` on
- **THEN** the compositor reserves the panel's strip at its collapsed width, tiled windows move aside, and the value is saved

#### Scenario: Disable reserve while pinned
- **WHEN** mbv runs in the panel at its collapsed width and holding a reserved strip, and the user toggles `Reserve space` off
- **THEN** the value is saved, the panel covers the strip it already holds, a neutral toast says the space is released on the next `mbv --pin` launch, and no tiled window moves

#### Scenario: Enable reserve while expanded
- **WHEN** mbv runs in the panel at its expanded width started with `reserve = false` and the user toggles `Reserve space` on
- **THEN** the value is saved, no pinwin request is sent, no tiled window moves, and a neutral toast says the strip is reserved on the next collapse

#### Scenario: Change while not pinned
- **WHEN** mbv runs in a terminal and the user changes a Panel row
- **THEN** the value is saved and used the next time `mbv --pin` starts

#### Scenario: Rejected layout while pinned
- **WHEN** mbv runs in the panel and a step would leave the panel's output with no width
- **THEN** a warning toast names the reason, the row keeps its previous value, and neither the panel nor `config.toml` changes

#### Scenario: Second step while a request is pending
- **WHEN** mbv runs in the panel, a step's request is still pending, and the user steps a Panel row
- **THEN** the second step is ignored, no second request is sent, and the row keeps its value until the first outcome arrives

#### Scenario: Old column keys
- **WHEN** `config.toml` still sets `cols = 40` and `cols_expanded = 120` under `[panel]`
- **THEN** both keys are ignored, one logged warning names them, and `width` and the other keys keep their values

#### Scenario: mbv gutters win over pinwin's configuration
- **WHEN** `[panel] gutter_top = 8` in mbv and pinwin's `[panel.mbv]` sets `gutter_top = 30`, and the user runs `mbv --pin`
- **THEN** the panel starts 8 pixels below the output's top edge

#### Scenario: Malformed accent colour
- **WHEN** `config.toml` sets `accent_color = "orange"`
- **THEN** the colour falls back to `#dabc7f` with a logged warning, and the other `[panel]` keys keep their values

#### Scenario: Custom colour survives cycling
- **WHEN** `accent_color` is `#123456`, which is not in the fixed list, and the user steps the `Accent color` row through the whole fixed list
- **THEN** the step after the last fixed entry returns to `#123456`, until the F2 settings screen is dismissed or mbv exits

#### Scenario: Malformed cover value
- **WHEN** `config.toml` still sets `cover = "yes"` or `cover = true` under `[panel]`
- **THEN** the key is ignored without a warning, and the other `[panel]` keys keep their values

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
mbv SHALL accept `--toggle`, which asks pinwin on the current Wayland display to hide the panel
named `mbv` if shown, or to show it if hidden, and exits with status 0. It SHALL NOT start mbv, a
TUI or a panel. When no such panel runs or the request fails, it SHALL report pinwin's reason
(stderr on a terminal, otherwise a desktop notification) and exit with status 1. Hiding and
showing SHALL NOT resize the TUI or change its width state. The Tray icon SHALL ask for the same
toggle while a pinned Client is attached; the Owner process sends the request then, and a failed
request is reported as a desktop notification.

#### Scenario: Hide from a compositor key
- **WHEN** the user presses a compositor key bound to `mbv --toggle` while the pinned panel is shown
- **THEN** the panel hides, playback continues, and the `mbv --toggle` process exits with status 0

#### Scenario: Show from a compositor key
- **WHEN** the user presses the same key while the panel is hidden
- **THEN** the panel shows at the same width with the TUI as it was, and takes keyboard focus

#### Scenario: Nothing pinned
- **WHEN** the user runs `mbv --toggle` in a terminal with no `mbv` panel running
- **THEN** one line gives pinwin's reason, and the process exits with status 1

#### Scenario: Nothing pinned from a key
- **WHEN** a compositor key runs `mbv --toggle` with no `mbv` panel running
- **THEN** a desktop notification gives pinwin's reason, and no mbv starts

#### Scenario: Toggle from the Tray
- **WHEN** a pinned Client is attached and the user left-clicks the Tray icon
- **THEN** the Owner process sends the same request, and the panel hides or shows as it did for
  `mbv --toggle`

### Requirement: Panel width decides pushing or covering
`mbv --pin` SHALL build the collapsed panel layout in pushing mode when `[panel] reserve` is true
and in covering mode when it is false, and the expanded panel layout
in covering mode always. A pushing panel makes the compositor reserve a strip
beside it, so tiled windows move aside. A covering panel draws over the tiled windows and holds
the strip already reserved, so expanding and collapsing move no tiled window. mbv SHALL provide no
runtime toggle for the expanded width's covering mode.

#### Scenario: Collapsed panel pushes
- **WHEN** `mbv --pin` starts with `reserve` unset or true
- **THEN** the compositor reserves a strip beside the panel and tiled windows move aside

#### Scenario: Collapsed panel covers
- **WHEN** `mbv --pin` starts with `reserve = false`
- **THEN** the panel draws over the tiled windows and no strip is reserved

#### Scenario: Expanding leaves tiles in place
- **WHEN** mbv runs in the panel at its collapsed width and the user expands it
- **THEN** the panel draws over the tiled windows and no tiled window moves

#### Scenario: Collapsing leaves tiles in place
- **WHEN** mbv runs in the panel at its expanded width and the user collapses it
- **THEN** the panel shrinks to the collapsed strip and no tiled window moves

### Requirement: Hidden panel follows its width
A hidden panel SHALL release its reserved strip at any width, and showing it SHALL take the strip
back at the width it had.

#### Scenario: Hiding a collapsed panel
- **WHEN** the panel runs at its collapsed width and the user hides it
- **THEN** the reserved strip is released and tiled windows take the space back

#### Scenario: Hiding an expanded panel
- **WHEN** the panel runs at its expanded width and the user hides it, then shows it
- **THEN** tiled windows take the strip while it is hidden, move back beside it when it shows, and the panel shows at its expanded width

### Requirement: Pinned Client identity
An mbv Client SHALL run as pinned exactly when pinwin started it inside a panel, whatever started
the panel. It SHALL be pinned only when its environment holds a non-empty `PINWIN_PANEL` and its
argv holds the hidden `--in-panel` launch flag. A Client with `PINWIN_PANEL` but without the flag
SHALL run as unpinned. The `--pin` launcher SHALL append `--in-panel` to the mbv command it gives
`pinwin run`. An mbv that starts in a panel SHALL remove `PINWIN_PANEL` and `PINWIN_SOCKET` from
its environment whether it is pinned or not, so no child inherits them. Its panel requests SHALL
go to that panel and its daemon. A Client started outside a panel SHALL run as unpinned, also
when `--pin` asked for the panel. A process that mbv starts, including the Owner process, SHALL
NOT be pinned because its starter was.

#### Scenario: Panel started by hand
- **WHEN** the user runs `pinwin run --name tunes -- ghostty --gtk-single-instance=false -e mbv --in-panel`
- **THEN** that mbv runs as pinned, and `x` resizes the `tunes` panel

#### Scenario: Panel started by hand without the launch flag
- **WHEN** the user runs `pinwin run --name tunes -- ghostty --gtk-single-instance=false -e mbv`
- **THEN** that mbv runs as unpinned, `x` keeps its normal behavior, and its children inherit no `PINWIN_*`

#### Scenario: Shell started inside the panel
- **WHEN** a shell or tmux started in the panel terminal starts `mbv` without `--in-panel`
- **THEN** that mbv runs as unpinned even though it inherited `PINWIN_PANEL`, and its children inherit no `PINWIN_*`

#### Scenario: Terminal gives the launch to a running copy
- **WHEN** `mbv --pin` runs with a pin terminal that gives its launch to a running copy of itself
- **THEN** mbv opens in that copy's normal window and runs as unpinned

#### Scenario: Owner started from the panel
- **WHEN** the first Client is a pinned Client that starts the Owner process with stay-alive on, and the panel then ends
- **THEN** the Owner process keeps running and does not count as pinned

### Requirement: Pin terminal
The terminal that runs mbv in the panel SHALL be the `[panel] pin_terminal` argv prefix when set,
otherwise the `TERMINAL` environment variable followed by `-e`. mbv SHALL NOT fall back to any
other terminal. With neither set, `--pin` SHALL fail with a reason that names both sources.

#### Scenario: Config value set
- **WHEN** `pin_terminal = ["ghostty", "--gtk-single-instance=false", "-e"]` and the user runs `mbv --pin`
- **THEN** the panel holds a Ghostty window that runs mbv

#### Scenario: Neither set
- **WHEN** neither `pin_terminal` nor `TERMINAL` is set and the user runs `mbv --pin` in a terminal
- **THEN** one line names both sources and mbv runs in that terminal

### Requirement: No panel library is linked
No mbv crate or binary SHALL link a panel library or its Wayland client, keyboard, font or raster
stack. mbv SHALL reach pinwin only by running the `pinwin` program, version 0.4.1 or later. No
mbv binary SHALL link GTK or gtk4-layer-shell.

#### Scenario: Build without pinwin
- **WHEN** `mbv` or `mbvd` is built on a machine without pinwin
- **THEN** the build succeeds and no pinwin, Wayland client, xkbcommon or fontconfig crate is involved

#### Scenario: Runtime without pinwin
- **WHEN** `pinwin` is not installed and the user runs `mbv --pin` in a terminal
- **THEN** one line says that pinwin is not installed and mbv runs in that terminal

### Requirement: Retired pinned width keybinding
A `[keys]` entry that names the retired `pinned_width_toggle` action SHALL be ignored with one
logged warning that names the action. It SHALL NOT stop startup, and it SHALL NOT bind any
action. Other unknown action identifiers SHALL keep the existing load error.

#### Scenario: Config still binds the retired action
- **WHEN** `config.toml` contains a `[keys]` entry for `pinned_width_toggle`
- **THEN** mbv logs one warning that names the retired action, starts normally, and no key runs a pinned width toggle
