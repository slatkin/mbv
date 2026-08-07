## MODIFIED Requirements

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
