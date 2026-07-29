## Context

`render_power_home_list` (in `home.rs`) renders the Continue Watching home tab in the right panel's library content area. It currently stacks content vertically: pill row (1 row), then a hero panel below that, then the item list below the hero. The hero includes an image thumbnail on the right side and metadata text on the left side within the hero row.

When the terminal is wide (right column >= 80 columns), this vertical arrangement wastes horizontal space—the hero image sits in a single row, and the item list below it doesn't benefit from the extra width.

## Goals / Non-Goals

**Goals:**
- At `area.width >= 80`, render hero on the left and item list on the right in a two-column split.
- At `area.width < 80`, preserve the existing vertical (hero-above, list-below) layout.
- Pill selectors remain full-width at the top in both layouts.
- Hero image and metadata rendering is identical to today; only the hero area position changes.
- Item list rendering (rows, scrollbar, hitmap) is identical; only the list area shrinks.
- No changes to cursor navigation, scroll state, or hitmap logic.

**Non-Goals:**
- Changing the hero panel sizing algorithm.
- Changing the hero image/media rendering.
- Adding new state fields to `HomePane`.
- Supporting arbitrary column resizing between the hero and list—the split is a fixed proportion.

## Decisions

**Split hero to 40% width in the two-column layout.**

The hero image is already computed as `(area.width * 2/5)` with min/max clamps in the existing code. In the two-column layout, the hero column gets 40% of the available width and the list column gets the remaining 60%. This keeps the hero sizing proportional and familiar.

*Alternative considered:* using a fixed-width hero column — rejected because it wouldn't scale with terminal size, wasting space or clipping at extremes.

**Hero column height is the full content height (minus pill row).**

In the vertical layout, the hero is capped by `content_area.height - 7` and other terminal-height-aware heuristics. In the horizontal layout, the hero column takes the full available height below the pills, and the metadata layout's `height` field already constrains the rendered rows. The image fills the remaining vertical space below the metadata.

*Alternative considered:* keeping the vertical hero height cap in the horizontal layout — rejected because it wastes the vertical space freed by removing the item list from below the hero.

**Item list gets the right column, full height.**

The list area is positioned to the right of the hero column and takes the full remaining height. The existing scrollbar and hitmap logic works unchanged, just with a narrower area.

**Gate on `area.width >= 80`, not terminal width.**

The right panel's library content area is already computed in `render_main` with padding applied via `power_right_panel_content_area`. Gating on this area's width (not the terminal width) correctly accounts for the left panel and gutters.

## Risks / Trade-offs

- [Narrow hero metadata] → At the 40% split with smaller terminals just above the 80-column threshold, the hero metadata column will be ~32 characters wide. The existing metadata wrapping (via `textwrap`) handles this gracefully.
- [Two-column layout hides the hero sooner when width drops below 80] → This is the intended behavior: smaller terminals get the vertical layout. The threshold is explicit and straightforward.
- [Existing test fixtures may need updating] → Tests that assert on pixel-precise render output will need their `area.width` adjusted. Any test with width exactly at the boundary needs both layouts covered.
