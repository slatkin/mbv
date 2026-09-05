## MODIFIED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, optional trailing text, a media kind (`Collection` for navigable containers, `Media` for playable leaves), an optional duration string, and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`, or disabled), plus non-selectable Heading and Spacer rows. Heading and Spacer SHALL be excluded from selectable-target indexing. When a duration is shown it SHALL use the precise `M:SS`/`H:MM:SS` form (queue format, e.g. `4:32`, `1:02:03`); `Collection` rows SHALL NOT carry a duration. The model SHALL contain no provider client, `App`, source/header, raw style, callback, breakpoint, or effect.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Structural rows are displayed only
- **WHEN** a Heading or Spacer is rendered
- **THEN** it occupies display geometry
- **AND** it cannot be selected or activated

#### Scenario: Durations share one precise format
- **WHEN** any media list shows a duration (queue, home, feeds, TV episode, music track, book chapter)
- **THEN** every row uses the same `M:SS`/`H:MM:SS` format
- **AND** imprecise forms (`4m`, `1h12m`, unbounded `62:03`) never appear in list rows

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string
- **AND** the painter suppresses the duration slot even if one is projected
