# Queue visual slot right-click menu

## Context

The queue column's visual slot shows the now-playing artwork, the loading
placeholder, or the system-audio visualizer. Two keyboard actions control it
today:

- `v` (`visualizer` action) switches between artwork and the visualizer. The
  selection is session-local (ADR 0009).
- `h` (`hide_visual_slot` action) hides the slot. The hidden state persists
  across launches (`visual_slot_hidden`).

The visual slot has no pointer menu. A right-click on the slot reaches the
mounted `QueuePlaybackPanel`. That component handles left-clicks on the
playback-target indicator and the transport only. It drops right-clicks.

The user wants a right-click menu on the image with two entries:

1. `Show Visualizer`, which reads `Show Media Art` while the visualizer shows.
2. `Hide`.

The menu uses the standard context-menu modal. The dimmed backdrop and the
halfblock image path already exist. The dim forces halfblocks on every
dimming overlay (`dimmed-backdrop-images` spec). No new work is needed for
that part.

## Decisions (user-confirmed)

- Hide sets the same persisted `visual_slot_hidden` state as `h`. After a
  hide, `h` is the way back, because the image itself is gone.
- The hit target is the image rectangle (the card reservation), not the full
  slot band. Blank space beside a centered narrow image opens nothing.
- The menu is right-click only. `v` and `h` stay the keyboard routes to the
  same two actions. The `context-menu` spec gains one explicit pointer-only
  exception for this surface.
- Labels: `Show Visualizer`, `Show Media Art`, `Hide`. The codebase spelling
  is `Visualizer`; the user wrote `Visualiser`. This plan uses the codebase
  spelling.
- The menu clamps inside the queue column (`layout.left_area`). It does not
  move panel focus. The queue row menus keep their current focus-and-clamp
  behavior.
- While idle or hidden the slot does not exist, so no menu opens.
- A remote Session or a cast leaves `Show Visualizer` offered, exactly like
  `v` today. Capture stays off, so the box stays empty.

## Approach

```text
right-click on the card rect
  QueuePlaybackPanel::mouse()
    -> Msg::Shell(ShellRequest::VisualSlotContextMenu { x, y })
  Model::handle_shell_request
    -> App::open_visual_slot_context_menu(x, y)
       gate: visual_slot_shown() and no blocking overlay
       entries from visualizer_enabled
       pointer anchor + bounds = queue column
  ContextMenuComponent + context_menu_rect   (existing dim + halfblock path)
  entry select
    -> execute_context_action
       ShowVisualizer -> App::set_visualizer(true)
       ShowMediaArt   -> App::set_visualizer(false)
       HideVisualSlot -> App::set_visual_slot_hidden(true)
```

The panel learns the hit rectangle from the shell, like it already learns the
transport rectangle. `sync_queue_playback_panel` computes the rect with
`queue_card_reserved_rect` from the same `last_card_size` checkpoint the
paint path uses, then pushes it with `set_visual_slot_area`. The shell pushes
`None` while the slot is idle or hidden.

The menu needs one new placement fact: its containing panel. Today a
pointer-anchored menu clamps inside the panel chosen by
`effective_panel_focus()`. The visual slot is chrome, so the image menu must
not depend on focus. `ContextMenu` gains an optional `bounds: Option<Rect>`.
The shell sets the queue column for this menu only. Every existing menu
passes `None` and keeps today's behavior.

## Files to modify

- `crates/mbv-ui-msg/src/shell.rs` — add
  `ShellRequest::VisualSlotContextMenu { x: u16, y: u16 }`.
- `crates/mbv-components/src/queue_playback_panel.rs` — new
  `visual_slot_area: Rect` field, `set_visual_slot_area(Option<Rect>)`, and a
  right-click arm in `mouse()`.
- `crates/mbv-ui-model/src/context_menu.rs` — add
  `ContextAction::ShowVisualizer`, `ContextAction::ShowMediaArt`,
  `ContextAction::HideVisualSlot`, and `ContextMenu.bounds: Option<Rect>`.
- `crates/mbv-components/src/context_menu.rs` — store the bounds; `set_content`
  takes them and a `bounds()` reader returns them.
- `crates/mbv-render` — none. The card rect function is reused as-is.
- `src/app/shell/chrome_panels.rs` — push the hit rect in
  `sync_queue_playback_panel`.
- `src/app/shell/messages.rs` — handle the new shell request.
- `src/app/shell/overlays/modals.rs` — `mount_context_menu` passes
  `menu.bounds` to the component.
- `src/app/shell/overlays/menus.rs` — `context_menu_rect` prefers the bounds
  override.
- `src/app/dispatch/context_menu/menu.rs` — `open_visual_slot_context_menu`;
  the existing `ContextMenu` literals gain `bounds: None`.
- `src/app/dispatch/context_menu/actions.rs` — one
  `execute_context_visual_action` step; the final exhaustive match lists the
  three new variants.
- `src/app/infra/visualizer.rs` — factor `set_visualizer(bool)` out of
  `toggle_visualizer`; add `set_visual_slot_hidden(bool)` next to it.
