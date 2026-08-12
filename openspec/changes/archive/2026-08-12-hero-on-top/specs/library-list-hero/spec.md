# Library list: hero on top

## ADDED Requirements

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
