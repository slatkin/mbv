# Design: hero on the bottom of the library list

## Why bottom, not top

`hero-on-top` (prototyped on `try/hero-on-top`, never merged) pinned
the hero above the list. Detaching the hero from inline flow — so it
stops reflowing every time the cursor moves — is the real fix, and
either edge delivers it equally. Bottom is preferred over top because:

- The list stays immediately below the tab bar. A top hero pushes the
  whole list down past a poster on every frame; a bottom hero doesn't
  touch what's between the header and the list.
- It reads as a preview/inspector pane subordinate to the list
  (Finder, ranger, Total Commander), which matches how it's actually
  used — glance down to confirm the selection, then keep moving.

Everything else below is `hero-on-top`'s design carried over with the
split direction flipped; no other decision changes by edge.

## Decisions

### 1. List area, then hero area

`render_power_list` splits its `content_area` vertically into two
stacked rects: `list_area` on top, `hero_area` on the bottom edge,
using the existing `hero_height_for_width` calculation (unchanged) to
size `hero_area`. This replaces today's mechanism, where the row
renderer takes the *full* `content_area` and `render_power_list` passes
a `hero_rows` count through `ListRenderCtx` for the row renderer to
leave as blank `DisplayRow::Hero` filler rows immediately below the
cursor's row. Here there's no filler-row insertion at all: the row
renderer is simply given a shorter rect (`list_area`) and renders a
normal grid into it; the hero is painted into `hero_area` afterward,
same "paint last" step as today just at a fixed rect instead of
wherever the filler rows landed.

```
   content_area
   ┌──────────────────────┐
   │                      │
   │      list_area       │  height = content_area.height - h_hero
   │                      │
   ├──────────────────────┤
   │      hero_area       │  height = h_hero(content_area.width)
   └──────────────────────┘
```

The list renderer doesn't know about the hero. It just gets a rect and
renders rows into it. The 2-col / 1-col decision is unchanged —
`library_column_count(list_area.width)` drives the column count, with
the same `POWER_TWO_COLUMN_THRESHOLD` (82).

### 2. Hero height from the image's natural aspect ratio

Already implemented, not a new decision: `hero_height_for_width` in
`list.rs` derives the hero's height from the poster image, reusing the
home view's formula (16:9 aspect ratio in terminal cells, where cells
are roughly twice as tall as they are wide):

```
image_height = div_ceil(width * 9, 32), capped at HERO_IMAGE_CAP_ROWS (12)
```

Total hero rows = `image_height + title_rows + gap_rows + meta_rows +
block_extra_rows`, where (per the constants already in `list.rs`):
`HERO_TITLE_ROWS = 1` (2-col lists only — the yellow title row; 1-col
lists skip it since the row above the hero already shows the full
title), `HERO_GAP_ROWS = 1`, `HERO_META_ROWS = 5`,
`HERO_BLOCK_EXTRA_ROWS = 4` (the existing `▁`/`▔` border rows plus their
inner padding rows).

Because `HERO_IMAGE_CAP_ROWS` is only 12, and the uncapped formula
already exceeds 12 at any width above ~43 columns, **the image is
capped at every terminal width the list is realistically used at** —
the hero does not keep growing as the terminal gets wider, unlike what
`hero-on-top`'s original design doc assumed:

| content_width | cols | image_height | title | gap | meta | border/pad | total hero |
|---|---|---|---|---|---|---|---|
| 60 (1-col) | 1 | 12 (capped) | 0 | 1 | 5 | 4 | 22 |
| 82 (2-col kick-in) | 2 | 12 (capped) | 1 | 1 | 5 | 4 | 23 |
| 100 | 2 | 12 (capped) | 1 | 1 | 5 | 4 | 23 |
| 150 | 2 | 12 (capped) | 1 | 1 | 5 | 4 | 23 |

The hero is effectively a constant ~22-23 rows regardless of terminal
width. In a 60-col terminal (`list_area.height >= ~5` after subtracting
22 rows plus tab bar/borders) this can still be tight — see decision 3.

### 3. Image height cap: already decided, carried forward

`hero-on-top`'s design doc framed the cap as an open a/b/c choice; it's
no longer open — `HERO_IMAGE_CAP_ROWS = 12` is already shipped on
`main` and used by today's inline hero. Carry it forward unchanged
unless visual feedback at narrow widths (near 60 cols, where 22 hero
rows can leave very few list rows in a short terminal) says otherwise.
If it doesn't read well, the two documented fallbacks remain available:
cap the hero *width* instead (leaving more list rows), or suppress the
hero entirely below some minimum terminal width.

### 4. Selected cell indicator

Unchanged from `hero-on-top`, but a real change from what's on `main`
today: the current selected cell (`build_list_row_spans` in
list_rows.rs) uses a `▍` grabber mark plus the same
`PLAYBACK_PANEL_BG`/`MEDIA_SELECTED_BG` background as the inline hero
block, so the row and the hero read as one continuous selected block.
That coupling goes away here since the hero is no longer adjacent to
the selected row. The selected cell instead gets:

- `##` (2 cols) prefix in the title for selected cells.
- A `▌` mark on the left edge of the selected cell.
- The bg color is the same as the rest of the list (no special
  selected bg) — `MEDIA_SELECTED_BG` is reserved for the hero.

### 5. The hero always reflects the current selection, not scroll position

