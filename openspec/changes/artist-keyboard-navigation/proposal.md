## Why

The music library's grouped album view displays albums organized by artist, but keyboard navigation only supports line-by-line (Up/Down) or page-sized jumps (PageUp/PageDown). Navigating between artists in a large library requires many keystrokes. Adding Ctrl+PageUp/PageDown to jump directly to the next/previous artist header would make large music libraries far more navigable.

## What Changes

- Add Ctrl+PageUp and Ctrl+PageDown keybindings in the library panel when viewing grouped album rows.
- Ctrl+PageDown jumps the cursor to the next artist header in the display target list.
- Ctrl+PageUp jumps the cursor to the previous artist header in the display target list.
- When no further artist header exists in the pressed direction, the cursor moves to the last/first item respectively.
- Scroll offset is updated to keep the new cursor position visible.

## Capabilities

### New Capabilities
- `artist-keyboard-navigation`: Ctrl+PageUp/PageDown keybindings that jump the library cursor between artist groups in the grouped album view.

### Modified Capabilities

(none)

## Impact

- Affected code: input handling in `input_lib_power_keys.rs` (key dispatch), cursor navigation in `album_cursor.rs` (jump logic).
- No changes to rendering, state model, or persistence.
- No new dependencies.
