## Why

Tracking issue: [#583](https://github.com/slatkin/mbv/issues/583)

Context menus opened via the keyboard shortcut appear at inconsistent, often
unrelated screen positions because renderers independently publish only the
selected row's y-coordinate into shared layout fields. Multi-column lists now
make that insufficient: the menu also needs the selected cell's horizontal
position and width. Missing geometry falls back to a hardcoded terminal corner.

The menu also lacks the dimmed backdrop and input ownership used by the app's
other modal surfaces. Images remain in their normal rendering mode, and the
keyboard cannot navigate or dismiss the menu. Ad hoc guards stop only selected
underlying handlers, so input behavior depends on stack order instead of the
menu owning interaction while it is open.

## What Changes

- Current context-menu surfaces (Home, Emby browse views, and the queue) publish
  the screen `Rect` of their selected row or grid cell. The outer selectable
  renderer is the sole owner of this geometry; nested hero/detail renderers do
  not overwrite it.
- A keyboard-opened menu retains a selected-item anchor and is positioned each
  frame from fresh layout geometry and the rendered menu size. Its right edge
  aligns with the selected item; it opens downward when it fits, otherwise
  upward, then clamps inside the containing panel when exact alignment is not
  possible.
- A mouse-opened menu retains its click-point anchor. Its existing click-based
  placement remains independent of selected-item geometry and is clamped only
  to keep the menu visible.
- The context menu uses the existing dim-backdrop path, including the existing
  half-block image rendering used while modal content is active.
- The open menu becomes the exclusive keyboard context. Up/Down navigate
  selectable entries, Enter executes, and Esc dismisses; every other key is
  swallowed, including sidebar, tab, and global-overlay shortcuts.
- Only one modal surface is active at a time. A context menu cannot open over an
  existing overlay; a mandatory asynchronously activated modal closes and
  replaces an open context menu.

Audiobookshelf and Feeds continue not to expose context menus. Their obsolete
y-coordinate writes are removed rather than replaced with unused anchor state.

## Capabilities

### New Capabilities
- `context-menu`: deterministic selected-item and pointer positioning, bounded
  panel placement, modal backdrop/image treatment, exclusive keyboard
  interaction, and one-modal-at-a-time behavior.

### Modified Capabilities
(none — no existing spec currently describes context menu behavior)

## Impact

- `src/app/types_context_menu.rs`: `ContextMenu` gains an anchor kind and shared
  rendered-size calculation instead of storing only resolved `x`/`y`.
- `src/app/layout.rs`: y-only cursor fields are replaced by selected-item rects
  for the library and queue panels.
- `src/app/input_context_menu.rs`: menu construction records anchor intent;
  positioning moves to one size-aware bounded-placement function.
- `src/app/render/overlays/context_menu.rs`: resolves the anchor from the fresh
  frame layout, calls `dim_backdrop`, and renders at the bounded position.
- `src/app/render/mod.rs`: context-menu state participates in
  `any_dim_modal_open` and the one-modal invariant.
- `src/app/render/list_rows.rs` and current Emby/Home/queue renderers: shared
  one/two-column cell geometry publishes one authoritative selected rect.
  Nested detail writers and unsupported Audiobookshelf writers are removed.
- `src/app/input_resolver.rs`: a highest-priority `context_menu` stack entry
  owns all keyboard input while active; redundant per-handler guards are
  removed.
- Mouse dispatch preserves menu click behavior and prevents non-menu mouse
  events from reaching the obscured view while the menu is open.
- `docs/adr/0002-centralized-input-handling.md`: records the new explicit
  context-menu precedence and replacement invariant.