The hero is derived from `power_selected_movie_item` /
`power_selected_series_item` — the cursor's item — not from what's
visible in `list_area`. Moving the cursor off-screen (scrolling the
list) still updates the hero; the hero's own screen position never
moves. This is what makes decision-1's split actually deliver on the
"hero stops moving" motivation — call it out explicitly since nothing
in the row renderer enforces it structurally.

### 6. Click on the hero opens the selected item

Unchanged from `hero-on-top`. Clicking inside `hero_area` is an Enter
equivalent — it opens the selected item.

### 7. Series inline detail keeps its content, loses its inline position

The series detail (season pills + episode table, painted by
`render_series_inline_detail` and sized by `series_inline_detail_rows`
in `detail_series.rs`/`detail_series_view.rs`) is not deleted — #448
restored it after an earlier attempt (`hero-on-top`) dropped it and
regressed the ability to browse/play an episode without leaving the
list. It keeps rendering the same content, just into the fixed
`hero_area` instead of the inline slot below the cursor row.

The row-count reservation for it moves out of `render_power_list`'s
per-frame `hero_rows` calc (list.rs:244-263 today) since there's no
more "reserve N blank rows below the cursor" step — `hero_area`'s
height is `hero_height_for_width(...)` for a movie or
`series_inline_detail_rows(...)` for a series, same branch as today,
just used to size a fixed rect instead of a filler-row count.

### 8. Compact banner layout extends to wider widths

Unchanged from `hero-on-top`. `compact_banner_layout_with_overview`
already takes `panel_width` and scales; the bottom hero just calls it
with the hero's (larger) width. No change to the function itself.

## What changes in the code

```
┌─── Touch points ─────────────────────────────────────────────────┐
│  render_power_list (src/app/render/list.rs)                     │
│    - split content_area into [list_area, hero_area] using the   │
│      existing hero_height_for_width / series_inline_detail_rows │
│      calc (hero_area is the bottom slice, not inline)            │
│    - call the row renderer with list_area instead of            │
│      content_area, and drop the hero_rows field from             │
│      ListRenderCtx entirely (list_rows.rs)                       │
│    - paint the selected item's banner into hero_area, after      │
│      the list has rendered (same render_power_compact_detail /   │
│      render_series_inline_detail call as today, new rect)        │
│    - drop the DisplayRow::Hero variant, the ▁/▔ border paint     │
│      currently in render_power_list (list.rs:311-403), and the  │
│      hero_rows calc block (list.rs:244-263)                      │
│                                                                    │
│  render_power_plain_rows (src/app/render/list_plain.rs)          │
│  render_power_letter_grouped_rows (list_letter_groups.rs)        │
│    - remove the hero_rows param and DisplayRow::Hero filler-row  │
│      insertion (list_plain.rs:58-71, list_letter_groups.rs:104-117)│
│    - selected cell becomes ▌ + ## prefix, ordinary list bg        │
│      (build_list_row_spans in list_rows.rs currently uses a ▍    │
│      grabber + PLAYBACK_PANEL_BG bg — both go away)               │
│                                                                    │
│  render_power_compact_detail (src/app/render/detail.rs)          │
│    - already takes panel_width; just gets called with a larger   │
│      value from render_power_list, same as today                 │
│                                                                    │
│  input handling                                                  │
│    - mouse click in hero_area → Enter equivalent (already the    │
│      case today per input_mouse_dispatch.rs; just a different    │
│      rect now)                                                   │
│    - keyboard Enter unchanged (still opens selected item)        │
│                                                                    │
│  tests                                                           │
│    - list_tests.rs has 4 tests today (packing, letter buckets,   │
│      cursor wrap/clamp, mouse-click-selects-cell); none reference │
│      a notched block or an invariant test to drop -- both were   │
│      removed from list_tests.rs in #448 as "brittle"              │
│    - add new tests for the hero area split, anchored at the      │
│      bottom of content_area, and a fresh 1-col/2-col parity test │
└───────────────────────────────────────────────────────────────────┘
```

## What stays the same

- `library_column_count`, `library_cell_width`, `LIBRARY_COLUMN_GAP`,
  `POWER_TWO_COLUMN_THRESHOLD`.
- hjkl nav, the sidebar h→x, the lib search input.
- The maintenance rule: list above the hero is the same renderer
  parameterized by `cols`. There's no standing invariant test to
  preserve (see touch points above) — this change should add one
  comparing `list_area` content at width 81 and 82.
- The 2-col padding in `power_right_panel_content_area` (smaller left
  pad in 2-col mode).

## What is NOT in this change

- Home view refactor.
- Music group view changes.
- Feed home video group view changes.
- The home/list crossover at 82 cols.
- Any new design for the home view's hero (which has its own
  right-rail layout in wide mode).

## Open questions for the implementer

1. Image height cap: already 12 rows, already shipped (decision 3) —
   not open. Revisit only if visual feedback says the constant ~22-23
   row hero is wrong for this layout.
2. Hero meta line: what fields go in it? This is unchanged from what
   `compact_banner_layout_with_overview` already renders today for the
   inline hero — confirm it still reads well at the wider bottom-hero
   width, don't redesign it from scratch.
3. The `▌` mark — is it visible enough on its own, now that it's no
   longer paired with a matching-bg block right below it (today's `▍` +
   colored-bg combo makes the row and hero read as one shape)? Or do we
   need something more distinct once the hero is no longer adjacent to
   the selected row?
4. Keep, drop, or simplify the existing `▁`/`▔` `SEEK_TRACK` borders
   now that the hero sits at a fixed screen edge rather than inline
   between list rows — a fixed bottom edge may not need the same visual
   separation the inline version relied on.
