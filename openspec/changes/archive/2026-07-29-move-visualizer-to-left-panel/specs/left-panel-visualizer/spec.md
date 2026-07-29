## ADDED Requirements

### Requirement: Left panel visualizer strip

The system SHALL render an audio visualizer strip at the bottom of the left panel (queue/card column) in the main view when the visualizer is enabled. The strip SHALL use the same height as the right-panel visualizer (11 rows) and SHALL display the same `visualizer_frame` data. The visualizer SHALL render within the queue panel's existing bounds — the queue list content area SHALL be reduced to make room, but the card area and overall panel dimensions SHALL remain unchanged. The existing right-panel visualizer SHALL remain unchanged.

#### Scenario: Visualizer enabled with left panel visible

- **WHEN** `visualizer_enabled` is true and the queue column is not collapsed
- **THEN** the left panel SHALL display a visualizer strip at its bottom, 11 rows tall, rendering bars from `visualizer_frame`

#### Scenario: Visualizer disabled

- **WHEN** `visualizer_enabled` is false
- **THEN** no visualizer strip SHALL render in the left panel; the left panel content SHALL use the full available height

#### Scenario: Queue column collapsed

- **WHEN** the queue column is collapsed (`queue_column_collapsed` is true)
- **THEN** no left-panel visualizer SHALL render (the entire left panel is hidden)
