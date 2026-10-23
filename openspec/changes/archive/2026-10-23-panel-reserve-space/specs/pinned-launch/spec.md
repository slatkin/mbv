# Spec Delta

## MODIFIED Requirements

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
