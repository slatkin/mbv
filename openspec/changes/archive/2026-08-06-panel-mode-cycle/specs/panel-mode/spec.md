## Purpose

Controls which of the two Power View panels is visible — both, library-only, or queue-only — through a one-key forward cycle, so the user can give the full window to whichever side matters most.

## ADDED Requirements

### Requirement: Panel layout is a three-state cycle on `x`

The Power View layout SHALL be controlled by a single mode with three states: both panels visible, only the library visible, only the queue visible. Pressing the `x` key SHALL advance the mode forward by one state in the cycle both -> library-only -> queue-only -> both. The mode SHALL NOT be persisted across sessions and SHALL start at `both`.

#### Scenario: Advance from both

- **WHEN** the layout is in `both` state and the user presses `x`
- **THEN** the layout changes to library-only, showing only the right (library) panel

#### Scenario: Advance from library-only

- **WHEN** the layout is in library-only state and the user presses `x`
- **THEN** the layout changes to queue-only, showing only the left (queue) panel

#### Scenario: Advance from queue-only

- **WHEN** the layout is in queue-only state and the user presses `x`
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

In queue-only state the queue panel SHALL render across the full window width and the right column (tab bar, player, library list, status bar) SHALL NOT be rendered.

#### Scenario: Full-width queue

- **WHEN** the layout is in queue-only state
- **THEN** the queue list SHALL span the full window width

#### Scenario: Right column not rendered

- **WHEN** the layout is in queue-only state
- **THEN** the tab bar, player, library list, and status bar SHALL NOT be rendered

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

### Requirement: Queue-only renders the queue panel unfocused

In queue-only state the queue panel SHALL render with the same visual styling it has when unfocused in the `both` state: the plain column background, the unfocused panel frame, and muted row text with no cursor highlight and no scrollbar. Panel focus for input SHALL remain on the queue.

#### Scenario: Unfocused appearance in queue-only

- **WHEN** the layout is in queue-only state
- **THEN** the queue panel SHALL render with the unfocused background, no cursor highlight, and no scrollbar

#### Scenario: Both keeps the focused appearance

- **WHEN** the layout is in `both` state and the queue panel is focused
- **THEN** the queue panel SHALL render with its focused background, cursor highlight, and scrollbar