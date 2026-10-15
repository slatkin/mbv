# Design

## Context

See proposal.md. The queue visual slot (`QueuePlaybackPanel` placement) shows
the now-playing artwork, the loading placeholder, or the system-audio
visualizer in one rectangle. `v` selects the content; `h` hides the slot. The
shell paints the slot on the panel's behalf, and the panel already receives a
shell-computed transport hit rect each sync pass.

The context-menu overlay is the app's standard modal: a mounted
`ContextMenuComponent`, a pointer or selected-item anchor, a dimmed backdrop,
and the halfblock image path while it is open.

## Goals / Non-Goals

**Goals:**

- A right-click on the visual slot's image rectangle opens the standard menu.
- The two entries reuse the existing keyboard action state paths.
- The menu is placed and clamped without depending on panel focus.
- No new playback, capture, or persistence state.

**Non-Goals:**

- No keyboard chord for the menu itself.
- No changes to the panel's transport or header gestures.
- No changes to row context menus, their focus behavior, or their clamp.

## Decisions

### D1: The hit target is the card reservation, pushed by the shell

`sync_queue_playback_panel` computes `queue_card_reserved_rect` from the same
last-card checkpoint the paint path uses and pushes it with
`set_visual_slot_area`; the panel resolves a right-click inside it.

Alternative: the panel derives the full slot band from its placement.
Rejected: the band includes the blank space beside a centered narrow image
and, in the wide layout, is shared with the transport.

### D2: Hide sets the persisted `h` state

The hide entry runs the same hidden-state transition as `h`
(`visual_slot_hidden`, persisted). After a hide the image is gone, so `h` is
the way back; the menu cannot reopen.

Alternative: a session-only or item-scoped hide. Rejected: it would add a
second hidden state the `h` action does not know, for no observed user need.

### D3: The menu names its containing panel

`ContextMenu` gains an optional `bounds` rect. The slot menu sets the queue
column (`layout.left_area`); the renderer's clamp prefers it. Row menus keep
`None` and their existing destination-derived panel.

Alternative: focus the Queue panel on the right-click and reuse the row-menu
clamp. Rejected: a right-click on chrome must not yank focus from the library,
and the row clamp is the list box under the image.

### D4: Actions carry the target content, not a toggle

Three `ContextAction` variants: `ShowVisualizer`, `ShowMediaArt`,
`HideVisualSlot`. The menu resolves the label against the state it sees when
it opens, and each action names the state to enter.

Alternative: reuse `Command::ToggleVisualizer` / `ToggleVisualSlotHidden`.
Rejected: a toggle reads the state at execution time; the label and the
effect could disagree if the state changes between open and select.

### D5: The menu is pointer-only

The slot paints no selectable row and has no keyboard anchor, so there is no
keyboard entry point. `v` and `h` remain the keyboard routes to the same two
state changes; the `context-menu` delta records the deliberate exception to
the row parity rule.

Alternative: a new rebindable chord that opens the same menu. Rejected: it
adds keybind, help, and conflict surface for entries that already have direct
keys.

### D6: Labels

`Show Visualizer` while artwork shows; `Show Media Art` while the visualizer
shows; `Hide` as the second entry. Each label names the content the image is
not showing.

### D7: Idle and remote parity

No menu exists while the slot is idle or hidden (`visual_slot_shown` gates
both the hit rect and the build). A remote Session or cast keeps
`Show Visualizer` offered, exactly like `v` today; capture stays off and the
box stays empty.

## Risks / Trade-offs

- Click anchor and clamp can separate near the queue-column edge: the menu
  stays visible inside the column instead of at the exact click.
  -> Mitigation: it drops to the column edge, the same clamp policy every
  menu uses.
- The hit rect follows the recorded card checkpoint and does not move while
  the menu is open.
  -> Mitigation: the dim halfblock paint deliberately does not update that
  checkpoint (#451), so the rect and the painted image stay aligned under the
  modal.
- The slot path adds a second gate on `visual_slot_shown` (hit rect and menu
  build).
  -> Mitigation: both derive from the one predicate; a stale rect cannot open
  a menu for a hidden slot.

## Migration Plan

No data or protocol migration. The feature ships with the code; no rollout or
rollback steps beyond the normal release.
