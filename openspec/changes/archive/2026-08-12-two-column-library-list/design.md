## Context

`render_power_list` (`src/app/render/list.rs`) builds a `Vec<DisplayRow>` each frame and renders it through one of two bodies: `render_power_plain_rows` (`list_plain.rs`) or `render_power_letter_grouped_rows` (`list_letter_groups.rs`). `DisplayRow` (`list_rows.rs:66-72`) is:

```rust
pub(super) enum DisplayRow {
    Spacer,
    LetterHeader(String),
    Item(usize),
    BannerFiller,
    SeriesDetailFiller,
}
```

`Item(usize)` holds one item index, which is where the single-column assumption lives. Everything else in the pipeline is already row-based and column-agnostic:

- The cursor is a flat item index stored per navigation level (`BrowseLevel { cursor, scroll }`, `src/app/types_browse.rs:47-48`).
- The scroll offset is a display-row index, clamped at render time between `selected_detail_lower_bound(...)` and `display_cursor` (`list_plain.rs:73-93`) and written back after render (`list.rs:410-419`).
- `render_selected_block_background` (`power_widgets.rs:168-191`) already paints the selected item **and** its detail filler as one background rectangle spanning `top_pad_abs..bottom_pad_abs` at `area.width`, drawn before the rows so row spans paint over it.
- `render_power_right_scrollbar` (`power_widgets.rs:82-104`) is driven by offset/max/visible over display rows.
- Detail filler heights come from `compact_banner_rows` (`list.rs:31-39`) and `series_inline_detail_rows` (`list.rs:198-212`), and the fillers are injected after the selected item's row (`list_rows.rs:93-129`).

So the change is narrow in principle: teach the row builder to put more than one item on a row, and teach the selected block to be notched instead of rectangular. The risk is not conceptual, it is that the scroll clamp and offset-walkback logic are **duplicated** between the plain and letter-grouped renderers and must stay in step.

## Goals / Non-Goals

**Goals:**

- Use the width of a wide list pane without changing what the list shows.
- Preserve the inline detail banner and inline series detail exactly as they behave today.
- Keep the cursor a flat item index and the scroll offset a display-row index, so resize preserves selection for free.
- Make the selected item read as visually attached to its detail panel.
- Produce one rendering path that covers both one-column and two-column mode, rather than a `cols == 1` special case.

**Non-Goals:**

- Column-major flow, paging, or more than two columns.
- Replacing inline detail with a side detail pane.
- Touching the existing four-wide season grid (`is_viewing_season_grid`, `src/app/lib_cursor_actions.rs:164-177`).
- A user setting for column count.

## Decisions

### 1. Row-major, not column-major

Row-major maps item `i` to row `i / cols`, column `i % cols`. The mapping is independent of viewport height, so the existing display-row scroll offset keeps its meaning and the existing clamp works unchanged.

Column-major (`i` → row `i % height`, column `i / height`) was rejected. It makes position depend on viewport height, so scrolling either pages or teleports one item across the screen per scroll step, and resize reshuffles every item. Worse, a full-width detail band has no coherent insertion point: it would cut column 1 at a row unrelated to the cursor, which means column-major implicitly requires deleting the inline banner and building a side detail pane instead.

The accepted cost is that the alphabet serpentines — column 0 reads A, C, E rather than A, B, C. This is the known tradeoff and is accepted for this change.

### 2. `DisplayRow::Item(usize)` becomes a multi-item row

Replace `Item(usize)` with a row that carries the item indices occupying it, in column order. In one-column mode every such row carries exactly one index, so both modes share a single rendering path and there is no `cols == 1` branch in the renderers.

Cell geometry is derived from the content area and the column count: each cell is `(content_width - gap * (cols - 1)) / cols` wide, and cell `c` starts at `c * (cell_width + gap)`. Item rendering — marker, title, right-aligned metadata, truncation — operates on the cell rect instead of the full content rect, so existing per-row rendering is reused with a narrower `Rect`.