- `src/app/dispatch/action.rs` — the `Command::ToggleVisualSlotHidden` arm
  calls the new setter.

Tests:

- `crates/mbv-components/src/queue_playback_panel/tests.rs`
- `src/app/tests/context_menu_entries.rs` (entry sets and label flip)
- `src/app/tests/context_menu_placement.rs` (queue-column clamp)
- `src/app/tests/tick_integration/mouse.rs` (one end-to-end open and select)

## Reuse

- `queue_card_reserved_rect` (`crates/mbv-render/src/components/card.rs`): the
  one authority for the image rectangle. The paint path and the hit rect must
  call it with the same inputs.
- `ContextMenu::place` and `rendered_size`
  (`crates/mbv-ui-model/src/context_menu.rs`): the existing clamp math.
- The `ContextMenuComponent` overlay, `mount_context_menu`, and the
  `execute_context_action` chain: the whole modal path, including the dim
  backdrop and the halfblock image fallback.
- `App::toggle_visualizer` and the `Command::ToggleVisualSlotHidden` arm: the
  existing state paths the menu entries must call.
- The mouse eligibility ladder (`src/app/shell/library.rs`): it already sends
  mouse input to a blocking overlay alone, so a second menu cannot open.
- `visual_slot_hidden` prefs round trip
  (`src/app/input.rs::visual_slot_hidden_prefs_round_trip`): already covered.

## Steps

- [x] `ShellRequest::VisualSlotContextMenu { x, y }`.
- [x] Panel field, setter, and right-click arm. Return `None` on a
      zero-size rect.
- [x] Shell sync pass pushes the hit rect; gate on `visual_slot_shown()`.
- [x] Shell request arm calls `App::open_visual_slot_context_menu(x, y)`.
- [x] Menu builder: gate on `visual_slot_shown()` and
      `!blocking_overlay_active()`; build the two entries; set the anchor and
      the queue-column bounds (`layout.left_area`, `None` if empty).
- [x] `ContextMenu.bounds` plus component and render plumbing.
- [x] `set_visualizer(bool)` and `set_visual_slot_hidden(bool)`; dispatch the
      three new context actions.
- [x] Tests (see Verification).
- [x] `cargo fmt` for each Rust change.
- [x] `cargo nextest run -p mbv-components`, `cargo nextest run -p mbv-ui-model`,
      `cargo nextest run -p mbv`.
- [x] `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Commit after the user confirms. No push without the user's word.

## Verification

Contracts and layers (per the `docs/architecture/tui-frontend.md` matrix):

1. Component layer. A right-click inside the pushed slot rect emits
   `ShellRequest::VisualSlotContextMenu` with the click point. A right-click
   outside the rect, or with no pushed rect, emits nothing. Contract: the
   panel recognizes the image gesture and reports the anchor.
2. Shell layer. `visualizer_enabled == false` builds `[Show Visualizer,
   Hide]`; `true` builds `[Show Media Art, Hide]`. Selecting an entry runs
   the matching state change: `set_visualizer(true)`, `set_visualizer(false)`,
   or `visual_slot_hidden = true`.
3. Placement layer. The visual-slot menu stays inside `layout.left_area` when
   the click is near the column edge, and it does not follow the library
   panel.
4. Live check (user). Right-click the artwork: the menu opens and the image
   turns to halfblocks. Choose `Show Visualizer`: the vectorscope replaces
   the artwork. Reopen the menu: the first entry reads `Show Media Art`.
   Choose `Hide`: the slot collapses and the queue list takes the rows. Press
   `h`: the artwork returns.

No new test covers `v` or `h` behavior. Those paths do not change.

## Spec deltas (for `/opsx-propose`)

- `context-menu`: add a requirement for a pointer-only menu on the queue
  visual slot, clamped inside the queue column. Record the deliberate
  exception to the right-click-parity rule (this surface has no keyboard
  menu).
- `queue-playback-panel`: amend "The user can hide the visual slot". The hide
  action is also reachable from the visual slot menu; the persisted-state
  semantics do not change.
- `system-audio-visualizer`: amend "Queue card selects artwork or
  visualization". The selection is also reachable from the visual slot menu;
  `v` stays the keyboard route.
- `dimmed-backdrop-images`: no change. The spec already covers every dimming
  overlay.

Implementation starts from an OpenSpec change. This plan is the reviewed
design input, not the implementation. Use `/opsx-propose`, then
`/opsx-apply`.

## Risks and notes

- The hit rect follows the recorded card checkpoint, so it does not move while
  the menu is open. The dim halfblock paint deliberately does not update that
  checkpoint (#451).
- The menu steals no focus. This is deliberate and differs from the queue row
  menu.
- `hide_visual_slot` is missing from the help Global rows
  (`GLOBAL_CONFIGURABLE_ROWS`). Out of scope. The menu partly covers that
  discoverability gap.
- Section 2 tests should live with the existing entry-set tests in
  `src/app/tests/context_menu_entries.rs` instead of a new file, unless that
  file becomes unwieldy.
