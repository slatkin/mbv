## Why

The Continue Watching tab currently uses a vertical stacked layout: hero panel on top, item list below. On narrow terminals this is fine, but when the right column has 80+ columns of available space, a side-by-side layout would make better use of the horizontal real estate—keeping the hero image always visible while scrolling the item list independently.

## What Changes

- When the right column width (`area.width`) is >= 80, split the content area (below the pill row) into two columns: hero image on the left, item list on the right.
- When the right column width is < 80, the existing vertical layout (hero on top, list below) is preserved.
- The pill selectors remain at the top spanning the full width in both layouts.
- Hero panel sizing and rendering logic is reused as-is; only its position changes.
- Item list rendering and scroll behavior is reused as-is; only its area shrinks horizontally.

## Capabilities

### New Capabilities

_None_

### Modified Capabilities

- **Home / Continue Watching tab**: Two-column responsive layout at wider terminal widths.

## Impact

- **Render code**: `src/app/render/home.rs` — `render_power_home_list` is the sole function modified. The hero-area and list-area computations are gated on `area.width >= 80` to produce a horizontal split instead of the current vertical stack.
- **Hero rendering**: `src/app/render/home_hero.rs` — No changes. `render_keep_watching_hero_image` and `render_keep_watching_hero_meta` are called with a repositioned hero area.
- **No new state**: The layout switch is purely render-time; `HomePane` is unchanged.
- **Tests**: `src/app/render/home_tests.rs` — Existing tests may need updated fixture widths to exercise both layouts. New tests should cover the >= 80 threshold.
