## Why

The search modal looks identical in fuzzy and global mode, so users have no immediate visual cue for which mode they are in. The first implementation attempt changed the palette constant references but produced no visible difference, suggesting the mode was not threaded through to the rendering calls that set the background. This change picks a palette colour that already exists and uses it to make the two modes visually distinct.

## What Changes

- The search modal body background SHALL differ by mode: global mode uses `LIBRARY_SIDE_BG` (#2d353b, the current value), fuzzy mode uses `BG_GREEN` (#3c4841).
- All rendering sites that currently hard-code `LIBRARY_SIDE_BG` for the modal body, result rows, state messages, and type-filter gaps SHALL pick the colour based on the modal's current `SearchMode`.
- The modal-frame background (passed to `render_modal_frame`) SHALL follow the same mode-dependent choice.

## Capabilities

### New Capabilities

_(none)_

### Modified Capabilities

- `search-modal`: The "Modal styling matches the application palette" requirement is amended so the modal body background is no longer a single fixed colour but depends on the active search mode.

## Impact

- `src/app/render/overlays/search_modal.rs`: every site that references `palette::LIBRARY_SIDE_BG` as the modal body background must become mode-dependent.
- `src/app/palette.rs`: no changes; `BG_GREEN` and `LIBRARY_SIDE_BG` already exist.
- No API, daemon, or ctrl-protocol changes.
