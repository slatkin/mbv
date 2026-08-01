## Context

`render_power_home_list` currently models the list as a flat sequence of one-row items. It uses `trunc_str` for labels, computes content height from the number of logical items, keeps the cursor visible using a one-row span, and records one one-row mouse hitbox per item. The Home video/feed renderers already provide precedent for variable-height rendering, while the hero and album renderers provide precedent for `textwrap`-based layout.

The change must improve readability without changing what the Home tab means or which fields it shows. A Music item must retain its existing default representation; this change must not decide whether that representation is a track, album, or another item type, nor add artist/album metadata.

## Goals / Non-Goals

**Goals:**

- Show the complete existing Home label instead of truncating it with an ellipsis.
- Preserve the current one-row appearance whenever the content fits.
- Use a coherent fallback for episodes whose existing inline series/title representation does not fit.
- Keep durations, markers, colors, selection backgrounds, scrolling, mouse interaction, and scrollbar geometry correct.
- Reflow wrapping from the actual list width in both wide two-column and stacked layouts.

**Non-Goals:**

- Adding or removing Home metadata, including artist, album, year, overview, or production information.
- Changing Home section ordering, filtering, playback behavior, or cursor semantics.
- Adding a user setting or toggle between wrapped and truncated modes.
- Expanding the left hero panel; it already provides a separate detailed presentation.

## Decisions

### 1. Treat wrapping as a physical-row layout over logical items

Keep the existing logical item/cursor model, but derive a display layout for each item containing its wrapped lines and physical height. Total content height is the sum of those heights, and the existing physical-row scroll offset remains the coordinate used for viewport and scrollbar calculations.

The cursor remains an item index. Its physical top and bottom are found from the cumulative heights of preceding items, and the cursor-visibility clamp receives the complete item span rather than `cursor_row..cursor_row + 1`.

If a selected item is taller than the viewport, the implementation cannot show its entire span at once. In that case, entering the item SHALL keep its marker/first physical row visible and SHALL allow the remaining physical rows to extend below the viewport; scrolling within that oversized item may reveal its continuation rows without changing the logical cursor.

### 2. Preserve the current representation for non-episodes

For movies, music, albums, and every other non-episode item, wrap the same rendered label currently used by the Home list. Reserve the existing duration area on the first line only; continuation lines use the full label width and are indented beneath the content column.

```text
▶ A long existing Home label that needs wrapping       42m
  continues here without changing its meaning
```

An item that fits remains exactly one row. No new Music-specific fields are introduced.

### 3. Preserve inline episodes when possible, stack only when necessary

First measure the current inline episode representation, including the duration reservation. If it fits, render it as the current one-row yellow-series/white-title line.

If it does not fit, render a stacked representation using the same series and episode fields and styles:

```text
▶ Series Name
  Complete episode title that can wrap              42m
```

The series name may use continuation lines when necessary. The episode title begins as its own indented block, with duration on its first line and any further title lines using the full content width. No text is truncated in either form.

### 4. Keep row decorations attached to the logical item

The marker is rendered only on the item's first physical line; continuation lines begin at the existing content indentation. A focused selection background and any item border extend across the item's full physical height. The duration remains right-aligned on the first line of the relevant label block and is not repeated on continuation lines.

Mouse hitmaps use one rectangle per logical item whose height covers all of its physical rows (clipped to the visible list area as needed), so clicking any continuation line selects the same item.

### 5. Use existing wrapping and variable-height patterns

Reuse the repository's `textwrap` conventions and the variable-height layout approach already used by Home video/feed and album-detail renderers. Keep the implementation localized to Home rendering unless compilation or shared layout types require a small supporting change. Avoid precomputing data-model fields or changing API responses.

### 6. Resolve scrollbar width before final wrapping

Wrapping width depends on whether the list needs a scrollbar. Resolve this deterministically in two passes: first measure using the list width without the scrollbar gutter, determine whether the resulting physical content height overflows the viewport, then reserve the gutter and remeasure if needed. Because reserving width can only preserve or increase physical height, the overflow decision will not oscillate. Use the final measurement for rendering, scrolling, hitmaps, and scrollbar geometry.

### 7. Handle narrow widths by vertical growth, not truncation

Wrapping is based on the actual available width after the marker, indentation, scrollbar gutter, and first-line duration reservation. Narrow terminals may therefore produce taller items. The physical content height and scrollbar must account for that growth; the implementation should not silently reintroduce truncation at a width threshold.

If the terminal is too narrow for the marker, duration, indentation, and one label cell to coexist on a single line, preserve the complete label and duration by placing the duration on its own indented physical line. This is a degraded layout fallback for extreme widths; it must not truncate either value.

## Risks / Trade-offs

- **Reduced density**: Long or narrow layouts show fewer logical items at once. This is intentional in exchange for seeing complete labels.
- **Resize reflow**: A terminal resize can change many item heights and the current scroll position's visual context. Recalculate layout from the new width and clamp the physical offset and cursor span together.
- **Very long labels**: An unusually long user-controlled label can consume many rows. It remains readable and scrollable rather than being truncated.
- **Episode complexity**: Inline measurement and stacked fallback require more layout code than simply wrapping one string. Keeping the inline path unchanged minimizes visual churn for common cases.

## Open Questions

None for the initial implementation. The existing Home representation is the source of truth for Music and other non-episode items.
