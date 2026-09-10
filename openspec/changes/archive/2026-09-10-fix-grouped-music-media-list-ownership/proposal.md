## Why

Grouped Music still duplicates album-list position and hit geometry between the shell, render path, and embedded controls. This violates the existing canonical-media-list ownership contract and prevents campaign row 2 from closing.

## What Changes

- Make the persistent active WideMediaList or InlineMediaBrowser the sole live owner of grouped-album position.
- Keep ordinary content refresh distinct from explicit re-anchor and transfer one ViewportAnchor only at responsive handoff.
- Remove render-time album-control reseeding, paint-result position writeback, and parent album row-map/compatibility point resolution.
- Preserve Music-owned track workspace, search, group pills, images, effects, persistence, and typed intent translation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an ownership refactor that conforms to the existing canonical-media-list and interactive-component-framework requirements without changing observable requirements.

## Impact

- Affected area: Grouped Music interactive component, its shell projection, and its Wide/Inline render seam and tests.
- No API, dependency, persistence-format, or unrelated-destination change.
