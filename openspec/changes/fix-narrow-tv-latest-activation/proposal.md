# Proposal

## Why

At narrow (non-Wide) geometry, activating a TV `Latest` row plays a different episode than the one selected. The flat Latest list is painted newest-first, but the narrow key path resolves the selection by indexing the cursor into the same items re-sorted alphabetically, so Enter, Ctrl+P, Ctrl+A and the other selected-item actions address the wrong episode. Wide geometry is unaffected because it resolves the row by its stable target.

## What Changes

- Selected-item resolution in the TV content owner resolves by the selected row's stable target whenever the flat `Latest`/`Upcoming` list is showing, so every narrow-geometry keyboard action (activate, play, enqueue, shuffle, watched toggle, context) addresses the displayed row.
- No change to Wide geometry, mouse activation, the show-tree path, or row painting.

## Capabilities

### New Capabilities

### Modified Capabilities
- `tv-library-content-modes`: "Latest and Upcoming rows play directly or open their series" gains a scenario pinning that keyboard actions in every geometry address the row displayed under the selection.

## Impact

- `src/app/components/tv_content/episode_rows.rs` (`selected_item`), exercised by `keyboard.rs` narrow handlers and `panel_owner.rs`.
- New component test in `src/app/components/tv_content/tests/`.
- Out of scope: Movies not playing after a remote-daemon disconnect (empty server URL/api key in the mpv stream URL) is a separate, not-yet-root-caused bug.
