# Spec Delta

## MODIFIED Requirements

### Requirement: Shared rows are provider-neutral and bounded
The controls SHALL accept selectable item rows with stable opaque targets, primary text, an optional
secondary title (the container name of a split row), an optional metadata slot, a media kind
(`Collection` for navigable containers, `Media` for playable leaves), an optional duration string,
and semantic state (ordinary, played, active with optional bounded integer progress `0..=100`,
now-playing with optional bounded integer progress `0..=100`, or disabled), plus non-selectable
Heading and Spacer rows. Feed entries and `Collection` rows SHALL NOT carry a duration. Heading and
Spacer SHALL be excluded from selectable-target indexing. `Active` SHALL mean that playing the item
would resume from a saved position, as `playback-resume` defines for rows; `NowPlaying` SHALL mean
live playback with live progress. A now-playing row with unknown
runtime SHALL show no progress. Only selectable Item rows SHALL carry a release year or publish
date. The model SHALL contain no provider client, `App`, source/header, raw style, callback,
breakpoint, or effect.

#### Scenario: Queue-like progress is presented safely
- **WHEN** a parent supplies active progress
- **THEN** the control receives only a bounded percentage
- **AND** playback and queue authority remain with the parent/shell

#### Scenario: Active follows the resume decision
- **WHEN** a parent projects a played item whose saved position Play would resume from
- **THEN** the row carries active state, not played state

#### Scenario: Now-playing with unknown runtime
- **WHEN** a now-playing row carries no progress
- **THEN** no percentage renders

#### Scenario: Structural rows are displayed only
- **WHEN** a Heading or Spacer is rendered
- **THEN** it occupies display geometry
- **AND** it cannot be selected or activated

#### Scenario: Library list rows carry no duration
- **WHEN** a library list row is projected (Home, Podcast, feed entry)
- **THEN** it carries no duration string

#### Scenario: Collections stay duration-free
- **WHEN** a row is a navigable container (movie/series folder, album, show, book title)
- **THEN** it carries no duration string

#### Scenario: Only Item rows carry dates
- **WHEN** a Heading or Spacer row is rendered
- **THEN** it carries no release year or publish date
