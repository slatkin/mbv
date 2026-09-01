# Restore feed-group inline expansion

## Why

Issues #634 and #637. After `BrowserComponent` took over the feed-view home-video group
picker (`is_feed_home_video_group_view` surfaces: feed-view home-video
libraries and Emby podcast channels), the selected video row stopped
expanding into its framed Inline hero: `render_feed_group_picker_content`
hardcodes `selected_h = 1`, which makes its own `h.saturating_sub(5)` banner
paint dead, and `render_wide_feed_layer` hardcodes a content-independent
`row_height = 5` with no banner paint at all. Real Emby home-video items carry
runtime/genre, so production hits the broken path on the first selected row;
the checked-in baselines missed it because their fixtures have no metadata.
This restores the Selected-row replacement the surface had at `fbc6888e` and
pins it with metadata-bearing fixtures.

## What Changes

- In Normal geometry, the feed-group picker's selected row expands to
  `compact_banner_layout_with_overview(item, panel_width, truncate_overview = true).content_rows() + 5`
  and paints the compact banner (meta line, truncated overview) inside the
  framed block, exactly like the generic home-video inline hero.
- Scroll for the picker uses the legacy accumulated-height clamp: scrolling
  keeps the full expanded block addressable instead of the current
  `offset = selected` approximation; the survivor of that clamp is written
  back to the component's scroll and projected into `video_scroll` through
  the existing Library-position path.
- Remove the unconditional trailing `▔` + last-item echo block in
  `render_feed_group_picker_content` — the framed expansion already owns the
  bottom border; the extra block is the "stray trailing border" from #634.
- Restore click hit-testing for picker rows: `render_feed_group_picker_content`
  populates `layout.left_row_map` / `left_item_rows` so
  `BrowserComponent::resolve_left_cursor` maps a click to the row under it
  (legacy `render_feed_home_video_group_view` built a `row_map` plus an
  offset+`click_y` fallback). Single- and double-click row selection currently
  do nothing.
- Drop the one-column right shift of the row at index `selected + 1`
  (`x + 1`, `width - 1`) in `render_feed_group_picker_content` — the indent
  was meant to sit inside the expanded frame's border, so it only makes sense
  once the frame is actually drawn; align it once the expansion is restored.
- Remove the dead `feed_items` work in `render_narrow_browse_with_ctx`: the
  function computes `inline_hero_rows` / `feed_ctx` / pills branches for the
  `feed_items` case and then returns early (delegating to
  `render_feed_group_picker_content`), leaving the later `feed_items.is_some()`
  arms unreachable and the pill-bar logic duplicated.
- Regenerate `FEED_NARROW_BASELINE` with
  metadata-bearing fixtures (runtime + genre + a wrapping overview) captured
  through `Model::draw_frame`; keep the metadata-free fixtures as the
  degenerate-case pin.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `library-list-hero`: adds a requirement pinning the feed-group picker's
  selected-row expansion and bottom-edge scrolling —
  the existing shared requirements cover hero-bearing browsers generically;
  this surface's inline-expansion height and single-paint behavior at both
  Panel modes become explicitly testable.

## Impact

- `src/app/render/components/list_narrow.rs`
  (`render_feed_group_picker_content`),
  possibly `src/app/components/browser_narrow.rs` (`NarrowBrowseExtras`) if
  the expansion height must be published shell-side.
- `src/app/tests_narrow_browse_migration.rs` (fixtures + baselines + scroll
  test).
- No ctrl protocol, daemon, persistence-format, or key-routing changes.
