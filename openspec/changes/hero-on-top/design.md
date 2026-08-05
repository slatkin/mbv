# Design: hero on top of the library list

## Decisions

### 1. Hero area, then list area

`render_power_list` splits its `content_area` vertically into two
stacked rects: `hero_area` on top, `list_area` below. The hero is
painted into `hero_area`; the row renderer is given `list_area`.

```
   content_area
   ┌──────────────────────┐
   │      hero_area       │  height = h_hero(content_area.width)
   ├──────────────────────┤
   │                      │
   │      list_area       │  height = content_area.height - h_hero
   │                      │
   └──────────────────────┘
```

The list renderer doesn't know about the hero. It just gets a rect and
renders rows into it. The 2-col / 1-col decision is unchanged —
`library_column_count(list_area.width)` drives the column count, with
the same `POWER_TWO_COLUMN_THRESHOLD` (82).

### 2. Hero height from the image's natural aspect ratio

The hero's height is governed by the poster image, not by a separate
design decision. Reuse the home view's formula:

```
image_height = max(1, (hero_width * 9 + 31) / 32)
```

(16:9 aspect ratio in terminal cells, where cells are roughly twice as
tall as they are wide.) The hero's total height is
`image_height + meta_height + 1` (image + 1-row gap + meta block).

The hero's width is the full content width. So the hero gets taller as
the terminal gets wider:

| content_width | image_height | meta_height | total hero |
|---|---|---|---|
| 60 (1-col) | 17 | 5 | 23 |
| 82 (2-col kick-in) | 23 | 5 | 29 |
| 100 | 28 | 5 | 34 |
| 150 | 42 | 5 | 48 |

This makes the hero grow proportionally. In a 60-col terminal the hero
is 23 rows tall, leaving ~1 row for the list. That's a problem in
narrow terminals — see decision 3.

### 3. Hero width scales with content width, but caps at a max

The current 1-col / 2-col threshold is 82 cols, but the hero at 82 cols
is already 29 rows tall — too much. Cap the hero width at a reasonable
maximum so the list always has at least a few rows:

```
hero_width = min(content_width, 60)   // e.g. 60
list_width = content_width - hero_width - 1  // gap
```

With a 60-col hero at 16:9, `image_height = 17`, plus meta = 5, plus
gap = 23 rows. In a 24-row terminal that leaves 1 row for the list.
Still too tight. Two options:

a) **Cap the image height** at e.g. 12 rows regardless of width. The
   image is letterboxed; the meta block sits below.

b) **Cap the hero width** at a value that leaves at least 6 rows for
   the list: `hero_width = min(content_width, content_width - 6 * 32/9 - 5 - 1)`.

c) **Don't have a hero at all** in narrow terminals. Below a threshold
   (e.g. < 70 cols), no hero, just the list.

The implementer should pick (a) or (c) based on visual feedback. The
default in this change is (a) — cap the image height at 12 rows.

### 4. Selected cell indicator

The selected cell loses the tab but keeps its identity. Two layers:

- `##` (2 cols) prefix in the title for selected cells, like the
  current 2-col inline design.
- A `▌` mark on the left edge of the selected cell.
- The bg color is the same as the rest of the list (no special
  selected bg). The `##` and `▌` are the visual identifier.

The `MEDIA_SELECTED_BG` color is no longer used for the list below the
hero. It's reserved for the hero itself.

### 5. Click on the hero opens the selected item

Clicking inside `hero_area` is an Enter equivalent — it opens the
selected item. This gives the hero a purpose beyond decoration and
matches the user's intuition that "the big thing at the top is
important and interactive."

The mouse hit test is the same as the existing Enter handling: if the
hero is clicked, fire the same action as Enter on the selected item.

### 6. Series inline detail goes away

The series inline detail (overview + episode count) below the selected
series row is replaced by the top hero. The hero shows the series'
overview, year, episode count, etc. — everything the inline detail
showed, just wider and at the top.

This means `series_detail_rows` is no longer reserved in
`render_power_list`. The series detail logic in
`render_power_list` is removed (or becomes a no-op for backward
compatibility with tests that check it).

### 7. Compact banner layout extends to wider widths

The current `compact_banner_layout_with_overview` is sized for the
40-col inline banner. It needs to work for the wider top hero too.

The layout function already takes `panel_width` as a parameter. The
top hero just calls it with a larger value (the hero's width). The
overview gets more wrapped lines, the meta block stays compact.

No change to `compact_banner_layout_with_overview` itself — it
already scales.

## What changes in the code

```
┌─── Touch points ─────────────────────────────────────────────────┐
│  render_power_list (src/app/render/list.rs)                     │
│    - split content_area into [hero_area, list_area]             │
│    - paint the selected item's banner into hero_area            │
│    - call the row renderer with list_area instead of            │
│      content_area                                               │
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
│    - add new tests for the hero area split                       │
└───────────────────────────────────────────────────────────────────┘
```

## What stays the same

- `library_column_count`, `library_cell_rect`, `library_cell_slot`,
  `library_cell_width`, `LIBRARY_COLUMN_GAP`, `POWER_TWO_COLUMN_THRESHOLD`.
- hjkl nav, the sidebar h→x, the lib search input.
- The maintenance rule: list below the hero is the same renderer
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
