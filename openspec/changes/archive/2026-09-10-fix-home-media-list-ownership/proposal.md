## Why

Home still duplicates list position and hit geometry between the shell, the render path, and its two persistent embedded controls. This violates the existing canonical-media-list ownership contract and prevents campaign row 3 from closing.

## What Changes

- Make the persistent active `WideMediaList` or `InlineMediaBrowser` the sole live owner of Home list position; only the active control moves.
- Migrate both controls onto the retained-result seam (parent sets geometry/paint policy, child `Component::view` paints, point-only resolution from retained current-frame geometry) and persist narrow scroll like Wide.
- Remove the render-to-component `resolved_section` writeback, the live `continue_cursor` App-wide mirror, the dead per-pill cursor cache, and parent compatibility point resolution.
- Keep ordinary content refresh distinct from explicit re-anchor and transfer one `ViewportAnchor` only at responsive handoff.
- Preserve Home-owned sections, pills, hero, images, effects, persistence, and typed intent translation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is an ownership refactor that conforms to the existing canonical-media-list and interactive-component-framework requirements without changing observable requirements.

## Impact

- Affected area: Home interactive component, its shell projection, and its Wide/Inline render seam and tests.
- No API, dependency, persistence-format, or unrelated-destination change.
