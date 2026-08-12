# Design: hero inline, just below the selected item

## Decisions

### 1. List splits into top section, hero, bottom section

The list is conceptually three pieces, rendered in order:

```
   top_section      hero       bottom_section
   ┌──────────┐  ┌────────┐  ┌──────────┐
   │ items    │  │        │  │ items    │
   │ above    │  │ selected│  │ below    │
   │ the      │  │ item's  │  │ the      │
   │ selected │  │ banner  │  │ selected │
   │ item     │  │        │  │ item     │
   └──────────┘  └────────┘  └──────────┘
```

The "top section" packs items in 1-or-2 cols as usual, ending with
the row that contains the selected item. The hero is painted into
the rows immediately below. The "bottom section" packs the rest of
the items in 1-or-2 cols, starting at the row below the hero.

### 2. Hero position follows the cursor

The hero is positioned **directly below the display row containing
the selected item**. As the cursor moves:
- Cursor moves up → hero moves up (toward the top of the list).
- Cursor moves down → hero moves down (toward the bottom of the list).
- Cursor at the top → hero is just below the first row.
- Cursor at the bottom → hero is just below the last row.

The hero position is `selected_display_row + 1` in absolute display
row coordinates.

### 3. Hero size stays the same as the top-hero design

Reuse the top-hero's height formula:
```
HERO_IMAGE_CAP_ROWS = 12
HERO_GAP_ROWS = 1
HERO_META_ROWS = 5
hero_height_for_width(width) =
    min((width * 9 + 31) / 32, 12) + 1 + 5  // 12 to 18 rows
```

The hero gets the full content width (same as the top-hero design).

### 4. Top section and bottom section are independent packings

Each section packs items in 1 or 2 cols. The column count is the same
for both sections (driven by `list_area.width`).

Edge case: if the selected item is in the last row of the top
section's packing, the top section still includes the selected row,
and the bottom section is empty. If the selected item is in the first
row, the top section is empty (the selected row IS the top section's
first row).

Wait — the top section always includes the selected row. The bottom
section starts with the row immediately below the selected row. So
if the selected item is item 0, the top section has just that one
row, the hero is below it, and the bottom section has items 1+.

### 5. Row map reflects the inserted hero

The `left_row_map` is a `Vec<Option<usize>>` indexed by display row,
where each entry is the item index that lives at that row, or `None`
for hero rows. Mouse click handling uses the row map to find which
item a click is on; a click on a hero row (None) hits the hero, not
an item.

```
   display row 0:  Some(item 0)     // top section
   display row 1:  Some(item 1)     // top section
   display row 2:  Some(item 2)     // top section (selected)
   display row 3:  None             // hero start
   display row 4:  None
   ...
   display row 20: None             // hero end
   display row 21: Some(item 3)     // bottom section
   display row 22: Some(item 4)     // bottom section
   ...
```

### 6. Auto-scroll accounts for the hero

The list auto-scrolls so the cursor stays visible. With the hero
inline, "visible" means: the cursor's row + the hero rows after it
must all fit in the content area. If they don't, the list scrolls
to bring them into view.

Specifically, the scroll calculation:
1. Compute the cursor's display row, given the current scroll offset
   and column count.
2. Compute the hero's display row (cursor's row + 1).
3. Compute the hero's bottom row (hero top + hero_height).
4. If the cursor's row is above the visible area, scroll up.
5. If the hero's bottom row is below the visible area, scroll down.

This is a small modification to the existing auto-scroll logic in
`render_power_plain_rows` (and `render_power_letter_grouped_rows`).

### 7. Selected cell marker stays the same

The selected cell uses the same `▌` + `##` marker as the top-hero
design. The hero provides visual identification through proximity,
not through a tab.

### 8. Double-click on the hero = Enter equivalent

**Amended.** A single click inside `hero_area` only focuses the
library panel, matching the app-wide "single click only focuses;
double-click plays" convention (see the fix for #448's mouse-click
review). A double-click inside `hero_area` opens the selected item --
the same activation `left_area`'s double-click and Enter perform.

### 9. Series keeps its own inline detail (season pills + episode table)

**Amended.** The original plan replaced the series inline detail with
the generic hero (image + meta + overview only), dropping the season
pills and episode table. That regressed real functionality (browsing
and playing an episode without leaving the list), so `detail_series.rs`
/ `detail_series_view.rs` were restored: a selected Series renders its
season pills + episode table (`render_series_inline_detail`) in the
same inline slot a selected Movie's hero would occupy, sized by
`series_inline_detail_rows` instead of the movie hero's
`hero_height_for_width`. Both share the same block framing (border +
padding rows, `HERO_BLOCK_EXTRA_ROWS`) -- only the content differs.

## What changes in the code

```
┌─── Touch points ─────────────────────────────────────────────────┐
│  render_power_list (src/app/render/list.rs)                     │
│    - drop the hero-at-top logic from hero-on-top                │
│    - compute the selected item's display row position           │
│    - split the row renderer into top_section + bottom_section  │
│    - paint the hero between the two sections                    │
│                                                                    │
│  render_power_plain_rows (src/app/render/list_plain.rs)          │
│  render_power_letter_grouped_rows (list_letter_groups.rs)        │
│    - add a parameter or context for the "start at item N"       │
│      and "render rows until item M" so each section can be       │
│      packed independently                                        │
│    - update the row map to insert None entries for hero rows    │
│    - update the auto-scroll to account for hero rows            │
│                                                                    │
│  layout.rs                                                       │
│    - LayoutMain.hero_area: still the hero rect, but now         │
│      positioned below the selected row, not at the top          │
│                                                                    │
│  input_mouse.rs                                                  │
│    - click_set_cursor: hero click is still Enter equivalent;    │
│      the click falls through to the hero_area check              │
│      (no change needed if the rect is correct)                  │
│                                                                    │
│  tests                                                           │
│    - update the hero-on-top tests to assert the inline          │
│      position (hero below the selected row, list wraps around)  │
│    - add new tests for the row map and auto-scroll with hero    │
│    - drop the hero-on-top specific tests                        │
└───────────────────────────────────────────────────────────────────┘
```

## What stays the same

- `library_column_count`, `library_cell_rect`, `library_cell_slot`,
  `library_cell_width`, `LIBRARY_COLUMN_GAP`, `POWER_TWO_COLUMN_THRESHOLD`.
- hjkl nav, the sidebar h→x, the lib search input.
- The maintenance rule: the list (top + bottom sections) is the same
  renderer parameterized by `cols`. The invariant test still applies.
- The 2-col padding in `power_right_panel_content_area`.
- The hero's content, image cap, meta block — same as top-hero.

## What is NOT in this change

- Home view refactor.
- Music group view changes.
- Feed home video group view changes.
- Smooth scrolling of the hero with the cursor (jump only for now).
- Changes to the top-hero branch.

## Open questions for the implementer

1. **Section packing in 2-col mode**: when the top section has 3
   items and they're packed 2-per-row, the selected item is in the
   second row. The hero goes below that row. The bottom section
   starts after the hero. Does this feel right, or should the
   section break align to row boundaries (e.g. always end the top
   section at the last full row before the selected item)?
2. **Auto-scroll behavior**: when the cursor + hero don't fit, do
   we prefer the cursor at the top (and the hero below) or the
   cursor at the bottom (and the hero above)? For the first pass,
   just make sure the cursor and hero are both visible.
3. **What about the search input?** The search input area is above
   the list. It still works the same way (3 rows at the top, list
   below). The hero is still below the selected item within the
   list, not below the search input.
