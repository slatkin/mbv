## Context

See `proposal.md` for motivation. The scrolling selectors for Home sections, feed groups, music groups, and letter filters already share most sizing, overflow, and hitbox logic in `render_pill_bar`, but Home enters that logic through a separate renderer and style function. Series seasons draw pill-like spans directly, and the selectable queue scope uses status-pill rendering. Ratatui styles are applied during immediate-mode rendering, so consistency requires shared Rust presentation code rather than a CSS-like cascade.

## Goals / Non-Goals

**Goals:**

- Give every interactive pill selector one canonical visual shell and palette.
- Keep a single rendering path responsible for pill geometry, spacing, selection styling, and overflow indicators.
- Preserve the state and input contracts of each selector context.

**Non-Goals:**

- Rework primary tab navigation or make Ratatui `Tabs` the selector renderer.
- Restyle non-interactive status pills.
- Change selector labels, selection semantics, keyboard bindings, persistence, or data fetching.

## Decisions

### Use the current Home appearance as the canonical pill-selector shell

The shared renderer will own the joined angled edges, selected and unselected styles, row background, and overflow indicators currently unique to Home. Palette constants will be named for pill selectors rather than Home so callers do not encode a context-specific theme.

Alternative: preserve the current yellow, separated library pills. Rejected because the chosen product direction is to propagate Home's appearance.

### Keep one pill-bar renderer with context-neutral inputs

`render_pill_bar` will be the only renderer for interactive pill-selector choices. It will continue to accept labels, target IDs, and the selected position, and return visible `(Rect, target)` hitboxes. The Home-only entry point and style callback will be removed. Row painting and pill spacing will no longer be caller-selectable appearance variants.

Layout-only inputs, such as a leading inset or a separately rendered `Series:` prefix, may remain outside the bar. They do not alter the pill visual shell.

Alternative: share only palette constants while retaining separate renderers. Rejected because geometry and edge treatment could still drift.

### Adapt outliers at their selector boundary

Series-season choices will render their `Series:` prefix separately and delegate the remaining row to the common pill bar. This also lets the common overflow logic keep the selected season visible.

The selectable Local/Remote queue-scope pills are intentionally left on the status-pill path: they double as connection status (device name, connection icon, "Connected:" label) and are the only place that status is displayed. Rerouting them through the shared pill bar would strip that status with nowhere else to render it. They are treated as connection status, not a selector, and are out of scope for the shared pill bar.

Alternative: generalize all status-pill rendering into the pill bar. Rejected because status pills have richer content and different semantics, and doing so would expand the change beyond interactive selectors.

### Preserve domain state and input dispatch

The refactor will not introduce a new selector state model. Existing selected indices, caller-defined IDs, keyboard handlers, and click dispatch remain authoritative. Only render delegation and hitbox production change.

## Risks / Trade-offs

- [Joined pills consume different widths than separated pills] -> Reuse the common Unicode-width calculation and existing selected-item visibility logic, then update narrow-width render assertions.
- [Series seasons previously showed only the first page] -> Delegating to the shared pill bar now scrolls the visible window to keep the selected season on screen; update the render assertion if it assumed page-one-only display.
- [A shared row background may contrast differently across panels] -> Make the row surface part of the canonical pill-selector shell rather than inheriting context backgrounds.
- [Unicode edge glyphs can expose hitbox drift] -> Keep glyph width in the same renderer that records each pill rectangle and verify rendered cells against those rectangles.

## Migration Plan

1. Introduce context-neutral pill-selector palette tokens and make the common bar render the canonical shell.
2. Move existing scrolling selector callers to the sole renderer and remove Home-specific appearance code.
3. Delegate series seasons and selectable direct-remote queue scope to the common renderer.
4. Update existing render assertions and run the targeted render/input tests followed by the full test suite.

The change affects only TUI rendering code and requires no persisted-data or protocol migration. Rollback consists of reverting the rendering refactor and palette renames.
