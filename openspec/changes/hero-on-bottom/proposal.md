# Move the library hero block to the bottom of the list

## Summary

The two-column library list currently paints the selected item's banner
inline, wedged into the cell grid so each cell carries its own piece of
the notched block. The result reads as "a list with a banner that pushes
the cells around" rather than "a list with a detail pane." This change
moves the hero out of the grid and onto a fixed panel below the list,
so the hero is one wide shape and the list above it is a clean 1-or-2-col
grid that never reflows when the cursor moves.

This supersedes `hero-on-top` (never merged to main; prototyped on
`try/hero-on-top`). Same underlying motivation — detach the hero from
inline flow — opposite edge. See "Why bottom, not top" in design.md.

## Motivation

```
   CURRENT (inline)                    THIS CHANGE (bottom hero)
   ┌──────────────────────┐            ┌──────────────────────┐
   │ Movie 1     Movie 2  │            │ Movie 1     Movie 2  │
   │ Movie 3   ▌Movie 4▐   │            │ Movie 3   > Movie 4 <│
   │ Movie 5     Movie 6  │            │ Movie 5     Movie 6  │
   │ Movie 7     Movie 8  │            │ Movie 7     Movie 8  │
   │   ↑ banner wedged    │            ├──────────────────────┤
   │     between cells    │            │ ▌Movie 4▐            │
   │                      │            │ 2024 · 1h 47m · ...  │
   └──────────────────────┘            │ Overview...          │
                                        └──────────────────────┘
```

Three things this buys:

1. **The hero stops moving.** Today, moving the cursor makes the hero
   block physically reflow — it re-wedges under whichever row is
   selected, and every row below it shifts. Pinning the hero to a fixed
   edge (top or bottom) means only its *content* updates on selection
   change; its screen position never moves. This is the primary win,
   and it's shared with `hero-on-top` — either edge fixes it.
2. **The list stays right under the header.** Bottom, specifically:
   nothing sits between the tab bar and the list you're scanning. A top
   hero pushes the whole list down past a poster on every frame.
3. **The hero reads as a preview pane, not a banner.** Detail-below-list
   is the Finder/ranger/Total Commander shape — selection info
   subordinate to the thing you're browsing — which matches how this
   hero is actually used (glance down to confirm, then move on).

## What stays the same

- 2-col packing, hjkl nav, `library_column_count`, `library_cell_rect`,
  `library_cell_slot`, `POWER_TWO_COLUMN_THRESHOLD`, the maintenance
  rule, the invariant test.
- The 1-col / 2-col split. The list above the hero is the same
  renderer, parameterized by `cols`.

## What goes away

- The notched block (tab + panel) inside the list.
- The `selected_block_bounds` machinery in `render_power_list`.
- The `compact_banner_rows` expansion of the selected row in 2-col mode.
- The series inline detail rows (the hero IS the series detail now).

## What changes

- `render_power_list` splits its `content_area` into `[list_area,
  hero_area]` (list on top, hero on the bottom edge) based on the
  selected item's banner height. The list renderer sees `list_area`;
  the hero is painted into `hero_area` afterward.
- `render_power_plain_rows` and `render_power_letter_grouped_rows` no
  longer compute or paint `selected_block_bounds`. The selected cell is
  a `▌` mark + `##` prefix, same list bg as every other cell.
- The compact banner layout gains a wider mode: the bottom hero uses
  the full content width, so the existing
  `compact_banner_layout_with_overview` is called with a larger
  `panel_width`. The overview gets more rows.

## Open design questions

- **Hero height.** Reuse the home view's `image_height = width * 9 /
  32` formula, capped so the list always keeps a few rows (see
  `hero-on-top`'s decision 3 — carries over unchanged).
- **What does clicking the hero do?** Enter equivalent — opens the
  selected item, same as `hero-on-top`.
- **Selected cell indicator.** `▌` mark + `##` prefix, same as
  `hero-on-top` decision 4 — no reason for this to differ by edge.

## Scope

- Plain list only (movies, series, podcasts, music album folders,
  homevideo list). NOT the music artist-group view or the feed home
  video group view — those have their own renderers and a different
  shape, and forcing a bottom hero on them would be a separate change.

## What this is not

- Not a refactor of the home view's hero.
- Not a unification of the home view and library view.
- Not a redesign of the music group view.
- Not a commitment. If this feels wrong in the terminal, it's reverted.

## Relation to `hero-on-top`

`hero-on-top`'s tasks.md shows implementation complete on
`try/hero-on-top` (never merged); its docs on `main` are stale relative
to that — worth archiving or deleting once this change's direction is
confirmed, so they stop reading as current intent.
