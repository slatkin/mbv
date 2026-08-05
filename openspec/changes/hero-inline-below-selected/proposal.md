# Try: hero inline, just below the selected item

## Summary

The third layout for the library list's hero block. Instead of either
(a) wedging the notched tab+panel into the 2-col grid (the original
2-col design) or (b) painting the hero at the top of the list with a
clean 1-or-2-col grid below (the `try/hero-on-top` design), the hero
lives **inline, just below the row containing the selected item**, and
the list wraps around it: items above the selected row pack normally,
the hero takes a full-width panel below that row, and items below
the selected row continue packing below the hero.

This is a *try*: the goal is to see whether moving the hero inline
feels better than the top-hero design. The user said the top-hero
design "looks good" but wanted to test this third option. If this
feels worse, it's reverted and the top-hero design stays.

## Motivation

```
   TOP HERO (hero-on-top)        INLINE (this change)
   ┌──────────────────────┐      ┌──────────────────────┐
   │ ▌Movie 4▐            │      │ Movie 1     Movie 2  │
   │ 2024 · 1h 47m        │      │ Movie 3   ▌Movie 4▐   │
   │ Overview...          │      │ ──────────────────── │
   ├──────────────────────┤      │ ▌ Movie 4 hero ▐     │
   │ Movie 1     Movie 2  │      │ 2024 · 1h 47m        │
   │ Movie 3   > Movie 4 <│      │ Overview...          │
   │ Movie 5     Movie 6  │      │ ──────────────────── │
   │ Movie 7     Movie 8  │      │ Movie 5     Movie 6  │
   └──────────────────────┘      │ Movie 7     Movie 8  │
                                 └──────────────────────┘
```

Three things this buys over the top-hero design:

1. **The hero is right next to the selected item.** Less eye travel
   between the list cursor and the hero. The "what am I looking at"
   connection is closer.
2. **The hero follows the cursor.** As the user moves down the list,
   the hero moves with them. The top hero stays fixed at the top
   regardless of cursor position.
3. **The list is "one piece" with the hero.** No "hero on top, list
   below" visual separation — they're interleaved.

## What this is not

- Not a return to the original 2-col notched-block design. The notched
  block had a tab (cell-width narrowing at the top) + panel (full-width
  below). This design has only a panel — no tab. The selected cell
  uses the same `▌` + `##` marker as the top-hero design.
- Not a refactor of the top-hero design. The top-hero branch stays
  alongside this one for comparison.

## What stays the same

- 2-col packing, hjkl nav, `library_column_count`, `library_cell_rect`,
  `POWER_TWO_COLUMN_THRESHOLD`, the maintenance rule.
- The hero's content (image + meta + overview) — same
  `compact_banner_layout_with_overview`, same image cap (12 rows), same
  meta block (5 rows), same 1-row gap.
- The selected cell indicator (`▌` + `## `).
- Click on the hero = Enter equivalent.
- The invariant test pattern: the list below the hero is the same
  renderer parameterized by `cols`.

## What changes from the top-hero design

- The `hero_area` is no longer at the top of `content_area`. It is
  positioned **after the display row containing the selected item**.
- The list splits into two render passes: items above the selected row
  (top section) and items below the selected row (bottom section).
  Each section packs rows in 1 or 2 cols as usual.
- The row map (`left_row_map` / `left_row_targets`) reflects the
  inserted hero rows. Items below the selected row are at higher
  display-row indices than they would be in a flat list.
- `render_power_list` is restructured to:
  1. Compute the selected item's display row (where the hero goes).
  2. Render the top section (items above the selected row).
  3. Paint the hero.
  4. Render the bottom section (items below the selected row).
- `layout.left_area` is the full content area (not the area below the
  hero). `layout.hero_area` is the rect below the selected row.

## Open design questions

1. **Where exactly does the hero go?** Just below the row containing
   the cursor (the user moves cursor, hero follows). Not above the
   row, not in the middle of the row — directly below, full width.
2. **What if the selected item is the last row?** The hero appears
   below the last row. The list below the hero is empty. The hero
   is still painted.
3. **What if the hero is taller than the available content area?**
   The hero is clipped to the content area's height. The list
   sections are squeezed (possibly to 0 rows).
4. **What about the cursor going off the visible area?** The current
   design auto-scrolls the list so the cursor stays visible. With
   the hero inline, the "visible area" is reduced by `hero_height`.
   The auto-scroll must account for the hero's height when
   positioning the cursor.
5. **The `▌` + `## ` selected cell marker** — keep it the same as the
   top-hero design. The hero provides the visual identification
   through proximity, not through a tab.
6. **Should the hero move smoothly with the cursor, or jump?** For
   the first pass, jump. Smooth scrolling is a separate enhancement.

## Scope

- Plain list only (movies, series, podcasts, music album folders,
  homevideo list). NOT the music artist-group view or the feed home
  video group view.

## What this is not

- Not a refactor of the home view's hero.
- Not a unification of the home view and library view.
- Not a redesign of the music group view.
- Not a commitment. If this feels wrong in the terminal, it's reverted
  to the top-hero design (or to the original 2-col notched block).

## The third branch

This is a *third* branch, separate from the other two. The
comparison:

- `feature/two-column-library-list` (PR #447 open): the original 2-col
  design with notched block inline.
- `try/hero-on-top`: hero at the top of the list, 1-or-2-col list
  below. User said "looks good with some styling."
- `try/hero-inline-below-selected` (this change): hero inline, just
  below the selected row, list wraps around it. Trying to see if it's
  better than the top-hero.
