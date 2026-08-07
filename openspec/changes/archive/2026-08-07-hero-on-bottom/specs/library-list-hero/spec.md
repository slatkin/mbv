## Purpose

Gives the library list a persistent, always-visible detail view of the
selected item — a compact banner pinned above the list — instead of
splicing detail rows inline with the scrolling list content.

## ADDED Requirements

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

#### Scenario: Hero suppressed when too little space remains

- **WHEN** the content area is too short to fit even the hero's own
  minimum block size
- **THEN** the hero area collapses to zero height and the list uses
  the full content area

#### Scenario: Letter pills sit between hero and list

- **WHEN** the letter-pill row is shown for the current library view
- **THEN** the pill row renders directly below the hero with no gap,
  and the list renders below the pill row

### Requirement: Selected cell indicator

The selected cell in the list SHALL be identified by two visual
elements: a `▌` mark on the left edge of the cell, and a `##` prefix
in the title text. The cell's background SHALL use the ordinary list
background, not the media-selected background — that treatment is
reserved for the hero.

#### Scenario: Selected cell marked without a background change

- **WHEN** a cell in the list is the current selection
- **THEN** it shows a `▌` left-edge mark and a `##` title prefix, with
  the list's ordinary (non-selected) background

### Requirement: Hero tracks the current selection independent of scroll position

The hero SHALL always reflect the currently selected (cursor) item,
regardless of whether that item's row is scrolled into view within
the list area. The hero's own screen position SHALL NOT change when
the cursor moves — only its content.

#### Scenario: Selection scrolled out of view

- **WHEN** the cursor moves to an item whose row is scrolled off
  screen in the list area
- **THEN** the hero still updates to show that item's banner, in the
  same fixed screen position as before

### Requirement: Hero click focuses without activating

A single click inside the hero area SHALL behave the same as a click
anywhere else in the library pane: it focuses the Library panel only,
since the cursor is already on the selected item and there is nothing
else to move. Activation (playing a movie, entering a Series' season
selection) SHALL remain a double-click gesture, handled the same way
as any other library-row activation.

#### Scenario: Single click on the hero

- **WHEN** the user single-clicks inside the hero area
- **THEN** the Library panel gains focus and no item is activated

#### Scenario: Double click on the hero

- **WHEN** the user double-clicks inside the hero area
- **THEN** the selected item is activated the same as a double-click
  on its list row would be

### Requirement: Column-count invariant preserved

The library list SHALL remain one renderer parameterized by column
count: the list area at a 1-column width and the equivalent 2-column
width SHALL render the same per-cell content, modulo cell-width
truncation and the right cell's trailing-column absorption.

#### Scenario: Same content at the 1-col/2-col boundary

- **WHEN** the list area is rendered once at a width just below the
  two-column threshold and once at a width just above it
- **THEN** each cell's content matches between the two renders, aside
  from truncation and trailing-column absorption differences
