## Context

See proposal.md - Why. Today `AppLayout` carries `cursor_screen_y: Option<u16>`
(library panel) and `queue_cursor_screen_y: Option<u16>` (queue panel), each
independently written by ~13 render call sites, one per view. Positioning code
(`context_menu_spawn_point`) reads whichever one applies and falls back to a
hardcoded corner if it was never set for the current frame. Multi-column views
(e.g. `album.rs`'s two-column track packing) already compute a row→column
mapping for drawing the selection marker (`left_item_rows`,
`draw_column_selection_markers` in `list_rows.rs`) but don't expose it as a
rectangle anywhere.

Separately, `context_menu.is_some()` is checked ad hoc inside several
`CONTEXT_STACK` handlers (`input_lib_keys.rs`, `input_queue_keys.rs`,
`input_confirm_keys.rs`) purely to stop those handlers from acting while a
menu is open — there is no handler that lets the keyboard act *on* the menu.

## Goals / Non-Goals

**Goals:**
- One geometric fact per panel ("here is the selected item's rect") that
  every view is responsible for keeping current, replacing the y-only field.
- Deterministic anchor + flip positioning built from that rect.
- The context menu becomes a normal `CONTEXT_STACK` context with its own key
  handler, so its keyboard behavior is explicit and testable the same way
  every other modal context already is (confirm_modal, daemon_lost_modal, etc.).

**Non-Goals:**
- Changing what appears in the context menu (entries/actions) — unaffected.
- Changing mouse click-to-open positioning (`open_context_menu_at`) — unaffected.
- Submenus, mouse hover-to-highlight, or any interaction beyond
  navigate/execute/dismiss.
- Reworking `left_item_rows` / `draw_column_selection_markers` themselves —
  only exposing the rect they already imply.

## Decisions

**Replace `cursor_screen_y: Option<u16>` / `queue_cursor_screen_y: Option<u16>`
with `selected_item_rect: Option<Rect>` / `queue_selected_item_rect: Option<Rect>`.**
Keeping two fields (one per panel) mirrors the existing split — both panels
can render simultaneously regardless of which has focus, so collapsing them
into one field would require tagging which panel it belongs to for no benefit.
Widening `u16` to `Rect` is the minimal change that makes right-alignment and
the fits-below/fits-above test possible, since both need width and height, not
just a y-coordinate. Every render call site that currently sets the y-only
field is updated to set the rect instead, in the same place — this doesn't
remove the "one call site per view" convention (still a manual step per view),
but it does mean a view that forgets it gets an absent rect the anchor logic
can detect and clamp rather than a stale/wrong `y` from a previous frame.

**Grid/column views (album.rs) compute the rect from the same `left_item_rows`
mapping already used by `draw_column_selection_markers`, at the render call
site that already draws the marker.** Alternative considered: give every
column-aware view its own from-scratch geometry pass for the anchor. Rejected
— `left_item_rows` already encodes exactly "which row and column is the
selected item in," so deriving the rect there is a few lines against existing
data, not a new mapping to keep in sync with the one `draw_column_selection_markers`
uses.

**Anchor and flip logic lives in one function taking a `Rect` (selected item)
and the containing area, returning the menu's `(x, y)`.** Keeps the
right-align/flip rule in exactly one place regardless of how many panels feed
it, so it can't drift between panels the way position computation has
drifted between views historically.

**Context menu keyboard handling becomes a new `CONTEXT_STACK` entry
(`context_menu`), positioned above the entries whose ad hoc
`context_menu_open()` guards exist solely to avoid double-handling a key
while the menu is open.** Once the new entry exists and sits above them in
the stack, it claims Up/Down/Enter/Esc and any other key while the menu is
open (matching how `confirm_modal` and other modal contexts already claim
all input while active), making those scattered guards redundant — they're
removed as part of this change rather than left as dead defense-in-depth,
since the whole point is to have one authoritative place instead of many.
`'.'` opening the menu again while already open becomes a no-op via the new
entry's ownership of all input while active, rather than reopening.

**Non-selectable entries (separators) are skipped by Up/Down, matching
`ContextMenu::first_selectable`'s existing skip-to-first-actionable
behavior used when the menu opens.**

## Risks / Trade-offs

- [Every render call site must still remember to set the new rect field,
  same as today] → An absent rect degrades to a safe, visible fallback
  (e.g. top-left of the panel) rather than the current top-left-of-terminal
  `(4, 4)`, and existing test coverage patterns (one characterization test
  per view, as already used for `cursor_screen_y`) extend naturally to the
  rect.
- [Removing the scattered `context_menu_open()` guards changes behavior for
  any key not already covered by a characterization test] → The new
  `context_menu` stack entry is written to swallow (not fall through) any
  key it doesn't bind, which is a strictly more defensive default than the
  guards it replaces; existing regression tests for those guards (e.g. `c`
  not leaking through) are re-pointed at the new entry rather than deleted.

## Migration Plan

Single-PR, behavior-preserving until the last step (no intermediate broken
state to roll forward/back through):
1. Add `Rect`-based fields alongside the existing `u16` fields; positioning
   logic still reads the old fields.
2. Switch positioning logic to the new rect-based anchor/flip function; keep
   old fields until every view sets the new one.
3. Update each view's render call site to set the new rect field; remove the
   old `u16` fields once all call sites are migrated.
4. Add the `context_menu` `CONTEXT_STACK` entry; wire dim backdrop; remove
   the now-redundant `context_menu_open()` guards one at a time, re-pointing
   their regression tests at the new entry.

Rollback is a revert of the PR; no persisted state or protocol changes are
involved.
