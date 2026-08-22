# panel-mode Specification

## Purpose

Controls which of the two Power View panels is visible — both, library-only, or queue-only — through a one-key forward cycle, so the user can give the full window to whichever side matters most.

## Requirements

### Requirement: Panel layout is a three-state cycle on `x`

The Power View layout SHALL be controlled by a single mode with three states: both panels visible, only the library visible, and only the queue visible. Pressing the `x` key SHALL advance the mode forward by one state in the cycle both -> queue-only -> library-only -> both. The mode SHALL NOT be persisted across sessions and SHALL start at `both`.

#### Scenario: Advance from both

- **WHEN** the layout is in `both` state and the user presses `x`
- **THEN** the layout changes to queue-only, showing only the left (queue) panel

#### Scenario: Advance from queue-only

- **WHEN** the layout is in queue-only state and the user presses `x`
- **THEN** the layout changes to library-only, showing only the right (library) panel

#### Scenario: Advance from library-only

- **WHEN** the layout is in library-only state and the user presses `x`
- **THEN** the layout returns to `both`, showing both panels

#### Scenario: Starts at both

- **WHEN** the application starts
- **THEN** the panel mode SHALL be `both`, regardless of the mode when the application last exited

### Requirement: Library-only hides the queue column

In library-only state the library panel SHALL occupy the full window width and the queue column SHALL not be rendered. This state SHALL be the same as today's collapsed-queue behavior.

#### Scenario: Full-width library

- **WHEN** the layout is in library-only state
- **THEN** the library list SHALL span the full window width

#### Scenario: Queue not rendered

- **WHEN** the layout is in library-only state
- **THEN** the queue list, playback card, and visualizer SHALL NOT be rendered

### Requirement: Queue-only hides the library column

In queue-only state the queue panel SHALL render across the full window width. The tab bar, library list, and status bar SHALL NOT be rendered. The playback panel SHALL be rendered within the left column (see `queue-only-playback` capability for layout details).

#### Scenario: Full-width queue

- **WHEN** the layout is in queue-only state
- **THEN** the queue list SHALL span the full window width

#### Scenario: Right column not rendered

- **WHEN** the layout is in queue-only state
- **THEN** the tab bar, library list, and status bar SHALL NOT be rendered

#### Scenario: Playback panel rendered in left column

- **WHEN** the layout is in queue-only state
- **THEN** the playback panel SHALL be rendered within the queue-only left column layout

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

The queue-column resize keys and the Alt+Left return-to-queue key SHALL be inactive whenever the layout is not in `both` state.

#### Scenario: Resize disabled in queue-only

- **WHEN** the layout is in queue-only state
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing

#### Scenario: Resize disabled in library-only

- **WHEN** the layout is in library-only state
- **THEN** the Shift+Left/Shift+Right column-width resize keys SHALL do nothing

#### Scenario: Return-to-queue disabled outside queue

- **WHEN** the layout is not in `both` state
- **THEN** the Alt+Left return-to-queue key SHALL do nothing

### Requirement: Mini view replaces the cycle below 80 columns

When the terminal is narrower than 80 columns, the Power View layout SHALL use a separate two-state "mini view" instead of the three-state both/library-only/queue-only cycle: only the library panel or only the queue panel is shown, full width. Pressing `x` SHALL toggle between these two states. Mini view SHALL start at queue-only whenever it is first shown, including application start or when the terminal narrows below 80 columns after being wide, and SHALL NOT be persisted across sessions.

Mini view's last-shown panel SHALL be tracked independently of the three-state `panel_mode` used at 80+ columns. Widening the terminal back to 80+ columns SHALL restore whatever `panel_mode` and panel focus were active before the terminal narrowed, unchanged by any mini-view toggling that happened while narrow.

#### Scenario: Narrow terminal starts in queue-only mini view

- **WHEN** the application starts, or the terminal narrows below 80 columns
- **THEN** the layout shows only the queue panel, full width

#### Scenario: `x` toggles mini view

- **WHEN** the terminal is narrower than 80 columns and the user presses `x`
- **THEN** the layout switches to showing only the other panel (queue-only <-> library-only), full width

#### Scenario: Both is unreachable while narrow

- **WHEN** the terminal is narrower than 80 columns
- **THEN** pressing `x` any number of times SHALL never show both panels at once

#### Scenario: Widening restores prior wide-mode state

- **GIVEN** the terminal was wide in `queue-only` mode with queue focus, then narrowed (entering mini view) and the user toggled mini view to library-only
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

In queue-only state — at any terminal width — the queue panel SHALL render with the same focused styling it has when focused in the `both` state whenever panel focus is on the queue: the focused background, cursor highlight, and scrollbar. The prior behavior of always rendering the queue-only panel with unfocused styling is removed.

#### Scenario: Queue-only renders focused when the queue holds focus

- **WHEN** the layout is in queue-only state and panel focus is on the queue
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar

#### Scenario: Both keeps the focused appearance

- **WHEN** the layout is in `both` state and the queue panel is focused
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar