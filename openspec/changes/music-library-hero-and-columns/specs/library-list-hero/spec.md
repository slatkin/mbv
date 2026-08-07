## MODIFIED Requirements

### Requirement: Hero area pinned above the list

The library list SHALL render the selected item's compact banner in a
dedicated area pinned to the top of the content area, not inline with
the list cells. The list area SHALL occupy the remaining space below
the hero (and below the letter-pill row, when shown), and SHALL NOT
scroll the hero out of view.

The hero area's height SHALL be derived from the banner's actual
content (meta line, overview/director text, poster height), not a
width-derived aspect-ratio guess, capped at a maximum that leaves at
least 1 row for the list. Below that minimum the hero SHALL be
suppressed entirely (zero height) rather than painted malformed.

For music libraries at the album-browsing level, the hero SHALL size
to the selected album's expanded block (album art, metadata, and track
list). The hero content branch for albums is a third case alongside
movies and series.

When the letter-pill row is shown (per `tv-letter-filtering`), it
SHALL occupy a dedicated row directly below the hero, with no
additional gap between them. When the pill row is not shown, the hero
and list SHALL be separated by a single blank row.

The list renderer SHALL receive the resulting `list_area` (below the
hero and pill row) as its content area, not the full content area. The
column count is derived from `list_area`'s width using the same
column-count threshold as before.

#### Scenario: Hero renders above the list

- **WHEN** a movie library's top-level view has a selected item
- **THEN** the hero banner for that item renders in a fixed-height area
  at the top of the content area, and the list renders below it

#### Scenario: Hero renders for a selected album in a music library

- **WHEN** a music library with levels has a selected album at the album-browsing level
- **THEN** the hero shows the album's expanded detail (art, metadata, track list) and the list renders below it

#### Scenario: Hero suppressed when too little space remains

- **WHEN** the content area is too short to fit even the hero's own
  minimum block size
- **THEN** the hero area collapses to zero height and the list uses
  the full content area

#### Scenario: Letter pills sit between hero and list

- **WHEN** the letter-pill row is shown for the current library view
- **THEN** the pill row renders directly below the hero with no gap,
  and the list renders below the pill row
