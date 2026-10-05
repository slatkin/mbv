## MODIFIED Requirements

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
