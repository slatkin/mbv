## Context

Selection background is copy-pasted, not shared. `selection_marker` (the `▎`
glyph) is already a shared component; the background never got the same treatment.
`queue.rs` is the correct reference: `SURFACE_FOCUSED`, full `area.width`.

## Decisions

### One primitive, in the palette/theme layer

Add `selected_row_background(f, panel: Rect, row_y: u16, focused: bool)` (name
provisional) beside the existing `resolve_surface_focus` / `selection_marker`
helpers. It paints one row of `resolve_surface_focus(focused)`-derived focused
surface across `panel`'s full inner width at `row_y`. Callers pass the **panel**
rect (list panel or inset box), not a pre-inset content rect — that is what makes
the highlight reach the edges uniformly.

- **Colour:** `SURFACE_FOCUSED` via the shared focus resolution. This is the one
  place the selection colour is chosen. Libraries currently use `SURFACE_RESTING`
  (a span-pad); they move to the block. This is the intended visual convergence,
  not a regression to preserve.
- **Geometry:** full panel width. Book browser's inset and album's art-reserved
  widths are dropped.

### Callers become one line

Each site replaces its hand-rolled `Block::default().style(bg(...))` + rect with a
call to the primitive:

| Site | Panel rect it passes |
|---|---|
| `queue.rs` | queue `area` (already correct — becomes the call) |
| `render_plain_rows` / `render_letter_grouped_rows` | the list content panel |
| `music_wide.rs` | `track_panel` (recessed box) |
| `audiobookshelf_book_browser.rs` | `list_panel` (not the inset `browser_area`) |
| `album.rs` / `album_detail.rs` | the row's panel |
| `selection_modal.rs` | the modal list panel |

Text/marker/columns are still drawn over the background afterward; order matters
(background first, then the row line).

### `item_cell_spans` loses its selection pad

The `SURFACE_RESTING` pad span for selected rows in `item_cell_spans` is deleted —
the block now provides the background, so the pad would double-paint a second
colour. Unselected padding stays raw.

## Risks

- Libraries visibly change colour (RESTING → FOCUSED). Expected; call it out in
  the PR so it is judged as correct, not flagged as a regression.
- Two-column library rows: the block must span the whole panel, not one column —
  verify the marker/right-column layout still reads against the filled row.
- Focus resolution must stay centralized; do not let any caller pass a literal
  colour.

## Migration

No data or protocol migration. Land the primitive + all call-site swaps together
so no surface is left on the old model mid-tree (partial migration would make the
drift worse, not better).
