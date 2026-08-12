## MODIFIED Requirements

### Requirement: Hero area pinned above the list

The library list SHALL render the selected item's compact banner in a dedicated area pinned to the top of the content area, not inline with the list cells. The list area SHALL occupy the remaining space below the hero (and below the letter-pill row, when shown), and SHALL NOT scroll the hero out of view.

The hero area's height SHALL be derived from the banner's actual content (meta line, overview/director text, poster height), not a width-derived aspect-ratio guess, capped at a maximum that leaves at least 1 row for the list. Below that minimum the hero SHALL be suppressed entirely (zero height) rather than painted malformed.

For grouped Music below the shared wide-layout breakpoint, the selected album hero SHALL use this same pinned-above-list composition. At or above the breakpoint, grouped Music SHALL instead use the side hero and right-rail browser defined by `music-library-hero`; this is a Music-specific responsive exception and SHALL NOT affect other library types.

When the letter-pill row is shown (per `tv-letter-filtering`), it SHALL occupy a dedicated row directly below the hero, with no additional gap between them. When the pill row is not shown, the hero and list SHALL be separated by a single blank row.

The list renderer SHALL receive the resulting `list_area` (below the hero and pill row) as its content area, not the full content area. The column count is derived from `list_area`'s width using the same column-count threshold as before, except that wide grouped Music uses its capability-specific one-column right rail.

#### Scenario: Hero renders above the list
- **WHEN** a movie library's top-level view has a selected item
- **THEN** the hero banner for that item renders in a fixed-height area at the top of the content area, and the list renders below it

#### Scenario: Narrow grouped Music uses the pinned hero
- **WHEN** grouped Music is below the shared wide-layout breakpoint
- **THEN** its selected album hero renders above its one-column album list

#### Scenario: Wide grouped Music uses its side hero
- **WHEN** grouped Music reaches the shared wide-layout breakpoint
- **THEN** its selected album hero moves to the left of its one-column album browser as defined by `music-library-hero`

#### Scenario: Hero suppressed when too little space remains
- **WHEN** the content area is too short to fit even the hero's own minimum block size
- **THEN** the hero area collapses to zero height and the list uses the full content area

#### Scenario: Letter pills sit between hero and list
- **WHEN** the letter-pill row is shown for the current library view
- **THEN** the pill row renders directly below the hero with no gap, and the list renders below the pill row
