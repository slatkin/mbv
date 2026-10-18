# Spec Delta

## ADDED Requirements

### Requirement: Pinned Client identity
An mbv Client SHALL run as pinned exactly when pinwin started it inside a panel, whatever started
the panel. Its panel requests SHALL go to that panel and its daemon. A Client started outside a
panel SHALL run as unpinned, also when `--pin` asked for the panel. A process that mbv starts,
including the Owner process, SHALL NOT be pinned because its starter was.

#### Scenario: Panel started by hand
- **WHEN** the user runs `pinwin run --name tunes -- ghostty --gtk-single-instance=false -e mbv`
- **THEN** that mbv runs as pinned, and `x` resizes the `tunes` panel

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

## MODIFIED Requirements

### Requirement: Pin flag
mbv SHALL accept `--pin`, which runs the TUI in a pinwin panel named `mbv`: pinwin docks the pin
terminal as the panel, and that terminal runs mbv. The process that ran `--pin` SHALL wait until
the panel's terminal ends. Without `--pin`, mbv SHALL start in the current terminal exactly as
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
desktop notification naming it when stdin is not a terminal, and exit with that status; it SHALL
NOT then run in the terminal. A fatal start-up error of the mbv in the panel SHALL be logged and
sent as a desktop notification.

#### Scenario: Terminal fallback
- **WHEN** the user types `mbv --pin` in a terminal with `WAYLAND_DISPLAY` unset
- **THEN** one warning line is printed and mbv runs in that terminal

#### Scenario: Saved layout does not fit
- **WHEN** `mbv --pin` starts with a `[panel] width` that leaves the output no width
- **THEN** the panel does not open, and mbv exits with pinwin's failure status

#### Scenario: Desktop launch without layer-shell
- **WHEN** the desktop entry is launched on GNOME or X11
- **THEN** a notification says the panel could not open, and mbv exits with a non-zero status

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`width` (integer logical pixels 1 through 65535, default 400), `width_expanded` (integer logical
pixels 1 through 65535, default 1100), `accent` (boolean, default `true`), `accent_color`
(`"#RRGGBB"` or `"RRGGBB"`, default `"#dabc7f"`), `accent_width` (integer pixels 1 through 65535,
default 1), `pin_terminal` (argv prefix, default unset) and `terminal` (argv prefix, default
unset). An out-of-range or malformed value SHALL fall back to its default with a logged warning,
without changing the other keys. A `cover` key left in the section by an earlier version SHALL be
ignored without a warning. `cols`, `cols_expanded`, `gutter_top`, `gutter_bottom`, `gutter_left`
and `gutter_right` left by an earlier version SHALL be ignored with one logged warning that names
them. The panel's gutters SHALL come from pinwin's own configuration. The F2 settings screen SHALL
provide a Panel page with one row per value except the two argv prefixes: `Side` cycles between
left and right, `Accent` toggles on and off, `Accent color` cycles through a fixed colour list
that always includes the configured value, the width rows step down and up by 10, or by 100 with
Shift, and `Accent width` steps by 1, or by 10 with Shift, within their ranges. Whether a layout
fits the output is pinwin's decision.

While pinned, a change to `Width` SHALL switch the panel to its collapsed width and a change to
`Expanded width` SHALL switch it to its expanded width, so the edited value is the width the panel
validates; a change to `Side` SHALL move the panel to that edge at its current width. A change to
an accent row SHALL NOT touch the running panel.

#### Scenario: Live change while pinned
- **WHEN** mbv runs in the panel and the user steps the width in F2's Panel page
- **THEN** the panel resizes without restarting mbv, and the value is saved

#### Scenario: Editing the expanded width while collapsed
- **WHEN** mbv runs in the panel at its collapsed width and the user steps `Expanded width`
- **THEN** the panel switches to its expanded width using the new value, and the value is saved

#### Scenario: Gutter change while expanded
- **WHEN** mbv runs in the panel at its expanded width and the user edits a gutter in pinwin's `[panel.mbv]` table
- **THEN** the running panel keeps its gutters and width, mbv's F2 Panel page offers no gutter row, and the new gutter applies at the next `mbv --pin` launch

#### Scenario: Side change while expanded
- **WHEN** mbv runs in the panel at its expanded width and the user steps `Side`
- **THEN** the panel moves to the other edge at its expanded width, and the side is saved

#### Scenario: Change while not pinned
- **WHEN** mbv runs in a terminal and the user changes a Panel row
- **THEN** the value is saved and used the next time `mbv --pin` starts

#### Scenario: Rejected layout while pinned
- **WHEN** mbv runs in the panel and a step would leave the panel's output with no width
- **THEN** a warning toast names the reason, the row keeps its previous value, and neither the panel nor `config.toml` changes

#### Scenario: Old column and gutter keys
- **WHEN** `config.toml` still sets `cols = 40` and `gutter_top = 8` under `[panel]`
- **THEN** both keys are ignored, one logged warning names them, and `width` and the other keys keep their values

#### Scenario: Malformed accent colour
- **WHEN** `config.toml` sets `accent_color = "orange"`
- **THEN** the colour falls back to `#dabc7f` with a logged warning, and the other `[panel]` keys keep their values

#### Scenario: Custom colour survives cycling
- **WHEN** `accent_color` is `#123456`, which is not in the fixed list, and the user steps the `Accent color` row through the whole fixed list
- **THEN** the step after the last fixed entry returns to `#123456`, until the F2 settings screen is dismissed or mbv exits

#### Scenario: Malformed cover value
- **WHEN** `config.toml` still sets `cover = "yes"` or `cover = true` under `[panel]`
- **THEN** the key is ignored without a warning, and the other `[panel]` keys keep their values

### Requirement: Panel toggle
mbv SHALL accept `--toggle`, which asks pinwin on the current Wayland display to hide the panel
named `mbv` if shown, or to show it if hidden, and exits with status 0. It SHALL NOT start mbv, a
TUI or a panel. When no such panel runs or the request fails, it SHALL report pinwin's reason
(stderr on a terminal, otherwise a desktop notification) and exit with status 1. Hiding and
showing SHALL NOT resize the TUI or change its width state.

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

### Requirement: Hidden panel follows its width
A hidden panel SHALL release its reserved strip at any width, and showing it SHALL take the strip
back at the width it had.

#### Scenario: Hiding a collapsed panel
- **WHEN** the panel runs at its collapsed width and the user hides it
- **THEN** the reserved strip is released and tiled windows take the space back

#### Scenario: Hiding an expanded panel
- **WHEN** the panel runs at its expanded width and the user hides it, then shows it
- **THEN** tiled windows take the strip while it is hidden, move back beside it when it shows, and the panel shows at its expanded width

## REMOVED Requirements

### Requirement: Panel libraries stay out of the daemon crates
**Reason**: mbv links no panel library at all after the move to pinwin 0.4.
**Migration**: See "No panel library is linked".

### Requirement: Pinned panel width toggle
**Reason**: `x` is the only pinned width control; the width-only toggle had no remaining use.
**Migration**: Use `x` while pinned. A config that rebinds `pinned_width_toggle` must drop that entry.
