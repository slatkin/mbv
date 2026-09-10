# Fix Book Media-List Ownership

## Why

The Audiobookshelf Book destination still constructs its Wide book list per frame and duplicates book position and hit geometry in parent/content state, violating the canonical media-list ownership contract and blocking umbrella rows 8.1–8.3. Its inline presentation also omits the selected book’s chapter detail and its cover-loading and placeholder paths diverge from the accepted Emby/TV and Podcast patterns.

## What Changes

- Make persistent Wide and Inline book controls the sole live owners of book cursor, scroll, retained hit geometry, and responsive `ViewportAnchor` handoff; keep the content-struct cursor only as effect-path state keyed by component-resolved values.
- Delete the Book parent’s bespoke book-row geometry, browser offset, render-time selection/scroll reseeding, and paint-result writeback; resolve click and wheel interaction through the active control’s retained current-frame geometry.
- Carry the resolved optional chapter index in `ActivateChapter`, removing the shell’s component downcast while preserving Book-owned chapter workspace mechanics and absolute chapter-seek authority.
- Implement the existing narrow book-browsing requirement by painting the selected book’s hero and chapter detail in the admitted inline replacement, using the shared content-budget pattern.
- Converge cover-loading and empty/loading presentation on accepted shared behavior by adopting the shared series image dimensions, placeholder flag pattern, and current-frame claim invalidation at both breakpoints.
- Add focused component/media-list, render-buffer, and Wide plus Normal/Narrow shell-tick coverage for active-control movement, retained hits and claims, rail-painter/no-underpaint ownership, presentation convergence, and target-plus-offset breakpoint handoff.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. The narrow chapter detail implements the existing `audiobookshelf-book-browsing` requirement, and the ownership repair conforms to the existing `canonical-media-lists` contract.

## Impact

- Affected area: `AudiobookshelfBookComponent`, its Book render component and shell request boundary, app test registration and tick harness access, and focused component/render/shell-tick tests.
- No API, dependency, persistence-format, Service lifecycle, surname-bucket data/grouping, chapter workspace mechanics, absolute chapter-seek, playback/enqueue, download/progress, image fetch/paint, router, mounted identity, or shared-helper change.
