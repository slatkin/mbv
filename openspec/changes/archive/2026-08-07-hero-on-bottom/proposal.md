# Move the library hero block to the bottom of the list

## Summary

The two-column library list already paints the selected item's banner as
a full-width block, not wedged into individual cells — but it's inserted
*inline*, directly below whatever row holds the cursor (`list.rs`'s
`hero_rows` / `DisplayRow::Hero` mechanism, landed in #448). The block
itself already has borders and reads as a detail panel; the problem is
that it physically relocates every time the cursor moves to a different
row, and everything below it reflows to make room. This change keeps
the same block and the same content (`render_power_compact_detail`,
`render_series_inline_detail`) but pins it to a fixed panel at the
bottom of the content area instead of inserting it after the cursor
row, so the list above it is a clean 1-or-2-col grid whose height never
changes as the cursor moves.

This supersedes `hero-on-top` (never merged to main; prototyped on
`try/hero-on-top`, and briefly tried as one of three layouts explored
within #448 before the team settled on the inline-below-cursor version
that's live today). Same underlying motivation — detach the hero from
cursor-relative flow — opposite edge. See "Why bottom, not top" in
design.md.

## Motivation

```
   CURRENT (inline, below cursor)       THIS CHANGE (bottom hero)
   ┌──────────────────────┐             ┌──────────────────────┐
   │ Movie 1     Movie 2  │             │ Movie 1     Movie 2  │
   │▍Movie 3     Movie 4  │             │ Movie 3   > Movie 4 <│
   ├──────────────────────┤             │ Movie 5     Movie 6  │
   │  hero: Movie 3 detail│             │ Movie 7     Movie 8  │
   │  poster/meta/overview│             ├──────────────────────┤
   ├──────────────────────┤             │ hero: Movie 4 detail │
   │ Movie 5     Movie 6  │             │ poster/meta/overview │
   │ Movie 7     Movie 8  │             └──────────────────────┘
   └──────────────────────┘
      ↑ hero re-inserts itself
        under whichever row is cursored
```

Three things this buys:

1. **The hero stops moving.** Today, moving the cursor makes the hero
   block physically relocate — it's re-inserted below whichever row is
   selected, and every row below it shifts up or down. Pinning the hero
   to a fixed edge (top or bottom) means only its *content* updates on
   selection change; its screen position never moves. This is the
   primary win, and it's shared with `hero-on-top` — either edge fixes
   it.
2. **The list stays right under the header.** Bottom, specifically:
   nothing sits between the tab bar and the list you're scanning. A top
   hero pushes the whole list down past a poster on every frame.
3. **The hero reads as a preview pane, not a banner.** Detail-below-list
   is the Finder/ranger/Total Commander shape — selection info
   subordinate to the thing you're browsing — which matches how this
   hero is actually used (glance down to confirm, then move on).

## What stays the same

- 2-col packing, hjkl nav, `library_column_count`, `library_cell_width`,
  `POWER_TWO_COLUMN_THRESHOLD`, the maintenance rule.
- The hero's row-height formula: `hero_height_for_width` and its
  constants (`HERO_IMAGE_CAP_ROWS`, `HERO_GAP_ROWS`, `HERO_META_ROWS`,
  `HERO_TITLE_ROWS`, `HERO_BLOCK_EXTRA_ROWS`) in `list.rs`. Nothing about
  *how tall* the hero is needs to change, only *where* it sits.
- The 1-col / 2-col split. The list above the hero is the same
  renderer, parameterized by `cols`.

Note: there is no standing "invariant test" comparing 1-col/2-col output
on `main` today — `one_and_two_column_render_the_same_per_cell_content`
was added in #448 and deliberately deleted in the same PR as one of nine
"brittle" layout-internals tests. This change should add a fresh one
(see tasks.md), not assume one exists to update.

## What goes away

- The `hero_rows` / `DisplayRow::Hero` filler-row insertion in
  `render_power_plain_rows` (list_plain.rs) and
  `render_power_letter_grouped_rows` (list_letter_groups.rs) that
  reserves blank rows directly below the cursor's display row.
- The `▁`/`▔` border-and-fill paint in `render_power_list` (list.rs)
  that currently paints the hero into those blank rows at whatever y
  the cursor put them.
- The `▍` grabber mark + `PLAYBACK_PANEL_BG` selected-cell background
  (`build_list_row_spans` in list_rows.rs) — replaced by the `▌` mark +
  `##` prefix below.
- The series inline detail rows (`series_inline_detail_rows` /
  `render_series_inline_detail`) as a *separate* code path — the
  content itself is kept, just painted into the fixed `hero_area`
  instead of an inline block.

## What changes

- `render_power_list` splits its `content_area` into `[list_area,
  hero_area]` (list on top, hero on the bottom edge) using the existing
  `hero_height_for_width` calculation — same formula and constants,
  computed once up front instead of being threaded through
  `ListRenderCtx.hero_rows` as filler rows. The list renderer sees
  `list_area`; the hero is painted into `hero_area` afterward, same as
  today's "paint hero last" step just at a fixed rect.
- `render_power_plain_rows` and `render_power_letter_grouped_rows` no
  longer take a `hero_rows` parameter or emit `DisplayRow::Hero` filler
  rows — every display row is a real item/header/spacer. The selected
  cell becomes a `▌` mark + `##` prefix, same list bg as every other
  cell (no `MEDIA_SELECTED_BG` or `PLAYBACK_PANEL_BG` on the cell
  itself).
- The compact banner layout gains a wider mode: the bottom hero uses
  the full content width, so the existing
  `compact_banner_layout_with_overview` is called with a larger
  `panel_width`. The overview gets more rows.

## Open design questions

- **Hero height.** Already implemented and shipped: `hero_height_for_width`
  in list.rs caps the image at `HERO_IMAGE_CAP_ROWS` (12 rows), which in
  practice means the hero is a *constant* height (~22-23 rows) at every
  terminal width wide enough to use the list at all — it does not keep
  growing with terminal width the way `hero-on-top`'s original design
  doc assumed. No new height decision needed; see design.md decision 2.
- **What does clicking the hero do?** Enter equivalent — opens the
  selected item, same as `hero-on-top` and same as today's inline hero.
- **Selected cell indicator.** `▌` mark + `##` prefix, same as
  `hero-on-top` decision 4. This is a change from what's on `main`
  today (`▍` grabber + colored background), not a no-op.
- **Does the hero need a top border now?** The current inline hero
  already paints `▁`/`▔` borders (`SEEK_TRACK`) around itself. Once
  it's pinned to a fixed bottom edge, decide whether to keep both
  borders, keep just the top one (separating it from the list), or drop
  them now that a fixed screen position makes the block's edges less
  ambiguous.

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

## Relation to `hero-on-top` and `hero-inline-below-selected`

`openspec/changes/hero-on-top/` and
`openspec/changes/hero-inline-below-selected/` are both still committed
on `main`. `hero-inline-below-selected` documents the design that
actually shipped (#448, the mechanism this proposal replaces) and is
the more useful reference for "what's live today" than `hero-on-top`,
which was tried and abandoned within the same PR before the inline
version was chosen. `hero-on-top`'s tasks.md shows implementation
complete on `try/hero-on-top` (never merged); its docs on `main` are
stale relative to what shipped — worth archiving or deleting once this
change's direction is confirmed, so they stop reading as current
intent.
