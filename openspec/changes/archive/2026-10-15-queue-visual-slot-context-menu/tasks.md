# Tasks

## 1. Gesture and shell request

- [x] 1.1 Add `ShellRequest::VisualSlotContextMenu { x: u16, y: u16 }` next to
  `RowContextMenu` in `crates/mbv-ui-msg/src/shell.rs`; verify with
  `cargo check -p mbv-ui-msg`.
- [x] 1.2 Add the `visual_slot_area` field, `set_visual_slot_area(Option<Rect>)`
  (zero-size clears it), and the right-click arm to `QueuePlaybackPanel`;
  verify with `visual_slot_right_click_reports_the_image_anchor` in
  `crates/mbv-components/src/queue_playback_panel/tests.rs` (inside/outside/
  no-rect cases).
- [x] 1.3 Push the hit rect from `sync_queue_playback_panel` using
  `queue_card_reserved_rect` over the same last-card checkpoint the paint
  path uses, gated on `visual_slot_shown()`; verify via the panel accessor and
  the end-to-end tick test (task 4.1).

## 2. Menu build and placement

- [x] 2.1 Add `ContextMenu.bounds: Option<Rect>`, carry it through
  `ContextMenuComponent::set_content`/`bounds()`, and let
  `context_menu_rect` prefer it over the destination-derived panel; verify
  with `visual_slot_menu_clamps_inside_the_queue_column` in
  `src/app/tests/context_menu_placement.rs` (focus stays on Library, menu
  stays inside the queue column).
- [x] 2.2 Add `App::open_visual_slot_context_menu(x, y)` building the two
  entries and the queue-column bounds, gated on `visual_slot_shown()`; wire
  `ShellRequest::VisualSlotContextMenu` in `handle_shell_request` under the
  `!blocking_overlay_active()` guard; verify with
  `visual_slot_menu_names_the_other_content` in
  `src/app/tests/context_menu_entries.rs`.

## 3. Actions and visual state

- [x] 3.1 Add `ContextAction::ShowVisualizer`, `ShowMediaArt`, and
  `HideVisualSlot`, and route them in `execute_context_visual_action` ahead of
  the selection chains; verify with
  `visual_slot_content_entry_selects_the_named_content` and
  `visual_slot_hide_entry_hides_the_slot` in
  `src/app/tests/context_menu_entries.rs`.
- [x] 3.2 Factor `App::set_visualizer(bool)` out of `toggle_visualizer` and add
  `App::set_visual_slot_hidden(bool)`; make the `Command::ToggleVisualSlotHidden`
  arm use the setter; verify the existing visualizer action tests still pass
  (`cargo nextest run -p mbv visual_slot`).

## 4. End-to-end and gates

- [x] 4.1 End-to-end tick test: right-click the projected slot rect, the menu
  mounts, Down+Enter selects Hide, the slot hides; verify with
  `tick_visual_slot_right_click_opens_the_menu_and_hide_hides_the_slot` in
  `src/app/tests/tick_integration/mouse.rs`.
- [x] 4.2 Run `cargo fmt --all`, `cargo nextest run -p mbv-components`,
  `cargo nextest run -p mbv-ui-model`, `cargo nextest run -p mbv`, and
  `cargo clippy --workspace --all-targets -- -D warnings`; all green before
  commit.

## Workflow follow-up

- After the review agent's findings are resolved, archive this change to sync
  the `context-menu`, `queue-playback-panel`, and `system-audio-visualizer`
  deltas into `openspec/specs/`.
