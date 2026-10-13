# Proposal

## Why

Issue #928. Each scrollable list surface gets the viewport height for its wheel clamp from a different place:

- `MediaList` (`media_list.rs` `scroll_wheel`) clamps against a fixed one-row viewport. The viewport can end with only the last row at its top. The next paint snaps it back.
- `ThreeLineFlatList::scroll_viewport(delta, painted_height)` takes the height as an argument. `SessionsComponent` keeps its own `painted_content_area` field only to supply it.
- `TreeBrowser` reads `configured_geometry`, then its own `last_painted` area, then `1`.

The result is one rule with four implementations. A wheel at the end of one list overshoots and snaps back. The same wheel on another list clamps exactly. Each surface also stores a copy of a fact that the shared `PaintRetainedState` already holds (the painted content rectangle).

## What Changes

- `PaintRetainedState` keeps the height of the last completed content rectangle. That value survives `begin` and `invalidate`. Only the next completed paint replaces it. The existing `content_rect()` keeps its meaning (current completed frame only).
- `MediaList` wheel clamp uses the viewport height of its `WideMediaList` presentation, not a constant `1`. The canonical owner with no presentation keeps the one-row guarantee.
- `ThreeLineFlatList::scroll_viewport(delta)` drops its height argument and reads the retained height.
- `SessionsComponent` drops `painted_content_area`. Its test accessor reads the list's retained rectangle.
- `TreeBrowser` drops `last_painted`. Its fallback is the shared retained height.
- No change to the wheel step, to selection rules, or to anchor rules.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mouse-input`: the boundary scenario of "Wheel scrolling moves the viewport by a uniform step" states that a wheel notch toward the end leaves the last flow row at the bottom edge of the viewport, never above it.

## Impact

- `crates/mbv-components/src/list/paint.rs`, `list/three_line.rs`, `list/tree_browser.rs`, `list/tree_browser/operations.rs`, `list/tree_browser/render.rs`
- `crates/mbv-components/src/media_list.rs`, `media_list/wide.rs`, `media_list/carrier.rs`
- `crates/mbv-components/src/sessions.rs`
- No public API outside `mbv-components`. No new dependency.
- Resolves the three questions in #928 (see `design.md`).
