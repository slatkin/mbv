## Why

Artist headers in the grouped music album view are selectable action targets, but this introduces a third focus state beside album browsing and track selection and makes the selected album detail ambiguous. The headers read more clearly as structural labels, while albums remain the only selectable rows.

## What Changes

- Make artist headers non-selectable visual grouping labels in the grouped music album view.
- Remove Ctrl+PageUp/PageDown navigation to artist headers.
- Remove artist-header Play, Shuffle, Enqueue, current-item, and context-menu action scope.
- Make mouse interaction with artist headers inert while preserving album-row interaction.
- Remove artist-header focus state and the rendering, cursor, and test paths that support it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `artist-keyboard-navigation`: Remove Ctrl+PageUp/PageDown navigation to selectable artist headers.
- `stable-music-library-grouping`: Preserve stable visual artist grouping while making artist headers non-selectable and removing header-scoped actions.

## Impact

- **Code**: Grouped music cursor planning and movement, library key and mouse handling, music action scope, artist-header rendering state, and focused tests under `src/app/`.
- **Behavior**: Artist headers remain visible but cannot receive focus or invoke bulk actions; album and track interactions are otherwise unchanged.
- **Data/API**: None.
- **Dependencies**: None.
