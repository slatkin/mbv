## Why

Context menus opened via the keyboard shortcut appear at inconsistent, often
unrelated screen positions because each view independently reports the
selected row's y-coordinate into a shared layout field (`cursor_screen_y`),
and any view that omits this falls back to a hardcoded corner. There is no
dim backdrop to set the menu apart from the content behind it, unlike every
other modal overlay in the app. And once a context menu is open, the
keyboard cannot navigate or dismiss it at all — no `CONTEXT_STACK` entry
handles Up/Down/Enter/Esc for it, so the only way to act on an open menu is
the mouse.

## What Changes

- Every view that supports opening a context menu exposes a `Rect` for its
  currently selected item (row or grid cell), replacing the scattered,
  y-only `cursor_screen_y` / `queue_cursor_screen_y` layout fields with a
  single geometric fact per panel.
- The keyboard-triggered context menu is positioned deterministically from
  that rect: right edge aligned to the selected item's right edge; opens
  downward from the item's top edge if the menu fits below, otherwise
  upward from the item's bottom edge. Mouse-triggered (`open_context_menu_at`)
  positioning is unchanged.
- A dim backdrop renders behind the open context menu, reusing the existing
  `dim_backdrop` mechanism already used by every other modal overlay.
- The open context menu becomes keyboard-navigable: Up/Down move the
  selection, Enter executes the highlighted entry, Esc closes the menu
  without acting — via a new `context_menu` entry in `CONTEXT_STACK`,
  replacing the ad hoc `context_menu_open()` guards scattered across other
  key handlers.

## Capabilities

### New Capabilities
- `context-menu`: keyboard-triggered context menu positioning (anchored to
  the selected item, flips to fit on screen), backdrop dimming while open,
  and keyboard interaction (navigate/execute/dismiss) for the open menu.

### Modified Capabilities
(none — no existing spec currently describes context menu behavior)

## Impact

- `src/app/types_context_menu.rs`: `ContextMenu` gains an anchor
  representation tied to a selected-item rect instead of bare `x`/`y`.
- `src/app/input_context_menu.rs`: `context_menu_spawn_point` replaced with
  rect-based anchor + flip logic; `open_context_menu_at` (mouse path)
  untouched.
- `src/app/render/overlays/context_menu.rs`: calls `dim_backdrop`.
- `src/app/render/mod.rs`: `any_dim_modal_open` includes
  `context_menu.is_some()`.
- `src/app/input_resolver.rs`: new `context_menu` `CONTEXT_STACK` entry with
  its own handler for Up/Down/Enter/Esc.
- Render call sites that currently set `cursor_screen_y` /
  `queue_cursor_screen_y` (list, list_plain, list_letter_groups, home,
  home_feed, home_video, album, album_detail, detail, music_wide,
  music_wide_browser, audiobookshelf, audiobookshelf_book_browser, queue):
  each updated to report a selected-item `Rect` instead of a bare `y`.
- Existing `context_menu_open()` guards in `input_lib_keys.rs`,
  `input_queue_keys.rs`, `input_confirm_keys.rs` are superseded by the new
  stack entry's precedence and can be reviewed for removal.
