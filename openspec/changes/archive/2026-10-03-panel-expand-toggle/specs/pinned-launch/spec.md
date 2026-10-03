## MODIFIED Requirements

### Requirement: Panel layout settings
The `[panel]` section of `config.toml` SHALL hold `side` (`"left"` or `"right"`, default `"left"`),
`cols` (integer 1 through 65535, default 40), `cols_expanded` (integer 1 through 65535, default 120)
and `gutter_top`, `gutter_bottom`, `gutter_left`, `gutter_right` (integers in pixels, may be
negative, default 0). An out-of-range or malformed value SHALL fall back to its default with a
logged warning, without changing the other keys. The F2 settings screen SHALL provide a Panel page
with one row per value: `Side` cycles between left and right, and the numeric rows step down and up
by 1, or by 10 with Shift, within their ranges. Layout semantics (docking, reservation, negative
gutters, validation) are those of the `pinwin-panel` capability.

While pinned, a change to `Cols` SHALL switch the panel to its collapsed width and a change to
`Expanded cols` SHALL switch it to its expanded width, so the edited value is the width the panel
validates; a change to any other row SHALL apply at the panel's current width.

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

## ADDED Requirements

### Requirement: Pinned panel width toggle
mbv SHALL provide a configurable keybind action, `pinned_width_toggle` (default `Ctrl+e`, Global
section), that switches the running pinned panel between its collapsed width (`cols`) and its
expanded width (`cols_expanded`), keeping `side` and the gutters. The switch SHALL resize the
running panel without restarting mbv, and the space reserved beside tiled windows SHALL follow the
new width. Which width is active SHALL NOT be saved: every pinned launch SHALL start at the
collapsed width. If the panel rejects the other width, a warning toast SHALL name the reason and
the panel SHALL stay at its current width. When mbv is not running in a pinned panel, the action
SHALL show a neutral toast saying it needs a pinned launch and change nothing.

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
- **WHEN** the user presses `Ctrl+e` and `cols_expanded` would leave the output no width
- **THEN** a warning toast names the reason and the panel keeps its collapsed width

#### Scenario: Not pinned
- **WHEN** mbv runs in a terminal and the user presses `Ctrl+e`
- **THEN** a neutral toast says the toggle needs a pinned launch, and nothing else changes
