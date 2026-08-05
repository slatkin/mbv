# Library list: hero inline, just below the selected item

## ADDED Requirements

### Hero position

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

### Hero follows the cursor

The hero's position MUST update as the cursor moves. When the cursor
moves up, the hero MUST move up to be below the new selected row.
When the cursor moves down, the hero MUST move down. When the cursor
is at the top row, the hero MUST be just below the top row. When the
cursor is at the bottom row, the hero MUST be just below the bottom
row (the section above the hero contains only the selected row, and
the section below the hero is empty).

### Row map reflects the hero

The row map (`left_row_map`) MUST have `None` entries for the display
rows occupied by the hero. A click on a hero row MUST hit the hero
(treated as an Enter equivalent on the selected item), not an item
in the list.

The row map's item entries MUST be at the display rows where they
actually paint:
- Top section: items 0 through the row containing the cursor
- Bottom section: items after the cursor's row, at display rows
  `top_section_height + hero_height` and below

### Auto-scroll

The list's auto-scroll MUST keep the cursor and the hero both
visible. If the cursor + hero don't fit in the content area, the
list MUST scroll to bring both into view.

### Selected cell indicator

The selected cell in both the top and bottom sections MUST be
identified by a `▌` mark on the left edge of the cell and a `##`
prefix in the title text. The cell's background MUST use the
ordinary list background, not `MEDIA_SELECTED_BG`.

### Hero interaction

A mouse click inside the hero area MUST be treated as an Enter
equivalent on the currently selected item. The hero is interactive.

### Invariant preserved

The maintenance rule that the library list is one renderer
parameterized by column count MUST still hold. The top section and
the bottom section MUST use the same packing logic, parameterized
by `cols`. The invariant test (1-col and 2-col render the same
per-cell content, modulo cell width) MUST still pass.

### Independence from top-hero design

This design is a positional variant of the top-hero design. The
hero's content (image, meta, overview) is identical. Only the
position changes. The top-hero branch is kept alongside for
comparison.
