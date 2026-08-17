## 1. Anchor data model

- [ ] 1.1 Add `selected_item_rect: Option<Rect>` and
      `queue_selected_item_rect: Option<Rect>` to `AppLayout` (`src/app/layout.rs`),
      alongside the existing `cursor_screen_y` / `queue_cursor_screen_y` fields.
- [ ] 1.2 Write the anchor/flip function: given the selected item's `Rect` and
      the containing area, return the menu's `(x, y)` per the positioning
      requirement in `specs/context-menu/spec.md` (right-align, flip up if it
      doesn't fit below).

## 2. Wire render call sites to the new rect fields

- [ ] 2.1 `render/list.rs`, `render/list_plain.rs`, `render/list_letter_groups.rs`:
      set `selected_item_rect` alongside `cursor_screen_y`.
- [ ] 2.2 `render/home.rs`, `render/home_feed.rs`, `render/home_video.rs`,
      `render/detail.rs`: set `selected_item_rect` alongside `cursor_screen_y`.
- [ ] 2.3 `render/album.rs`, `render/album_detail.rs`: derive
      `selected_item_rect` from the existing `left_item_rows` row/column
      mapping (the same data `draw_column_selection_markers` in
      `list_rows.rs` already uses) so the rect reflects the selected cell's
      actual column position and width, not the full panel width.
- [ ] 2.4 `render/music_wide.rs`, `render/music_wide_browser.rs`,
      `render/audiobookshelf.rs`, `render/audiobookshelf_book_browser.rs`:
      set `selected_item_rect` alongside `cursor_screen_y`.
- [ ] 2.5 `render/queue.rs`: set `queue_selected_item_rect` alongside
      `queue_cursor_screen_y`.

## 3. Switch positioning to the anchor, remove the old fields

- [ ] 3.1 Replace `context_menu_spawn_point` (`input_context_menu.rs`) with
      the anchor/flip function from 1.2, reading `selected_item_rect` /
      `queue_selected_item_rect`; keep `open_context_menu_at` (mouse path)
      untouched.
- [ ] 3.2 Remove `cursor_screen_y` and `queue_cursor_screen_y` from
      `AppLayout` and all call sites updated in section 2, now that nothing
      reads them.
- [ ] 3.3 Update/extend the existing characterization tests for spawn
      position (one per view, matching current `cursor_screen_y` test
      coverage) to assert against the new rect-based anchor instead.

## 4. Dim backdrop

- [ ] 4.1 Call `dim_backdrop` from `render_context_menu`
      (`render/overlays/context_menu.rs`), matching how `render_modal_frame`
      calls it for other overlays.
- [ ] 4.2 Add `self.context_menu.is_some()` to `any_dim_modal_open`
      (`render/mod.rs`).

## 5. Keyboard navigation for the open menu

- [ ] 5.1 Add `handle_key_context_menu` (Up/Down move `cursor`, skipping
      non-selectable entries per `ContextMenu::first_selectable`'s existing
      skip rule; Enter calls `execute_context_action` and closes the menu;
      Esc closes the menu without acting; any other key while open is
      swallowed).
- [ ] 5.2 Add a `context_menu` entry to `CONTEXT_STACK`
      (`input_resolver.rs`), positioned above every entry whose
      `context_menu_open()` guard exists solely to avoid double-handling a
      key while the menu is open.
- [ ] 5.3 Remove the now-redundant `context_menu_open()` guards in
      `input_lib_keys.rs`, `input_queue_keys.rs`, `input_confirm_keys.rs`,
      re-pointing their existing regression tests (e.g. the `c`/`x`
      leak-through tests) at the new `context_menu` stack entry.
- [ ] 5.4 Make `.` a no-op while the menu is already open (covered
      automatically once 5.2 gives the new entry priority, but add a
      regression test asserting it doesn't reopen/reset the menu).

## 6. Verification

- [ ] 6.1 `cargo nextest run -p mbv` (or the appropriate package) covering
      the new/updated tests from sections 3, 5.
- [ ] 6.2 `cargo clippy --workspace --all-targets`.
- [ ] 6.3 Manual pass: open the context menu via keyboard from a single-column
      list, a two-column album/track view, and the queue panel, near the top
      and near the bottom of the visible area; confirm anchor, flip, dim
      backdrop, and Up/Down/Enter/Esc all behave per spec.
