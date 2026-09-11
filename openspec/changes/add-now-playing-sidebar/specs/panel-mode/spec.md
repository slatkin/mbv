## MODIFIED Requirements

### Requirement: Library-only hides the queue column

In library-only state the library panel SHALL occupy the full window width and the queue column SHALL not be rendered. This state SHALL be the same as today's collapsed-queue behavior. Library-only SHALL render the right-column player strip, because the strip renders exactly when the queue column is hidden; the library's content area SHALL start below the strip.

#### Scenario: Full-width library

- **WHEN** the layout is in library-only state
- **THEN** the library list SHALL span the full window width

#### Scenario: Queue not rendered

- **WHEN** the layout is in library-only state
- **THEN** the queue list, playback card, and visualizer SHALL NOT be rendered

#### Scenario: Player strip renders in library-only

- **WHEN** the layout is in library-only state at a width of 80 columns or more, or the library panel is shown in narrow mini view
- **THEN** the playback panel SHALL render as the right-column strip below the tab bar

### Requirement: Queue-only hides the library column

In queue-only state the queue panel SHALL render across the full window width. The tab bar, library list, status bar, and right-column player strip SHALL NOT be rendered. When playback is active, the playback panel SHALL be rendered within the queue column (see `now-playing-sidebar` for the placement and idle rules); fully idle queue-only state SHALL omit it so its rows belong to the Queue.

#### Scenario: Full-width queue

- **WHEN** the layout is in queue-only state
- **THEN** the queue list SHALL span the full window width

#### Scenario: Right column not rendered

- **WHEN** the layout is in queue-only state
- **THEN** the tab bar, library list, status bar, and player strip SHALL NOT be rendered

#### Scenario: Playback panel rendered in left column

- **WHEN** the layout is in queue-only state and playback is active
- **THEN** the playback panel SHALL be rendered within the queue column layout

#### Scenario: Idle queue-only omits the panel

- **WHEN** the layout is in queue-only state and no transport is active
- **THEN** the playback panel SHALL NOT be rendered and its rows SHALL belong to the queue panel
