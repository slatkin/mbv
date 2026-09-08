## Why

The queue column can be resized only in five-column keyboard steps. Mouse users need the standard precise interaction of dragging the existing boundary between the Queue and Library panels without adding new visual chrome.

## What Changes

- Make the existing right edge of the Queue panel draggable when both panels are visible.
- Resize the queue column live at one-column precision while preserving the existing minimum and maximum widths.
- Persist the final width when the drag ends rather than writing preferences for every pointer movement.
- Keep click-only interaction, Queue row dragging, single-panel modes, and the existing visual boundary unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mouse-input`: Define ownership, recognition, exact-width resolution, and verification for queue-column boundary dragging.
- `panel-mode`: Extend the existing column-resize availability contract to mouse dragging.

## Impact

- Interactive ownership and mouse handling for the Queue panel boundary.
- Queue-column width mutation and preference persistence.
- Root chrome geometry used to place the Queue and Library panels.
- Focused component and live-tick integration tests for drag behavior and arbitration.
- No new dependency, divider, gutter, or visual variant.