`Spacer`, `LetterHeader`, `BannerFiller`, and `SeriesDetailFiller` are unchanged and continue to span the full content width.

### 3. Column count from the list pane width

The list pane width is `area.width - queue_column_width` (`src/app/render/mod.rs:256-261`), and is `area.width` when `queue_column_collapsed`. Column count is computed from that value, not from `terminal_width`:

```text
cols = 2  if list_width >= 2 * LIBRARY_COLUMN_MIN_WIDTH + LIBRARY_COLUMN_GAP
       1  otherwise
```

`LIBRARY_COLUMN_MIN_WIDTH` is 40, reusing the anchor already established by `POWER_LEFT_WIDTH_DEFAULT` (`src/app/mod.rs:159`) as the narrowest width at which a media title row is readable. `LIBRARY_COLUMN_GAP` is 2.

Deriving from the list pane rather than the terminal means widening the queue column or collapsing it changes the column count live, which is the correct behaviour: the decision is about how much room the list actually has.

The helper lives beside `src/app/queue_column_width.rs` as a sibling geometry module rather than being inlined in the renderer, so both the renderer and the cursor-movement code can call it.

### 4. Cursor movement reads the column count, cursor stays flat

`move_lib_cursor` (`src/app/lib_cursor_actions.rs:7`) already takes a signed delta. Column awareness is confined to choosing the delta at the key-handling site (`src/app/input_lib_power_keys.rs`):

| Key | Delta |
| --- | --- |
| Left / Right | ∓1 / ±1 |
| Up / Down | ∓cols / ±cols |
| PageUp / PageDown | ∓(cols × page rows) / ±(cols × page rows) |
| Home / End | unchanged (`jump_lib_cursor`) |

In one-column mode `cols` is 1, so up/down keep their current behaviour and left/right become no-ops exactly as they are today — the existing left/right bindings for navigation levels are unaffected because this only applies where the list is flat.

Down from the second-to-last row where no item sits directly below clamps to the last item, which falls out of `move_lib_cursor`'s existing clamping.

Letter-grouped mode complicates the pure arithmetic, because independently packed buckets (decision 6) mean item index and row are not related by `i / cols`. Up/down in that mode must move via the laid-out row structure rather than by adding `cols` to the index. The row layout is computed at render time, so the last frame's row map is the available source; this is the same pattern already used by `page_power_grouped_album_cursor` and the music group-view jumps, which likewise consult render-derived structure.

### 5. The selected block becomes a notch

`render_selected_block_background` currently paints one rectangle at `area.x`/`area.width`. It is extended to paint two:

```text
one column                     two columns
┌───────────────────────┐      ┌──────────┐
│ pad row               │      │ pad row  │ partner cell   <- tab: cell slot
│ selected item         │      │ selected │ partner item   <- tab: cell slot
│ detail rows           │      ├──────────┴─────────────┐
│                       │      │ detail rows            │  <- panel: full width
└───────────────────────┘      └────────────────────────┘
```

- **Tab region**: the selected cell's slot rect, spanning the top padding row and the item row.
- **Panel region**: the full content width, spanning the detail filler rows.

Both use the same background (`palette::MEDIA_SELECTED_BG` focused, `palette::PLAYBACK_PANEL_BG` unfocused, per `list_plain.rs:111-126`), so they abut with no seam and read as a tab attached to a panel. The top padding row **must** narrow with the tab; leaving it full width would band across the unselected partner cell and destroy the effect.

When `cols == 1` the tab slot equals the full content width and the two rectangles collapse into today's single rectangle, so single-column appearance is unchanged and the visual language is the same at both widths.

The partner cell keeps the ordinary list background — that contrast is the only thing creating the notch.

### 6. Letter buckets pack independently

Each bucket starts a fresh item row, so a row never mixes items from two buckets. The cost is a ragged trailing cell at the end of every bucket, which is correct: the alternative — a row straddling the A/B boundary with the header rendered between them — is incoherent.

This means the item-index-to-row mapping is no longer `i / cols` in letter-grouped mode and must come from the row map built during layout, which the renderers already construct.

