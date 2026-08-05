# Library list: hero on top

## ADDED Requirements

### Hero area above the list

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

### Selected cell indicator

The selected cell in the list below the hero MUST be identified by
two visual elements: a `▌` mark on the left edge of the cell, and a
`##` prefix in the title text. The cell's background MUST use the
ordinary list background, not `MEDIA_SELECTED_BG` — the selected bg
is reserved for the hero.

### Hero interaction

A mouse click inside the hero area MUST be treated as an Enter
equivalent on the currently selected item. The hero is interactive,
not just a reflection of the list selection.

### Invariant preserved

The maintenance rule that the library list is one renderer
parameterized by column count MUST still hold. The invariant test
(one-and-two-column render the same per-cell content) MUST still pass
when updated to compare the `list_area` content at width 81 and 82.
