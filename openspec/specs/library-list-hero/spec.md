# library-list-hero Specification

## Purpose

Gives the library list a persistent, always-visible detail view of the
selected item — a compact banner pinned above the list — instead of
splicing detail rows inline with the scrolling list content.
## Requirements
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

### Requirement: Hero position

The library list MUST render the selected item's compact banner in a
dedicated panel positioned **directly below the row containing the
selected item**, not at the top of the content area and not inside
a cell. The list MUST wrap around the hero: items above the selected
row pack in the rows above the hero, and items below the selected
row pack in the rows below the hero.

The hero's width MUST be the full content area width. The hero's
height MUST be derived from the image's natural aspect ratio (16:9
in terminal cells), capped at 12 image rows, plus a 1-row gap and a
5-row meta block, for a total of 12 to 18 rows depending on the
content width.

#### Title row

In a two-column list, the hero MUST render the selected item's title
as its top row, in `palette::YELLOW` foreground (bold when focused),
pushing the poster and meta content down a row. In a one-column list
the hero MUST NOT render a title row (the full-width list-row title
directly above the hero already shows the name), keeping the
12-to-18-row height.

### Requirement: Hero follows the cursor

The hero's position MUST update as the cursor moves. When the cursor
moves up, the hero MUST move up to be below the new selected row.
When the cursor moves down, the hero MUST move down. When the cursor
is at the top row, the hero MUST be just below the top row. When the
cursor is at the bottom row, the hero MUST be just below the bottom
row (the section above the hero contains only the selected row, and
the section below the hero is empty).

#### Scenario: Cursor moves
- **WHEN** the cursor moves to another item
- **THEN** the hero SHALL move directly below the newly selected row

### Requirement: Row map reflects the hero

The row map (`left_row_map`) MUST have `None` entries for the display
rows occupied by the hero. A click on a hero row MUST hit the hero
(treated as an Enter equivalent on the selected item), not an item
in the list.

The row map's item entries MUST be at the display rows where they
actually paint:
- Top section: items 0 through the row containing the cursor
- Bottom section: items after the cursor's row, at display rows
  `top_section_height + hero_height` and below

#### Scenario: Hero rows are clicked
- **WHEN** the user clicks within the hero rows
- **THEN** the hit target SHALL resolve to the hero and not to a list item

### Requirement: Auto-scroll

The list's auto-scroll MUST keep the cursor and the hero both
visible. If the cursor + hero don't fit in the content area, the
list MUST scroll to bring both into view.

#### Scenario: Selected block exceeds the viewport
- **WHEN** the selected row and hero do not fit in the content area
- **THEN** scrolling SHALL bring the selected block into view

### Requirement: Hero interaction

A single click inside the hero area MUST only focus the library panel
(the cursor is already on the selected item). A double-click inside
the hero area MUST be treated as an Enter equivalent on the currently
selected item -- the same activation Enter and a double-click on the
selected row perform (playing a movie, entering a Series' season/
episode selection, etc.), so hero activation can't drift from the
single-click-only-focuses convention used everywhere else in the
library list. The hero is interactive.

#### Scenario: Hero double-click
- **WHEN** the user double-clicks inside the hero area
- **THEN** the application SHALL perform the same activation as Enter on the selected item

### Requirement: Invariant preserved

The maintenance rule that the library list is one renderer
parameterized by column count MUST still hold. The top section and
the bottom section MUST use the same packing logic, parameterized
by `cols`. The invariant test (1-col and 2-col render the same
per-cell content, modulo cell width) MUST still pass.

#### Scenario: Narrow and wide list modes
- **WHEN** the same list is rendered in one- and two-column modes
- **THEN** both sections SHALL use the same column-parameterized packing logic

### Requirement: Independence from top-hero design

This design is a positional variant of the top-hero design. The
hero's content (image, meta, overview) is identical. Only the
position changes. The top-hero branch is kept alongside for
comparison.

The positional variant MUST preserve the same hero content as the top-hero design.

#### Scenario: Hero content remains consistent
- **WHEN** an item is shown in the inline hero
- **THEN** its image, metadata, and overview SHALL match the top-hero content

### Requirement: Hero area above the list

The library list MUST render the selected item's compact banner in a
dedicated area at the top of the content area, not inline with the
list cells.

The hero area's height MUST be derived from the image's natural aspect
ratio (16:9 in terminal cells), capped at a maximum that leaves at
least 1 row for the list. The list area MUST contain all of the list
rows; the row renderer no longer reserves space for an inline banner.

The list renderer MUST receive the `list_area` (below the hero) as its
content area, not the full `content_area`. The column count is derived
from `list_area.width` using the same `library_column_count` helper and
`POWER_TWO_COLUMN_THRESHOLD` as before.

#### Scenario: Hero renders above list
- **WHEN** a library item is selected
- **THEN** its hero SHALL occupy the dedicated area above the list and the list SHALL render below it

