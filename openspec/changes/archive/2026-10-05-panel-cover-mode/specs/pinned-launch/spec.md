## MODIFIED Requirements

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

## ADDED Requirements

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
