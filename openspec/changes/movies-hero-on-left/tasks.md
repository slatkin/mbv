## 1. Shared Hero Card

- [x] 1.1 Identify the Home wide selected-Emby hero-card data preparation, layout measurement, image fetch/cache, and paint entry points, including the active Movie fields and the `id:pwr_kw` cache key.
- [x] 1.2 Extract the selected-Emby hero-card path so Home and the wide Movies arrangement call the same implementation without moving Home section/cursor state into the shared card path.
- [x] 1.3 Preserve Home's existing wide rendering output while confirming the shared card retains 16:9 artwork, centered image behavior, watch-state glyph, release date, duration, overview, and graceful empty-field handling.

## 2. Movies Hero-On-Left Composition

- [x] 2.1 Add the Movies-only wide arrangement dispatch using the existing Hero-onLeft width/height thresholds and pane geometry helpers; leave TV, podcasts, feeds, home videos, Music, and Audiobookshelf dispatch unchanged.
- [x] 2.2 Feed the selected Movie from the active right-rail list source into the shared Home hero card, including the active inline-search result cursor when search is open.
- [x] 2.3 Compose the wide Movies panes as a read-only shared hero card on the left and a right rail containing the existing pill slot and list-panel chrome.
- [x] 2.4 Keep the wide Movies hero out of interactive hero geometry and ensure no hero-pane focus state or activation path is introduced.
- [x] 2.5 Preserve the existing Hero-onTop Movies fallback below the shared breakpoint, including its current portrait-poster card and list behavior.

## 3. Right-Rail Movies List

- [x] 3.1 Render eligible Movies letter-range pills through the existing shared pill bar at the top of the Hero-onLeft right rail, with active search replacing that slot through the existing search control.
- [x] 3.2 Render the Movies list below the right-rail pill slot as one column while retaining the existing plain/letter-grouped row data, cursor, scrolling, and marker behavior.
- [x] 3.3 Keep `LayoutMain.left_area` and existing library cursor/page-size bookkeeping aligned with the right-rail list so `j`/`k`, `Up`/`Down`, paging, and list activation continue to target Movies rows.
- [x] 3.4 Verify `Enter` and existing list activation remain the only keyboard activation path for wide Movies; do not add left/right pane-focus behavior for the read-only hero.

## 4. Verification

- [x] 4.1 Add targeted render/model coverage proving Home and wide Movies use the same selected-Emby hero-card path and image cache key for the same Movie.
- [x] 4.2 Add coverage proving the wide Movies cursor updates the left card even when the selected row is scrolled out of the visible right rail.
- [x] 4.3 Add coverage proving wide Movies has a right-rail pill row and one-column list, while narrow Movies remains Hero-onTop and existing activation behavior is unchanged.
- [x] 4.4 Verify queue focus dims both wide Movies surfaces without making the hero focusable, and verify the hero does not participate in keyboard activation.
- [x] 4.5 Run targeted tests, Rust formatting checks, the repository file-size check, and temporary wide/narrow capture comparisons; remove temporary captures after review.
