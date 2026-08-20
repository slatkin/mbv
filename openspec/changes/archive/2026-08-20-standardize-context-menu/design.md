## Context

`AppLayout` currently carries `cursor_screen_y: Option<u16>` for the library
panel and `queue_cursor_screen_y: Option<u16>` for the queue. The current tree
has fourteen writers, including nested detail renderers and renderers for
Audiobookshelf views where `open_context_menu` explicitly refuses to open a
menu. Some parent renderers already save and restore the y-coordinate around a
nested renderer, demonstrating that write ownership is ambiguous.

The original plan also drifted behind the rendering model. `list_plain.rs` and
`list_letter_groups.rs`, not only `album.rs`, now pack one or two columns through
`left_item_rows`. `home.rs` no longer writes cursor geometry directly;
`home_list_rows.rs` does. Existing tests have two direct y-coordinate assertions,
not one positioning characterization test per view.

Menu dimensions are currently computed only in `render_context_menu`. Therefore
a placement function taking only an item rect and panel rect cannot implement
right alignment or test whether the full menu fits vertically. Positioning must
share the rendered menu size.

Separately, `context_menu.is_some()` is checked ad hoc by selected
`CONTEXT_STACK` handlers. There is no context-menu stack entry, despite stale
comments in input tests referring to one. The render pass draws the context menu
before other overlays, so allowing two modal surfaces to coexist would also
allow double dimming or dimming the menu itself.

## Goals / Non-Goals

**Goals:**
- One authoritative selected row/cell rect per supported panel.
- Size-aware, deterministic placement recalculated from each fresh frame.
- One shared geometry rule for one- and two-column Emby lists.
- Existing modal backdrop and half-block image behavior while a menu is open.
- Exclusive menu input ownership and an explicit one-modal-at-a-time invariant.

**Non-Goals:**
- Changing menu entries or actions.
- Adding context menus to Audiobookshelf or Feeds.
- Submenus, type-to-select, hover-to-highlight, or configurable bindings.
- Preserving the old inline-image avoidance special case; deterministic item
  anchoring replaces it.

## Decisions

### Retain anchor intent and resolve it during rendering

`ContextMenu` stores an anchor kind rather than a resolved `(x, y)`:

- `SelectedItem(PanelFocus)` for keyboard opening.
- `Pointer { x, y }` for mouse opening.

`render_context_menu` runs after `render_main`, so it can resolve a keyboard
anchor from the fresh local `AppLayout` being built for that same frame. This
avoids stale placement after terminal resize or layout changes while the menu is
open. Pointer anchoring remains independent of selected-item geometry.

Menu construction remains independent of anchor availability so the mouse path
cannot fail merely because no selected-item rect exists. A missing keyboard
rect falls back to the containing panel's origin and is covered by a regression
test; supported renderers are expected to publish a rect.

### Share menu size and bounded placement

One menu-size calculation is used by both placement and rendering. A pure
function takes the anchor rect or pointer, containing panel rect, and rendered
menu size. For a selected item it:

1. right-aligns the menu to the selected rect;
2. opens from the selected rect's top when the full menu fits below;
3. otherwise aligns the menu bottom to the selected rect's bottom;
4. clamps the result inside the containing panel.

Exact anchor alignment wins when compatible with visibility; panel bounds win
otherwise. If a menu is itself larger than a panel dimension, the origin is
pinned to that panel edge and unavoidable overflow may be clipped. Saturating
arithmetic is used for all `u16` geometry.

### The outer selectable renderer owns the rect

The rect means “the row or cell the user selected,” not nested detail or hero
content. Only the outer renderer that maps the cursor to that selectable unit
writes it. `render_compact_detail`, `render_selected_home_video_detail`, and
other nested renderers stop writing cursor geometry.

Column-aware Emby lists derive the selected cell rect from the same
`left_item_rows`, area, offset, column-gap, and cell-width calculation already
used for drawing and mouse hit-testing. A helper beside
`draw_column_selection_markers` keeps marker, hit target, and context anchor in
sync. Existing concrete row rects/hitmaps are reused where available rather
than recomputed.

Audiobookshelf and Feeds remain unsupported by `open_context_menu`; their old
y-coordinate writes are deleted without replacement.

### The menu is the highest-priority keyboard context

A `context_menu` entry is first in `CONTEXT_STACK`. When active it handles
Up/Down/Enter/Esc and returns claimed for every other key. Up/Down wrap among
actionable entries and skip separators. Enter clones the selected action,
closes the menu and clears its layout hit target, then executes the action,
matching the existing mouse ordering.

Because all other keys are swallowed, F1-F4, Ctrl+/, tab switching, playback,
refresh, and view actions cannot replace or mutate content beneath the menu.
The user must press Esc or choose an action first.

### One modal surface at a time

Opening a context menu is refused while another modal or sidebar surface is
active. Any mandatory modal that may become active asynchronously closes the
context menu first. Context-menu actions close the menu before they may open a
follow-on modal. A render-time debug assertion protects the no-coexistence
invariant.

The context menu is added to `any_dim_modal_open` before main content renders,
selecting the existing half-block image path. `render_context_menu` calls
`dim_backdrop` after main content and before drawing the menu. Since modal
surfaces cannot coexist, the backdrop is applied exactly once and never dims
the menu itself.

Mouse click behavior is preserved. While a menu is open, non-menu mouse events
are swallowed so scrolling cannot move the target beneath a modal menu.

## Risks / Trade-offs

- A supported renderer can still omit its authoritative rect. The visible
  panel-origin fallback prevents an off-screen menu, while focused renderer
  tests make omissions detectable.
- Pointer coordinates are absolute; resizing after a mouse-opened menu may move
  the panel relative to that point. Per-frame clamping keeps the menu visible
  without inventing a relative pointer model.
- Extremely small panels can be smaller than the menu. Clipping is preferable
  to geometry underflow or escaping into another panel; current menus are short
  enough that this is only a degraded-terminal case.
- Changing `CONTEXT_STACK` order is intentional architecture behavior and must
  update ADR 0002 and its pinned order test together.

## Migration Plan

Single PR with each commit kept buildable:

1. Add the anchor kind, shared size calculation, pure placement helper, and
   focused geometry tests without switching the current renderer.
2. Add selected rect fields and migrate supported authoritative renderers while
   the old y fields remain available.
3. Switch context-menu rendering to fresh-frame anchor resolution, then remove
   the old y fields and obsolete/nested/unsupported writes.
4. Add dim/half-block participation and enforce the one-modal invariant.
5. Add the highest-priority keyboard context and mouse swallowing, remove
   redundant guards, and update ADR 0002 plus precedence tests.

Rollback is a revert; no persisted state or protocol changes are involved.
