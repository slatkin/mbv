## 1. Primitive

- [ ] 1.1 Add `selected_row_background(f, panel, row_y, focused)` beside
  `resolve_surface_focus` / `selection_marker`; paints one row of the
  focus-resolved focused surface across `panel`'s full inner width.
  → verify: unit test asserts the painted rect is `panel.x..panel.right()` at
  `row_y` and the bg is `SURFACE_FOCUSED` when focused.

## 2. Swap call sites (all in one change)

- [ ] 2.1 `queue.rs:404` and `:486` → call primitive with the queue `area`.
- [ ] 2.2 `render_plain_rows` / `render_letter_grouped_rows` (Emby libraries) →
  paint the primitive on the list content panel for the selected row.
- [ ] 2.3 `music_wide.rs:400` → primitive with `track_panel`.
- [ ] 2.4 `audiobookshelf_book_browser.rs:270` → primitive with `list_panel`
  (drop the inset `browser_area` width).
- [ ] 2.5 `album.rs:523` and `album_detail.rs:345` → primitive with the row panel
  (drop the art-reserved width).
- [ ] 2.6 `selection_modal.rs:91` → primitive with the modal list panel.
- [ ] 2.7 Delete the `SURFACE_RESTING` selected-row pad in `item_cell_spans`.

## 3. Verify

- [ ] 3.1 Remove now-dead per-site rect/colour math flagged by clippy.
- [ ] 3.2 Explicit buffer check: pick two surfaces (queue + one library) and
  confirm the selected-row bg is byte-identical colour and full-panel width.
- [ ] 3.3 `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
  --all-targets`, `rtk make check-code-file-lines`.
- [ ] 3.4 Manual: libraries now show the darker full-width selection; judged
  correct, matches queue.
