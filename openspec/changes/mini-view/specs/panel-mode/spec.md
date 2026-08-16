## ADDED Requirements

### Requirement: Mini view replaces the cycle below 80 columns

When the terminal is narrower than 80 columns, the Power View layout SHALL
use a separate two-state "mini view" instead of the three-state
both/library-only/queue-only cycle: only the library panel or only the
queue panel is shown, full width. Pressing `x` SHALL toggle between these
two states. Mini view SHALL start at library-only whenever it is first
shown (application start, or the terminal narrows below 80 columns after
being wide) and SHALL NOT be persisted across sessions.

Mini view's last-shown panel SHALL be tracked independently of the
three-state `panel_mode` used at 80+ columns. Widening the terminal back
to 80+ columns SHALL restore whatever `panel_mode` and panel focus were
active before the terminal narrowed, unchanged by any mini-view toggling
that happened while narrow.

#### Scenario: Narrow terminal starts in library-only mini view

- **WHEN** the application starts, or the terminal narrows below 80
  columns, with no mini-view state yet
- **THEN** the layout shows only the library panel, full width

#### Scenario: `x` toggles mini view

- **WHEN** the terminal is narrower than 80 columns and the user presses
  `x`
- **THEN** the layout switches to showing only the other panel
  (library-only ↔ queue-only), full width

#### Scenario: Both is unreachable while narrow

- **WHEN** the terminal is narrower than 80 columns
- **THEN** pressing `x` any number of times SHALL never show both panels
  at once

#### Scenario: Widening restores prior wide-mode state

- **GIVEN** the terminal was wide in `queue-only` mode with queue focus,
  then narrowed (entering mini view) and the user toggled mini view to
  library-only
- **WHEN** the terminal widens back to 80+ columns
- **THEN** the layout returns to `queue-only` mode with queue focus, as it
  was before narrowing

### Requirement: Mini view moves focus with the panel

Toggling mini view SHALL move panel focus to the panel now shown, the same
way the three-state cycle's queue-only state forces queue focus.

#### Scenario: Toggling to queue-only mini view focuses the queue

- **WHEN** the terminal is narrower than 80 columns and the user presses
  `x` to switch from library-only to queue-only mini view
- **THEN** panel focus SHALL move to the queue panel

#### Scenario: Toggling to library-only mini view focuses the library

- **WHEN** the terminal is narrower than 80 columns and the user presses
  `x` to switch from queue-only to library-only mini view
- **THEN** panel focus SHALL move to the library panel

## MODIFIED Requirements

### Requirement: Queue-only renders the queue panel focused when it holds focus

In queue-only state — at any terminal width — the queue panel SHALL
render with the same focused styling it has when focused in the `both`
state whenever panel focus is on the queue: the focused background, cursor
highlight, and scrollbar. The prior behavior of always rendering the
queue-only panel with unfocused styling is removed.

#### Scenario: Queue-only renders focused when the queue holds focus

- **WHEN** the layout is in queue-only state and panel focus is on the
  queue
- **THEN** the queue panel SHALL render with its focused background,
  cursor highlight, and scrollbar

#### Scenario: Both keeps the focused appearance

- **WHEN** the layout is in `both` state and the queue panel is focused
- **THEN** the queue panel SHALL render with its focused background,
  cursor highlight, and scrollbar
