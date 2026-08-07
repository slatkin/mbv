## Context

See proposal.md for motivation. The playback panel currently renders only inside the right column (`right_visible && player_h > 0` guard in `render_main`). Queue-only sets `right_visible = false`, so the panel disappears entirely.

The card image (`render_card_image` in `card.rs`) centers itself horizontally within its area and returns `(height, image_loading)` but not width. The actual rendered width is computed inside `size_for` but discarded after positioning.

## Goals / Non-Goals

**Goals:**
- Render the playback panel in queue-only mode using the existing `render_player_panel` call
- Support narrow (stacked) and wide (side-by-side) arrangements based on a terminal width threshold
- Left-align the hero image in wide mode so the playback panel can use the remaining space

**Non-Goals:**
- Redesigning the playback panel content (future follow-up after visual verification)
- Changing behavior in `Both` or `LibraryOnly` modes
- Persisting the narrow/wide threshold as a user setting

## Decisions

### 1. Image alignment via a parameter on `render_card_image`

Add a `left_align: bool` parameter to `render_card_image`. When true, `img_x = area.x` instead of centering. All existing call sites pass `false` to preserve current behavior; the queue-only wide path passes `true`.

Alternative: a separate method or enum. A bool is simpler and there are only two alignment cases.

### 2. Return image width from `render_card_image` and `render_power_card`

Change `render_card_image` return type from `(u16, bool)` to `(u16, u16, bool)` — `(height, width, image_loading)`. `render_power_card` propagates this. In the wide layout, the caller uses the returned width to place the playback panel at `image_x + image_width + 2`.

The width is the `actual.width` from `size_for`, or `area.width` for placeholders/loading states (full-width placeholder keeps the layout stable while the image loads).

Track `last_card_width` alongside `last_card_height` for the placeholder path, following the same pattern: store on successful render, reuse when image is loading or not yet available. Reset both in the same places.

### 3. Wide threshold is terminal width >= 100 columns

Checked against `area.width` in `render_main` when `panel_mode == QueueOnly`. This is the overall content area width, which matches terminal width minus any frame chrome.

### 4. Playback panel placement in `render_main`

The queue-only branch in `render_main` (around line 404–429) currently calls `render_power_card` then computes the queue area from `left_remaining`. The new logic inserts after the card render:

- **Narrow**: render playback panel as a full-width block between the card and queue. `player_h` rows consumed from `left_remaining`.
- **Wide**: render playback panel beside the card. Uses `card_h` as height, `area.width - card_w - 2` as width, positioned at `card_area.x + card_w + 2`. No height consumed from `left_remaining` beyond what the card already took.

The playback panel is rendered with `DARK_BG` in both cases, not the conditional `BG_GREEN`/`PLAYBACK_PANEL_BG` used by the right-column version.

### 5. Playback panel always rendered in queue-only (not gated on `right_visible`)

The existing `right_visible && player_h > 0` guard stays for the right-column rendering. The queue-only playback panel is a separate render call in the queue-only layout branch, independent of `right_visible`.

### 6. Wide-mode leftover space reuses the existing left-column visualizer

In wide mode the panel area is `card_h` rows tall but playback content only uses `player_h` rows, leaving `card_h - player_h` rows of `DARK_BG` below it. When `self.visualizer_enabled` and that leftover is `>= 3` rows (the same minimum `render_visualizer` already requires elsewhere, since it reserves a 1-row margin top and bottom), call the existing `render_visualizer(f, area)` into that leftover rect instead of leaving it flat `DARK_BG`. Below 3 rows, or when the visualizer is disabled, the space stays `DARK_BG` (already painted by the panel background fill, no extra code needed).

This is a second, independent call site for `render_visualizer` — distinct from the existing left-column one at the bottom of the queue area (`visualizer_h`/`left_viz_area`), which keeps rendering as-is in both narrow and wide queue-only layouts. No new state or threshold is introduced; `visualizer_enabled` is the existing toggle.

## Risks / Trade-offs

- **Image width instability during loading**: Before the image loads, `last_card_width` may be 0 (first render) or stale (item changed). Using `area.width` as the placeholder width means the playback panel gets zero width until the image loads. This matches the existing behavior where the card area reserves full height while loading — the playback panel simply appears once the image settles. Acceptable for an initial implementation.
- **Narrow terminals in wide mode**: A terminal at exactly 100 columns with a large backdrop image could leave very little width for the playback panel. The seekbar and controls degrade gracefully at small widths since they already handle narrow right columns, so this is safe.
