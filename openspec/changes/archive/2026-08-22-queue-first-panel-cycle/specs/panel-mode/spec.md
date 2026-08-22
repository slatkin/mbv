## MODIFIED Requirements

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

### Requirement: Mini view replaces the cycle below 80 columns

When the terminal is narrower than 80 columns, the Power View layout SHALL use a separate two-state "mini view" instead of the three-state both/library-only/queue-only cycle: only the library panel or only the queue panel is shown, full width. Pressing `x` SHALL toggle between these two states. Mini view SHALL start at queue-only whenever it is first shown, including application start or when the terminal narrows below 80 columns after being wide, and SHALL NOT be persisted across sessions.

Mini view's last-shown panel SHALL be tracked independently of the three-state `panel_mode` used at 80+ columns. Widening the terminal back to 80+ columns SHALL restore whatever `panel_mode` and panel focus were active before the terminal narrowed, unchanged by any mini-view toggling that happened while narrow.

#### Scenario: Narrow terminal starts in library-only mini view

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
