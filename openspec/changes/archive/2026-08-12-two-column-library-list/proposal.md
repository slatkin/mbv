## Why

The Power View library list renders one column of items regardless of terminal width. The list pane already receives all width left over after the queue column (`src/app/render/mod.rs:256-261`), so on a wide terminal a 40-column list of album titles sits in a 160-column pane. The unused width is wasted, and long libraries scroll further than they need to.

The obvious fix — flow items into two columns — has two failure modes that must be avoided. Column-major flow (`ls`-style) makes an item's position depend on viewport height, which forces paging and destroys the inline detail banner, because a full-width band cannot cut coherently through two columns whose vertical axes are unrelated. And letting the selected item consume its neighbour's slot makes every item below the cursor swap columns on each keypress.

Row-major flow with full-width filler rows avoids both. The cursor stays a flat item index, the scroll offset stays a display-row index, and the existing inline banner machinery is reused unchanged.

## What Changes

- Pack library list item rows two-per-line, row-major (item `i` occupies column `i % cols`), when the list pane is wide enough; fall back to one column otherwise.
- Derive the column count from the *list pane* width rather than the terminal width, so it responds to the queue column's width and to `queue_column_collapsed`.
- Keep letter headers, the inline movie banner, and inline series detail as full-width rows spanning all columns, inserted below the pair row containing the cursor — the existing behaviour, unchanged.
- Render the selected block as a notch rather than a rectangle: the selected cell's slot at pair-row height, joined seamlessly to the full-width filler below it, so the selection reads as a tab attached to its detail panel.
- Pack each alphabetical letter bucket independently, so a pair row never straddles a bucket boundary.
- Extend cursor movement so left/right move by one item and up/down move by one row, without changing the flat cursor representation.
- Keep cursor visibility clamped on the whole selected block, so the tab can never scroll away from its panel.

## Capabilities

### New Capabilities

- `library-list-columns`: the Power View library list flows items into multiple columns on wide panes while preserving its existing scroll, selection, and inline-detail behaviour.

### Modified Capabilities

None. Existing library list content, sort order, filtering, and playback behaviour are unchanged.

## Impact

- **Code**: `src/app/render/list.rs`, `list_rows.rs`, `list_plain.rs`, `list_letter_groups.rs`, the selected-block background helper in `src/app/render/mod.rs`, cursor movement in `src/app/lib_cursor_actions.rs`, and key handling in `src/app/input_lib_power_keys.rs`. New column-geometry helper alongside `src/app/queue_column_width.rs`.
- **Behavior**: On wide panes the library list shows two items per line. Collapsing or resizing the queue column can change the column count live. Narrow panes are unchanged.
- **Data/API**: None.
- **Risk**: Medium-high. Power View list layout is historically fragile; the scroll clamp is duplicated between the plain and letter-grouped renderers, and both must stay in step. Requires visual verification in a real terminal, not build/tests alone.

## Non-Goals

- Column-major (`ls`-style) flow, and any paged scrolling model.
- More than two columns.
- Replacing the inline detail banner with a side detail pane (the Home tab's model).
- Unifying the existing four-wide season grid (`is_viewing_season_grid`) with this column machinery.
- Any user-facing setting to force a column count.
