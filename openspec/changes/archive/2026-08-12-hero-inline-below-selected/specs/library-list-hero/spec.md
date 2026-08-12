# Library list: hero inline, just below the selected item

## ADDED Requirements

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