### 7. Cursor visibility clamps on the whole block

`selected_detail_lower_bound` (`list_plain.rs:73-75`) plus the filler walkback (`list_plain.rs:84-93`) already keep the selected item and its detail rows visible together. With pairing, `display_cursor` becomes the index of the **row containing** the cursor item rather than the item's own row index; with that substitution the existing logic keeps the tab and panel on screen together and no orphan panel can appear.

The letter-grouped renderer has its own header-aware copy of this scan. Both must receive the same `display_cursor` substitution.

### 8. Season grid left alone

`is_viewing_season_grid` already implements a four-wide stride with its own left/right handling (`input_lib_power_keys.rs:175-186`). Once general column machinery exists this is duplicated logic, and unifying them is tempting. It is deliberately out of scope: seasons are uniform, short, and have no inline detail, so they exercise none of the hard cases here. Unify later if the duplication still looks worth removing.

## Risks / Trade-offs

- **Serpentine reading order.** Accepted (decision 1). Mitigated by the change being gated on width, so narrowing the pane or collapsing the queue column reverts to one column, and by fuzzy search remaining the primary way to find a known title. If it proves wrong in daily use, reverting is deleting the packing step in the row builder; nothing else depends on it.
- **Duplicated scroll logic.** The plain and letter-grouped renderers each carry their own clamp and walkback. Any fix applied to one and not the other produces a scroll bug visible only in one mode. Both paths need explicit test coverage.
- **Background fill across the inter-column gap.** The tab's slot rect may make the gap column read as a smear next to the partner cell. Needs visual checking; if it reads badly, the tab rect can inset by the gap width.
- **Right-column tab.** A tab anchored on the right side of a full-width panel is less conventional than a left-anchored one. Expected to read acceptably, but is a named item for visual verification.
- **Truncation at the threshold.** Just above the two-column threshold each cell is 40 columns, which truncates many album and movie titles harder than one column did. The threshold may need raising after seeing it; it is a single constant.
- **Historically fragile area.** Power View layout changes have regressed before. Visual verification in a real terminal at several widths is a required part of this change, not an optional extra.

## Migration Plan

Not applicable — internal rendering change with no persisted state, no protocol surface, and no user data. The column count is derived per frame, so there is nothing to migrate on upgrade or downgrade.

## Open Questions

- Should `LIBRARY_COLUMN_MIN_WIDTH` stay at 40, or rise once the truncation at that width has been seen in a real terminal?
- Should the tab rect inset by the gap width, or fill its cell slot flush?

## Maintenance Rule: 1-col and 2-col stay the same view, parameterized

The library list is one renderer parameterized by `cols` (1 or 2), not two
renderers with branching logic. Every per-mode difference (cell width, slot
rect, left pad, page-row count, notched-block shape) is a parameter that
flows through the column-count abstraction (`library_column_count`,
`library_cell_rect`, `library_cell_slot`, the `cols` variable in
`render_power_plain_rows` / `render_power_letter_grouped_rows`). The two
modes share the same code path; only the inputs differ.

This means future tweaks apply to both modes by default — e.g. changing the
selected-cell pad, the banner indent, the cursor delta, or the item text
truncation in one place updates both. The exception is tweaks that are
genuinely 2-col-specific (e.g. the right cell absorbing the trailing
remainder column, or the smaller left pad in 2-col mode to avoid the
double-indent effect). Those go through the abstraction too: anything that
isn't keyed on `cols` or `library_column_count(width)` is a bug, not a
feature.

A regression test (`one_and_two_column_render_the_same_per_cell_content` in
`list_tests.rs`) locks this in by rendering the same library just below and
just above the column-count threshold and asserting that the selected
cell's content (truncation aside) is identical. If the test ever fails, the
two views have diverged and need to be reconciled before merging.

Visual polish that genuinely looks better in 2-col mode (e.g. the smaller
left pad) is fine as long as it remains a parameter and not a code branch
in the renderer. Anything that would require an `if cols > 1` in the
rendering pipeline should be extracted into a helper instead.

