# Try: move the library hero block to the top of the list

## Summary

The two-column library list currently paints the selected item's banner
inline, wedged into the cell grid so each cell carries its own piece of
the notched block. The result reads as "a list with a banner that pushes
the cells around" rather than "a list with a hero on top." This change
moves the hero out of the grid and onto a fixed panel above the list, so
the hero is one wide shape and the list is a clean 1-or-2-col grid
below it.

This is a *try*: the goal is to see whether the design feels better than
the inline version. If it doesn't, the change is reverted and the inline
design stays. Nothing about the 2-col packing, the hjkl nav, the shared
threshold, or the maintenance rule changes.

## Motivation

```
   CURRENT (inline)                    THIS CHANGE (top hero)
   ┌──────────────────────┐            ┌──────────────────────┐
   │ Movie 1     Movie 2  │            │ ▌Movie 4▐            │
   │ Movie 3   ▌Movie 4▐   │            │ 2024 · 1h 47m · ...  │
   │ Movie 5     Movie 6  │            │ Overview...          │
   │ Movie 7     Movie 8  │            ├──────────────────────┤
   │   ↑ banner wedged    │            │ Movie 1     Movie 2  │
   │     between cells    │            │ Movie 3   > Movie 4 <│
   │                      │            │ Movie 5     Movie 6  │
   └──────────────────────┘            └──────────────────────┘
```

Three things this buys:

1. **Cells become simple.** No more inline banner pushing cells around.
   Each cell is title + meta + bg, full stop. Row-major packing is
   easier to reason about.
2. **The hero gets the full content width.** The current 40-col inline
   banner aggressively truncates overview text. A top hero at 80+ cols
   has room for the full overview, plus a longer meta line.
3. **Home and library become the same shape.** The home view already
   uses "hero on top, list below" in narrow mode and "hero on left,
   list on right" in wide mode. Library adopting "hero on top" makes
   the two views parallel — easier mental model, opportunity to share
   hero painting code later.

## What stays the same

- 2-col packing, hjkl nav, `library_column_count`, `library_cell_rect`,
  `library_cell_slot`, `POWER_TWO_COLUMN_THRESHOLD`, the maintenance
  rule, the invariant test.
- The 1-col / 2-col split. The list below the hero is the same
  renderer, parameterized by `cols`.

## What goes away

- The notched block (tab + panel) inside the list.
- The `selected_block_bounds` machinery in `render_power_list`.
- The `compact_banner_rows` expansion of the selected row in 2-col mode.
- The series inline detail rows (the hero IS the series detail now).

## What changes

- `render_power_list` splits its `content_area` into `[hero_area, list_area]`
  based on the selected item's banner height. The hero is painted into
  `hero_area`; the list renderer sees `list_area`.
- `render_power_plain_rows` and `render_power_letter_grouped_rows` no
  longer compute or paint `selected_block_bounds`. The selected cell is
  a bg highlight + a small indicator (a `▌` mark or `##` prefix).
- The compact banner layout gains a wider mode: the top hero uses the
  full content width, so the existing `compact_banner_layout_with_overview`
  is called with a larger `panel_width`. The overview gets more rows.

## Open design questions

- **Hero height.** A fixed height (e.g. 6 rows) is simplest, but the
  image's natural aspect ratio (16:9 in terminal cells) makes the hero
  taller when the content area is wider. Reusing the home view's
  `image_height = width * 9 / 32` formula gives a hero that scales with
  the available width.
- **What does clicking the hero do?** Options: (a) inert — hero just
  reflects selection; (b) Enter equivalent — opens the selected item.
  (b) gives the hero a purpose beyond decoration.
- **Selected cell indicator.** With no tab, the selected cell needs
  something. Options: bg highlight only (subtle), `▌` mark on the left,
  `##` prefix in the title. The 2-col inline design had `##` + bg; the
  simplest move is to keep both but drop the banner that came after.

## Scope

- Plain list only (movies, series, podcasts, music album folders,
  homevideo list). NOT the music artist-group view or the feed home
  video group view — those have their own renderers and a different
  shape, and forcing "hero on top" on them would be a separate change.

## What this is not

- Not a refactor of the home view's hero.
- Not a unification of the home view and library view (just a step
  toward one).
- Not a redesign of the music group view.
- Not a commitment. If this feels wrong in the terminal, it's reverted.
