---
status: accepted
---

# The GUI Key Target Follows the Pointer

The GUI Client has no Panel focus. The thing under the pointer receives
each GUI key: a Library row, the Library column, or the Queue list box.
No press, no Tab key, and no pointer crossing stores a focus. The TUI
Client keeps the Client core's focus model unchanged
([ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md)).

## Decision

- The window resolves the pointer target from Blitz's hover node. After
  each window event — and so after each frame and each scroll — the
  launch copy reads `BaseDocument::get_hover_node_id`, walks up the
  parent chain, and builds a `PointerTarget` from DOM markers:
  `data-pointer-zone` names the Library column or the Queue list box,
  `data-pointer-list` names the list (a destination list, the Hero
  tracklist, or a Workspace table), and `data-pointer-row` carries the
  hovered row's id. The walk, `pointer_target(chain)`, is a pure
  function over attribute lookups, and the nearest marker of each kind
  wins. A pointer leave empties the target. The launch copy writes the
  value into a window store only when it changed, so a hover change
  re-renders nothing. The CSS `:hover` veil stays the hover paint, and
  it reads the same Blitz hover node, so the look and the key target
  always agree.
- Each key carries its resolved target to the Client core.
  `KeyOutcome::Core(KeyTarget)` names `Queue` or `None`. The window's
  key router (`route_library_key`) resolves the target from the pointer
  target at every point where it sends a key to the core. The Queue
  filter's text entry always names `Queue`, so the keys that the filter
  does not consume still reach the Queue leaf.
- The core sets `FocusStack`'s Panel focus per key, as an internal
  adapter. `CoreThread::route_key(key, target)` first sets Panel focus
  `Queue` for a `Queue` target and `Library` for `None`, and skips the
  call when the value is already in effect. It then routes with the
  Queue leaf only when the stack's focus is `Queue`. It keeps
  `ComponentId::Library` mounted, so a `None` target leaves only the
  leafless global and transport chords. Each `GuiLibraryCommand` the
  core applies sets Library Panel focus through the same adapter first.
  The shared `FocusStack` and the shared keyboard router stay unchanged
  for the TUI.
- The router's destination steps open presented destinations through
  the GUI's open step. In the GUI, `App::apply_gui_router_command`
  intercepts `NavigationIntent::NextLibraryTab`,
  `PreviousLibraryTab`, and `SetLibraryTab`, in the same way that it
  intercepts `MoveQueueRow`. Next and Previous step through the
  presented destinations in tab order and wrap. `SetLibraryTab` acts as
  `OpenDestination`. Each opens through the same path as a press on the
  destination's tab (`presents_open_position`,
  `open_gui_active_destination`), so the GUI never opens a destination
  it does not present. The window resets its per-destination state when
  the carried `current_tab` changes
  (`LibraryState::reset_for_destination`), so a router open looks like
  a tab press: the Narrow Hero overlay dismisses and an open row menu
  or item menu closes. `PanelIntent::FocusPanel` (`panel_right`,
  `panel_left`) is a documented no-op in the GUI, and the two actions
  become TUI-only in the `mbv-keybinds` registry.
- In the Mini arrangement, the window shows only the Queue panel. The
  pick is one function of the arrangement in the window; a later change
  replaces only that pick when it adds a tabbed Mini view. No key and
  no press changes the Mini panel. The GUI launch location saves no
  Panel focus, and a launch file that an earlier build saved still
  loads.

## Context

The GUI Client reused the TUI's focus model. One sticky Panel focus
decided where keys went, a Tab walk moved it, and each Library list
kept a keyboard cursor separate from the item that the Hero shows. A
GUI has a pointer: the user looks at and points at a list, and the keys
still went to the panel that they clicked last.

Blitz's hover resolution gave a better base. `BaseDocument::resolve`
re-resolves the hovered node from the last pointer position after each
layout and each scroll. It dispatches no `pointerenter` or
`pointerleave` event on a scroll under a still pointer, so per-row
enter events go stale, while `get_hover_node_id` stays correct.

## Considered options

- **Keep Panel focus and the Tab walk (rejected).** The pointer shows
  the target already. A second, sticky key-target state disagrees with
  what the user sees, and each Library cursor disagrees with the item
  that the Hero shows.
- **Per-row `onpointerenter`/`onpointerleave` (rejected).** These
  events do not fire on a scroll under a still pointer, so Enter would
  act on a row that is no longer under the pointer.
- **Geometry derivation (rejected).** Last pointer position, list
  rect, scroll offset, and row tops give the target. This is exact only
  for the virtual lists, and it repeats the hit testing that Blitz
  already does.
- **The core's hit map (rejected).** The Library reports no regions.
  Adding every Library row to the `LayoutReport` would put
  window-local interaction state back into the core
  ([ADR 0036](0036-gui-library-panel-owns-its-interaction-state.md)).
- **The hover-node walk with DOM markers (chosen).** One resolver, the
  same node that paints the hover look, correct after a scroll, and a
  pure function that a unit test covers without Blitz.

## Consequences

- The window keeps no copy of the core's focus. The frame's focus
  section, the take-focus commands, and the launch Panel-focus restore
  go. The window content holds no Library cursor and no focus inside
  the panel.
- A key that falls between a scroll and its redraw uses the target from
  before the scroll. The gap is at most one frame, and the key then
  acts on the row that showed the hover look when the key went down.
- The press-to-focus habit ends. A user who clicks the Queue and then
  types with the pointer over the Library sends the keys to the
  Library. The Queue's active row stays visible, so the state does not
  hide.
- `openspec/specs/gui-client/spec.md` and
  `openspec/specs/client-interaction-core/spec.md` own the current
  behavior.
