## Why

The visualizer currently renders only at the bottom of the right panel (below the library listing). Adding a second visualizer strip at the bottom of the left panel (queue/card column) gives the user a symmetric visual experience and makes use of otherwise empty space at the bottom of the narrower column.

## What Changes

- Render a visualizer strip at the bottom of the left panel in , using the same `VISUALIZER_HEIGHT` (11 rows) as the existing right-panel strip.
- The left-panel visualizer is an addition; the existing right-panel visualizer is **not** removed.
- Both strips share the same `visualizer_frame` data and toggle state (`visualizer_enabled`).
- The left-panel strip naturally adapts to the narrower column width via the existing bar-scaling logic in `render_visualizer`.

## Capabilities

### New Capabilities

_None_

### Modified Capabilities

_None_

## Impact

- **Render code**: `src/app/render/mod.rs` — the left-panel layout in `render_main` must split off a bottom strip and call `render_visualizer` for it.
- **Visualizer module**: `src/app/render/visualizer.rs` — `split_visualizer_area` and `render_visualizer` are reused as-is; no changes expected unless the narrower width requires a minimum-width guard.
- **Layout**: `src/app/layout.rs` — no new fields needed; the existing `panel_area` / `panel_content_area` already describe the left column geometry.
- **Tests**: Existing visualizer tests remain valid. New render tests may be added for the left-panel split.
