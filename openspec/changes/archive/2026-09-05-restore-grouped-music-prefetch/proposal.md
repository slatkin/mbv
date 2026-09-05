## Why

Grouped-Music album art warms slowly while scrolling in the `migrate-tui-to-tuirealm` worktree. On `main`, narrow/wide grouped music pre-warmed ±3 albums around the cursor every frame from inside the shared render path; in the worktree that loop survives as `App::prewarm_grouped_music_album_images` but its only call site is gated on `!wide`, so wide has zero neighbour prefetch and narrow warms from a freshly rebuilt context rather than the cursor actually being painted (see slatkin/mbv#647).

## What Changes

- Call the existing `prewarm_grouped_music_album_images` helper for the wide presentation as well as narrow, using the same ±3-ahead/±1-behind display-order window, idle gating, cache keys (`{album_id}:P`) and art types (`MUSIC_ALBUM_IMAGE_TYPES`).
- Source the prefetch window from the cursor and display order actually being painted (the component's authoritative cursor and the order it paints with), instead of a render-time rebuilt context that can disagree with the painted one.
- Skip prefetch while the search-results grid is active (the grid is not the canonical album rail).
- No change to cache keys, fetch params, `ratatui-image` version, idle-gate semantics, or the ±3/±1 window size.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `music-library-hero`: grouped Music neighbour album-art prefetch SHALL fire in both wide and narrow presentations, keyed off the painted cursor and display order.

## Impact

- `src/app/shell_music_workspace.rs` (prefetch call site), `src/app/components/music_workspace.rs` (possible order/cursor accessor), `src/app/render/components/music_wide.rs` (helper unchanged in shape, possibly call-shape).
- Test-only additions in `src/app/shell_music_workspace_image_tests.rs` (wide prefetch test, stale-order regression test).
- No new dependencies, no protocol/config changes, no keyboard/mouse routing changes.
