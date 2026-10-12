# panel-mode Specification

## Purpose

Controls which of the two Power View panels is visible — both, library-only, or queue-only — through a one-key forward cycle, so the user can give the full window to whichever side matters most.

## Requirements

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

### Requirement: Library-only hides the queue column

In library-only state the library panel SHALL occupy the full window width and the queue column SHALL not be rendered. This state SHALL be the same as today's collapsed-queue behavior. Library-only SHALL render the Library playback panel, because it renders exactly when the queue column is hidden; the library's content area SHALL start below it.

#### Scenario: Full-width library

- **WHEN** the layout is in library-only state
- **THEN** the library list SHALL span the full window width

#### Scenario: Queue not rendered

- **WHEN** the layout is in library-only state
- **THEN** the queue list, playback card, and visualizer SHALL NOT be rendered

#### Scenario: Library playback panel renders in library-only

- **WHEN** the layout is in library-only state at a width of 80 columns or more, or the library panel is shown in narrow mini view
- **THEN** the playback panel SHALL render as the right-column strip below the Tab panel

### Requirement: Queue-only hides the library column

In queue-only state the queue panel SHALL render across the full window width. The Tab panel, library list, Status bar panel, and Library playback panel SHALL NOT be rendered. When playback is active, the playback panel SHALL be rendered within the queue column (see `queue-playback-panel` for the placement and idle rules); fully idle queue-only state SHALL omit it so its rows belong to the Queue.

#### Scenario: Full-width queue

- **WHEN** the layout is in queue-only state
- **THEN** the queue list SHALL span the full window width

#### Scenario: Right column not rendered

- **WHEN** the layout is in queue-only state
- **THEN** the Tab panel, library list, Status bar panel, and Library playback panel SHALL NOT be rendered

#### Scenario: Playback panel rendered in left column

- **WHEN** the layout is in queue-only state and playback is active
- **THEN** the playback panel SHALL be rendered within the queue column layout

#### Scenario: Idle queue-only omits the panel

- **WHEN** the layout is in queue-only state and no transport is active
- **THEN** the playback panel SHALL NOT be rendered and its rows SHALL belong to the queue panel

### Requirement: Focus follows the mode

Panel focus SHALL follow the layout mode: library-only forces focus to the library; queue-only forces focus to the queue.

#### Scenario: Library-only forces library focus

- **WHEN** the mode changes to library-only and the focused panel is the queue
- **THEN** panel focus SHALL move to the library panel

#### Scenario: Queue-only forces queue focus

- **WHEN** the mode changes to queue-only
- **THEN** panel focus SHALL move to the queue panel

#### Scenario: Both leaves focus alone

- **WHEN** the mode changes to both
- **THEN** the panel focus SHALL be left unchanged

### Requirement: Column resize deactivated outside both

Queue-column resizing by keyboard or mouse and the Alt+Left return-to-queue key SHALL be inactive whenever the Panel mode is not `both`. Mouse resizing SHALL also be inactive while an overlay or popup exclusively owns mouse delivery. Leaving `both` or losing panel mouse eligibility during an armed boundary gesture SHALL cancel that gesture without changing or persisting the width again.

#### Scenario: Resize disabled in queue-only

- **WHEN** the Panel mode is queue-only
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing
- **AND** no queue-column mouse resize target SHALL be active

#### Scenario: Resize disabled in library-only

- **WHEN** the Panel mode is library-only
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing
- **AND** no queue-column mouse resize target SHALL be active

#### Scenario: Return-to-queue disabled outside queue

- **WHEN** the Panel mode is not `both`
- **THEN** the Alt+Left return-to-queue key SHALL do nothing

#### Scenario: Interrupted boundary drag is cancelled

- **WHEN** an armed queue-column boundary drag loses eligibility because the Panel mode changes or an overlay or popup takes exclusive mouse delivery
- **THEN** the resize gesture is cancelled
- **AND** a later drag event cannot continue from the stale anchor

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

### Requirement: Mini view moves focus with the panel

Toggling mini view SHALL move panel focus to the panel now shown, the same way the three-state cycle's queue-only state forces queue focus.

#### Scenario: Toggling to queue-only mini view focuses the queue

- **WHEN** the terminal is narrower than 80 columns and the user presses `x` to switch from library-only to queue-only mini view
- **THEN** panel focus SHALL move to the queue panel

