## Why

Grouped music albums currently frame only the selected album and repeat the artist inside that frame, obscuring the artist-level structure that navigation already exposes. The selection treatment should make the artist discography the stable context while clearly identifying whether an artist or album is the current action target.

## What Changes

- Frame the selected artist's entire discography as one block in the music-group view, with the artist rendered once above a pinned action-hint row.
- Add a fixed marker gutter and cursor styling that distinguish the current artist or album action target without shifting album text.
- Show the focused artist's albums below the pinned hint row, using a 12-album inline window for larger discographies so the outer artist block stays stable.
- Keep the artist block anchored while navigating within it when possible, with cursor-follow fallback for content that exceeds the viewport.
- Append expanded track details inside the artist block while keeping sibling albums visible.
- Swap inline art between the artist collage and selected album cover, using one constant narrowed width for the block.
- Remove the duplicated album-artist display row from grouped and plain/search album rendering while retaining per-album framing outside the music-group view.

## Deferred

Following a critic review of the initial implementation plan, row-aware top-down artwork wrapping that reclaims full width below the 12-row art band remains deferred. Larger discographies use a derived 12-album inline window; the window is not persisted and has no nested scrollbar.

## Capabilities

### New Capabilities

- `grouped-album-selection`: Artist-scoped selection blocks, target markers, nested album scrolling, inline expansion, artwork layout, and non-grouped album compatibility.

### Modified Capabilities

None.

## Impact

- Affects grouped album display planning and rendering under `src/app/render/`, plus related library cursor, viewport, action-target, and mouse interaction code.
- Removes the `GroupedAlbumDisplayRow::AlbumArtist` structural row and simplifies related art-offset calculations in all album-list paths.
- Requires focused unit and render-plan tests for artist boundaries, wrapped album titles, nested scrolling, expanded tracks, artwork overlap, and plain/search behavior.
- Introduces no external API, configuration, persistence, or dependency changes.
