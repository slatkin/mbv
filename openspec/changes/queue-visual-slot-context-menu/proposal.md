# Proposal

## Why

The queue visual slot's artwork/visualizer controls are keyboard-only: `v`
switches the content and `h` hides the slot. A pointer user has no way to
switch the image to the visualizer, switch back, or hide it. The slot is the
app's most prominent image surface, and every selectable list surface in the
app already answers a right-click with the standard context menu.

## What Changes

- A right-click on the queue visual slot's image rectangle opens the
  standard context menu at the click point.
- The menu offers the content the image is not showing (artwork or
  visualizer) and hiding the slot.
- The entries reuse the same state paths as the `v` and `h` keyboard
  actions. No new playback, capture, or display state is added.
- The menu is pointer-only: the visual slot paints no selectable row, so it
  has no keyboard anchor. This is a deliberate exception to the
  right-click-parity rule for row menus.
- The menu is clamped inside the queue column and does not move panel focus.
- The menu uses the existing context-menu modal, including the
  dimmed-backdrop halfblock image path while it is open.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `context-menu`: adds the pointer-only menu surface on the queue visual
  slot, clamped inside the queue column, and records the pointer-only
  exception to the row parity rule.
- `queue-playback-panel`: the visual slot's hide action is also reachable
  from the slot's menu. The persisted-state semantics do not change.
- `system-audio-visualizer`: the artwork/visualizer selection is also
  reachable from the slot's menu. `v` stays the keyboard route.

## Impact

- `crates/mbv-ui-msg`: the new `ShellRequest` variant.
- `crates/mbv-components`: `QueuePlaybackPanel` gesture and
  `ContextMenuComponent` placement bounds.
- `crates/mbv-ui-model`: the new `ContextAction` variants and
  `ContextMenu.bounds`.
- `src/app`: the shell sync pass, the menu builder, the context-action
  dispatcher, and the visual state setters.
- No provider, daemon, ctrl-protocol, or persistence-format changes.