#### Scenario: Toggling to library-only mini view focuses the library

- **WHEN** the terminal is narrower than 80 columns and the user presses `x` to switch from queue-only to library-only mini view
- **THEN** panel focus SHALL move to the library panel

### Requirement: Queue-only renders the queue panel focused when it holds focus

In queue-only state at 80+ columns the queue panel SHALL render with the same focused styling it has when focused in the `both` state whenever panel focus is on the queue: the focused background, cursor highlight, and scrollbar. Mini view is governed by `Pinned mini view follows window focus` instead: outside the pinned panel it renders the resting palette.

#### Scenario: Queue-only renders focused when the queue holds focus

- **WHEN** the layout is in queue-only state at 80+ columns and panel focus is on the queue
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar

#### Scenario: Both keeps the focused appearance

- **WHEN** the layout is in `both` state and the queue panel is focused
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar

### Requirement: Pinned mini view follows window focus

When mbv runs in the pinned panel and the terminal is narrower than 80 columns, the displayed
panel SHALL render with the focused palette while the window holds focus and with the resting
palette while it does not. The window SHALL be treated as focused from launch until a focus-lost
report arrives. Outside the pinned panel, mini view SHALL keep rendering the resting palette
regardless of window focus. At 80+ columns, rendering SHALL NOT depend on window focus.

#### Scenario: Pinned mini view is focused while the panel holds the keyboard

- **WHEN** the pinned panel is narrower than 80 columns and the window holds focus
- **THEN** the displayed panel renders its focused palette

#### Scenario: Pinned mini view dims when focus leaves

- **WHEN** the pinned panel is narrower than 80 columns and a focus-lost report arrives
- **THEN** the displayed panel renders its resting palette until a focus-gained report arrives

#### Scenario: Launch starts focused

- **WHEN** a pinned launch starts narrower than 80 columns and no focus report has arrived
- **THEN** the displayed panel renders its focused palette

#### Scenario: Unpinned mini view is unchanged

- **WHEN** mbv is not pinned, the terminal is narrower than 80 columns, and any focus report arrives
- **THEN** the displayed panel renders its resting palette

#### Scenario: Wide view ignores window focus

- **WHEN** the terminal is 80 or more columns wide and a focus-lost report arrives
- **THEN** panels keep rendering by panel focus alone

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

### Requirement: Two-panel layout hides an empty queue column

At 80+ columns, while the Panel mode is `both` and the displayed queue has no slots, the window SHALL render exactly as library-only: no queue column and no queue boundary, the Library playback panel shown, and the library at full width. The Panel mode SHALL stay `both`. When the displayed queue gains a slot, the two-panel layout SHALL return without user action. Queue-only and the mini-view queue panel SHALL keep the empty-queue placeholder.

#### Scenario: Empty queue renders library-only

- **WHEN** the Panel mode is `both` at 80+ columns and the displayed queue is empty
- **THEN** the library SHALL span the full window width, the Library playback panel SHALL render, and the queue column SHALL NOT render

#### Scenario: Queue column returns when the queue fills

- **WHEN** the queue column is hidden because the displayed queue is empty and a slot is added to it
- **THEN** the two-panel layout SHALL render with the queue column

#### Scenario: Cycle continues from both

- **WHEN** the queue column is hidden because the displayed queue is empty and the user presses `x`
- **THEN** the Panel mode SHALL advance to queue-only

#### Scenario: Queue-only keeps the placeholder

- **WHEN** the Panel mode is queue-only, or mini view shows the queue panel, and the displayed queue is empty
- **THEN** the queue panel SHALL render with the empty-queue placeholder

#### Scenario: Resize inactive while hidden

- **WHEN** the queue column is hidden because the displayed queue is empty
- **THEN** queue-column resizing and the Alt+Left return-to-queue key SHALL be inactive

### Requirement: Hidden empty queue moves focus to the library

When the queue column is hidden because the displayed queue is empty, panel focus SHALL be on the library. Focus SHALL stay on the library when the queue column returns.

#### Scenario: Queue focus moves when the queue empties

- **WHEN** the queue holds panel focus in the two-panel layout and its last slot is removed
- **THEN** panel focus SHALL move to the library

#### Scenario: Focus stays on the library on refill

- **WHEN** focus moved to the library because the queue emptied, and the user then adds an item to the queue
- **THEN** panel focus SHALL remain on the library
