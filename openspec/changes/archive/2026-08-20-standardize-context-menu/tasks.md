## 1. Anchor and placement model

- [x] 1.1 Add selected-item and pointer anchor kinds to `ContextMenu`; keep
      menu-entry construction independent of selected-item geometry.
- [x] 1.2 Extract one rendered menu-size calculation shared by placement and
      `render_context_menu`.
- [x] 1.3 Add a pure, saturating placement function taking anchor geometry,
      containing panel, and menu size: right-align selected items, prefer down,
      flip up, then clamp inside the panel.
- [x] 1.4 Add table-driven tests for down/up placement, horizontal and vertical
      clamping, pointer placement, missing selected geometry, zero dimensions,
      and a menu larger than its panel.

## 2. Publish authoritative selected-item rectangles

- [x] 2.1 Add `selected_item_rect: Option<Rect>` and
      `queue_selected_item_rect: Option<Rect>` to `LayoutMain` alongside the
      old y-only fields during migration.
- [x] 2.2 Add one selected-cell geometry helper beside
      `draw_column_selection_markers`, using the same `left_item_rows`, offset,
      cell-width, and column-gap inputs.
- [x] 2.3 Update `list_plain.rs` and `list_letter_groups.rs` to publish the
      selected row/cell rect through that helper in both one- and two-column
      modes.
- [x] 2.4 Update grouped album, album-detail track, wide-Music track/browser,
      Home list, expanded Emby feed/video, and queue renderers to publish their
      existing authoritative row/cell rects. The expanded item's outer renderer
      owns its full selectable item rect.
- [x] 2.5 Remove cursor-geometry writes from nested detail/hero renderers so
      they cannot overwrite the outer selectable anchor.
- [x] 2.6 Remove obsolete y-coordinate writes from Audiobookshelf renderers
      without adding rect writes; Audiobookshelf and Feeds remain unsupported.
- [x] 2.7 Add focused renderer tests for the shared one-column path, shared
      two-column left/right cells, grouped album, expanded item, wide Music,
      Home, and queue. Do not claim nonexistent per-view legacy coverage.

## 3. Resolve and render anchors each frame

- [x] 3.1 Resolve selected-item anchors from the fresh local frame layout in
      `render_context_menu`; resolve pointer anchors directly from their click
      point.
- [x] 3.2 Use the shared size and placement functions for both anchor kinds and
      keep `open_context_menu_at` independent of selected-item geometry.
- [x] 3.3 Remove `cursor_screen_y`, `queue_cursor_screen_y`,
      `context_menu_spawn_point`, the old inline-image avoidance branch, and all
      remaining readers/writers once migration is complete.
- [x] 3.4 Add integration tests showing keyboard placement follows fresh layout
      after resize and mouse placement remains click-anchored.

## 4. Modal presentation and coexistence

- [x] 4.1 Add `context_menu.is_some()` to `any_dim_modal_open` so images select
      the existing half-block modal path before main content renders.
- [x] 4.2 Call `dim_backdrop` before drawing the context menu.
- [x] 4.3 Refuse context-menu opening while another modal or sidebar surface is
      active; close the menu before mandatory asynchronous modals activate and
      before a selected action executes.
- [x] 4.4 Add a render-time debug assertion and tests for the one-modal-at-a-time
      invariant, single backdrop application, and undimmed menu foreground.

## 5. Exclusive input ownership

- [x] 5.1 Add `handle_key_context_menu`: Up/Down wrap among selectable entries
      while skipping separators, Enter closes then executes once, Esc closes
      without acting, and every other key is claimed as a no-op.
- [x] 5.2 Put `context_menu` first in `CONTEXT_STACK` and update the pinned stack
      order test.
- [x] 5.3 Remove redundant `context_menu_open()` guards from lower keyboard
      handlers and re-point their regression tests through the authoritative
      context-menu entry.
- [x] 5.4 Add regression tests proving `.`, F1-F4, Ctrl+/, Tab/BackTab, 1-9,
      refresh, playback, mutation, and ordinary view keys are swallowed while
      the menu remains open.
- [x] 5.5 Preserve actionable/outside menu clicks and swallow wheel and other
      non-menu mouse events while the menu is open.
- [x] 5.6 Update `docs/adr/0002-centralized-input-handling.md` with the explicit
      context-menu priority and one-modal replacement invariant.

## 6. Verification

- [x] 6.1 `cargo nextest run -p mbv` covering the geometry, rendering, input,
      mouse, dim-image, and modal-invariant tests above.
- [x] 6.2 `cargo clippy --workspace --all-targets`.
- [x] 6.3 `make check-code-file-lines`.
- [x] 6.4 Manual pass: keyboard-open from one-column, two-column left/right,
      grouped album, expanded item, wide Music, Home, and queue selections near
      each panel edge; resize while open; confirm flip/clamp, half-block images,
      exclusive keys, pointer anchoring, and Esc dismissal.
