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
stacked rects: `list_area` on top, `hero_area` on the bottom edge. The
row renderer is given `list_area`; the hero is painted into
`hero_area` afterward.

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

Unchanged from `hero-on-top`. The hero's height is governed by the
poster image, reusing the home view's formula:

```
image_height = max(1, (hero_width * 9 + 31) / 32)
```

(16:9 aspect ratio in terminal cells, where cells are roughly twice as
tall as they are wide.) The hero's total height is
`image_height + meta_height + 1` (image + 1-row gap + meta block).

The hero's width is the full content width, so the hero grows taller as
the terminal gets wider — same table as `hero-on-top`:

| content_width | image_height | meta_height | total hero |
|---|---|---|---|
| 60 (1-col) | 17 | 5 | 23 |
| 82 (2-col kick-in) | 23 | 5 | 29 |
| 100 | 28 | 5 | 34 |
| 150 | 42 | 5 | 48 |

In a 60-col terminal this leaves ~1 row for the list — a problem in
narrow terminals, see decision 3.

### 3. Hero width scales with content width, but caps at a max

Unchanged from `hero-on-top`. Cap the image height so the list always
keeps a few rows regardless of terminal width:

a) **Cap the image height** at e.g. 12 rows regardless of width. The
   image is letterboxed; the meta block sits below it.

b) **Cap the hero width** at a value that leaves at least 6 rows for
   the list.

c) **Don't have a hero at all** in narrow terminals (< 70 cols).

Default: (a), cap the image height at 12 rows — same as `hero-on-top`.
Pick (a) or (c) based on visual feedback.

### 4. Selected cell indicator

Unchanged from `hero-on-top`. The selected cell loses the tab but
keeps its identity:

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

### 7. Series inline detail goes away

Unchanged from `hero-on-top`. The series inline detail (overview +
episode count) below the selected series row is replaced by the
bottom hero, which shows the series' overview, year, episode count,
etc. — everything the inline detail showed, just wider and pinned to
the panel floor instead of interleaved with the row it followed.

`series_detail_rows` is no longer reserved in `render_power_list`.

### 8. Compact banner layout extends to wider widths

Unchanged from `hero-on-top`. `compact_banner_layout_with_overview`
already takes `panel_width` and scales; the bottom hero just calls it
with the hero's (larger) width. No change to the function itself.

## What changes in the code

```
┌─── Touch points ─────────────────────────────────────────────────┐
│  render_power_list (src/app/render/list.rs)                     │
│    - split content_area into [list_area, hero_area]             │
│      (hero_area is the bottom slice, not the top)                │
│    - call the row renderer with list_area instead of            │
│      content_area                                                │
│    - paint the selected item's banner into hero_area, after      │
│      the list has rendered                                       │
│    - drop banner_rows, series_detail_rows, selected_block_bounds│
│                                                                    │
│  render_power_plain_rows (src/app/render/list_plain.rs)          │
│  render_power_letter_grouped_rows (list_letter_groups.rs)        │
│    - remove the inline selected-block painting                  │
│    - selected cell becomes bg + ▌ + ## prefix                    │
│                                                                    │
│  render_power_compact_detail (src/app/render/detail.rs)          │
│    - already takes panel_width; just gets called with a larger   │
│      value from render_power_list                                │
│                                                                    │
│  input handling                                                  │
│    - mouse click in hero_area → Enter equivalent                 │
│    - keyboard Enter unchanged (still opens selected item)        │
│                                                                    │
│  tests                                                           │
│    - list_tests.rs: drop the notched-block tests (the tab/panel  │
│      are gone); keep the per-cell tests; the invariant test      │
│      still applies (now comparing list_area at 1-col and 2-col) │
│    - add new tests for the hero area split, anchored at the      │
│      bottom of content_area                                      │
└───────────────────────────────────────────────────────────────────┘
```

## What stays the same

- `library_column_count`, `library_cell_rect`, `library_cell_slot`,
  `library_cell_width`, `LIBRARY_COLUMN_GAP`, `POWER_TWO_COLUMN_THRESHOLD`.
- hjkl nav, the sidebar h→x, the lib search input.
- The maintenance rule: list above the hero is the same renderer
  parameterized by `cols`. The invariant test still applies; it just
  compares `list_area` content at width 81 and 82.
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

1. Image height cap: (a) cap at 12 rows, (b) no cap, (c) drop hero at
   narrow widths. Pick based on what looks right.
2. Hero meta line: what fields go in it? Title + year + runtime + genres
   in one line? Just title + runtime? Try a few and see.
3. The `▌` mark — is it visible enough? Or do we need something more
   distinct? E.g. a thin colored bar at the top of the cell, or a
   different bg shade for the selected cell.
4. Does the hero need a visible top border (`▁`) to separate it from
   the list above, given there's no gap row otherwise? `hero-on-top`
   didn't need this (the tab bar above served as the separator); here
   the list's last row sits directly above the hero.
