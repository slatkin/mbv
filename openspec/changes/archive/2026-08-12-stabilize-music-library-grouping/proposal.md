## Why

Opening a configured music album view can repeatedly reshuffle artist groups while album artist metadata arrives asynchronously. On slower machines or servers, the resulting redraw work and movement make the library feel unstable precisely when a user first enters it.

## What Changes

- Add a settled loading transition for grouped music album views: resolve the artist data needed for an opened album snapshot before presenting its artist-sorted rows.
- Commit each loaded album snapshot to the grouped display as one stable update instead of progressively regrouping it as individual artist lookups complete.
- Preserve the selected album and a coherent viewport when a grouped display is committed or replaced.
- Move grouping work and artist-data request scheduling out of the render path, and reuse a cached grouping result while its underlying snapshot is unchanged.
- Keep existing configured music navigation, artist-header actions, and album/track selection behavior intact.

## Capabilities

### New Capabilities
- `stable-music-library-grouping`: Present configured music album views as a stable, artist-grouped snapshot without progressive on-screen reshuffling.

### Modified Capabilities
- None.

## Impact

- Affects the music album grouping renderer and display-plan construction in `src/app/render/`.
- Affects music browse event handling, artist metadata resolution, and per-library UI state in `src/app/`.
- May enrich or restructure existing Emby library fetches, but introduces no user-facing configuration, API, or dependency changes.
