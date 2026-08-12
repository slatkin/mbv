## Why

Pill selectors currently use multiple render paths, so changing one selector's appearance does not consistently update the others. A single visual contract will keep these controls recognizable and prevent the Home selector from drifting from library, season, and queue-scope selectors.

## What Changes

- Make the current Home pill-selector appearance the canonical appearance for all interactive pill selectors.
- Apply the shared appearance to Home sections, feed groups, music groups, letter filters, series seasons, and the selectable Local/Remote queue scope.
- Centralize pill-selector colors and rendering so future appearance changes propagate to every pill selector.
- Preserve existing selection state, scrolling, caller IDs, mouse hitboxes, and keyboard behavior.
- Keep primary Home/library tabs and non-interactive status pills outside this visual contract.

## Capabilities

### New Capabilities

- `pill-selector-presentation`: Defines the shared appearance and behavior-preservation requirements for interactive pill selectors throughout the TUI.

### Modified Capabilities

None.

## Impact

- Affects pill-selector rendering and palette definitions under `src/app/render/` and `src/app/palette.rs`.
- Updates existing render assertions for Home, library, series-season, and Queue scope selectors.
- Does not change external APIs, persisted state, protocol capabilities, dependencies, primary tabs, or status-only pills.
