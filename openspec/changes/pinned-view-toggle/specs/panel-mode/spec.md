## MODIFIED Requirements

### Requirement: Panel layout is a three-state cycle on `x`

When mbv is NOT running in the pinned panel, the Power View layout SHALL be controlled by a single
mode with three states: both panels visible, only the library visible, and only the queue visible.
Pressing the `x` key SHALL advance the mode forward by one state in the cycle both -> queue-only ->
library-only -> both. The mode SHALL NOT be persisted across sessions and SHALL start at `both`.
When mbv runs in the pinned panel, this cycle SHALL NOT apply: `x` is governed by
`Pinned launches bind x to the pinned views` instead.

#### Scenario: Advance from both

- **WHEN** mbv is not pinned, the layout is in `both` state and the user presses `x`
- **THEN** the layout changes to queue-only, showing only the left (queue) panel

#### Scenario: Advance from queue-only

- **WHEN** mbv is not pinned, the layout is in queue-only state and the user presses `x`
- **THEN** the layout changes to library-only, showing only the right (library) panel

#### Scenario: Advance from library-only

- **WHEN** mbv is not pinned, the layout is in library-only state and the user presses `x`
- **THEN** the layout returns to `both`, showing both panels

#### Scenario: Starts at both

- **WHEN** the application starts
- **THEN** the panel mode SHALL be `both`, regardless of the mode when the application last exited

### Requirement: Mini view replaces the cycle below 80 columns

When mbv is NOT running in the pinned panel and the terminal is narrower than 80 columns, the
Power View layout SHALL use a separate two-state "mini view" instead of the three-state
both/library-only/queue-only cycle: only the library panel or only the queue panel is shown, full
width. Pressing `x` SHALL toggle between these two states. Mini view SHALL start at queue-only
whenever it is first shown, including application start or when the terminal narrows below 80
columns after being wide, and SHALL NOT be persisted across sessions. When mbv runs in the pinned
panel, this mini-view toggle SHALL NOT apply: `x` is governed by
`Pinned launches bind x to the pinned views` instead.

Mini view's last-shown panel SHALL be tracked independently of the three-state `panel_mode` used
at 80+ columns. Outside the pinned panel, widening the terminal back to 80+ columns SHALL restore
whatever `panel_mode` and panel focus were active before the terminal narrowed, unchanged by any
mini-view toggling that happened while narrow.

#### Scenario: Narrow terminal starts in queue-only mini view

- **WHEN** mbv is not pinned, and the application starts or the terminal narrows below 80 columns
- **THEN** the layout shows only the queue panel, full width

#### Scenario: `x` toggles mini view

- **WHEN** mbv is not pinned, the terminal is narrower than 80 columns and the user presses `x`
- **THEN** the layout switches to showing only the other panel (queue-only <-> library-only), full width

#### Scenario: Both is unreachable while narrow

- **WHEN** mbv is not pinned and the terminal is narrower than 80 columns
- **THEN** pressing `x` any number of times SHALL never show both panels at once

#### Scenario: Widening restores prior wide-mode state

- **GIVEN** mbv is not pinned, and the terminal was wide in `queue-only` mode with queue focus,
  then narrowed (entering mini view) and the user toggled mini view to library-only
- **WHEN** the terminal widens back to 80+ columns
- **THEN** the layout returns to `queue-only` mode with queue focus, as it was before narrowing

## ADDED Requirements

### Requirement: Pinned launches bind x to the pinned views

When mbv runs in the pinned panel, pressing `x` SHALL toggle the panel between its two pinned
views: from the collapsed width (`cols`) it SHALL expand the panel to `cols_expanded` through the
same resize path as the width toggle and show library-only with library focus; from the expanded
width (`cols_expanded`) it SHALL collapse the panel to `cols` and show queue-only with queue focus
(in mini view when the collapsed width is below 80 columns). If the target width is rejected by
the panel, a warning toast SHALL name the reason and neither the width nor the displayed panel
mode SHALL change. The three-state `x` cycle and the mini-view `x` toggle SHALL NOT be reachable
while pinned. Outside the pinned panel this requirement SHALL NOT apply.

#### Scenario: x expands to the library view

- **WHEN** mbv runs in the pinned panel at its collapsed width and the user presses `x`
- **THEN** the panel animates to `cols_expanded` columns and the layout shows library-only with library focus

#### Scenario: x collapses to the queue view

- **WHEN** mbv runs in the pinned panel at its expanded width and the user presses `x`
- **THEN** the panel collapses to `cols` columns and the layout shows queue-only with queue focus

#### Scenario: Collapsed queue view is mini view at the default width

- **WHEN** `cols` is below 80 columns and the user presses `x` to collapse from the expanded width
- **THEN** the collapsed panel shows the queue-only mini view with queue focus

#### Scenario: Rejected expanded width leaves the view alone

- **WHEN** mbv runs in the pinned panel at its collapsed width, `x` is pressed, and `cols_expanded`
  would leave the output no width
- **THEN** a warning toast names the reason, the panel keeps its collapsed width, and the displayed
  panel mode is unchanged

#### Scenario: The cycle is unreachable while pinned

- **WHEN** mbv runs in the pinned panel and the user presses `x` any number of times
- **THEN** the three-state cycle and the mini-view toggle never run; the layout is always one of
  the two pinned views
