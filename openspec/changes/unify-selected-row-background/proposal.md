## Why

The selected-row background is hand-painted by ~six independent call sites, each
computing its own rect and, in some cases, its own colour. `queue.rs` paints a
full-width `SURFACE_FOCUSED` block; `render_plain_rows` (every Emby library) uses
a `SURFACE_RESTING` span-pad with no block at all; the book browser paints an
*inset* block; music, album, and the modal each roll their own. Selection looks
different across screens and drifts by construction, violating the existing
`ui-design-language` requirement that "two screens display[ing] the same concept,
such as a selected row" share one definition. There is a shared `selection_marker`
(the `▎` glyph) but no shared background.

## What Changes

- Add one `selected_row_background` primitive: paints `SURFACE_FOCUSED` across the
  **full width of the parent panel** (the `queue.rs` model), given the panel rect
  and the selected row's y.
- Replace every hand-rolled selection background with a call to it: queue,
  Emby libraries (`render_plain_rows` / `render_letter_grouped_rows`), music wide
  tracks, ABS book browser, album / album_detail, and the selection modal.
- **BREAKING (visual):** Emby library selection changes from a `SURFACE_RESTING`
  span-pad to the `SURFACE_FOCUSED` full-width block — libraries and queue match
  after this.
- Remove the now-dead per-site rect/colour math and the `SURFACE_RESTING`
  selection pad in `item_cell_spans`.

## Capabilities

### New Capabilities
- `selected-row-highlight`: one component owns the selected-row background —
  colour role (`SURFACE_FOCUSED`, focus-resolved) and geometry (full parent-panel
  width, one row high). Every list-bearing surface paints selection through it;
  no surface computes its own selection rect or colour.

### Modified Capabilities
- None. This enforces the existing `ui-design-language` requirement (two screens
  showing the same concept share one definition) by adding the component that
  requirement always implied; no design-language behaviour changes.

## Impact

- `src/app/render/screens/queue.rs`, `src/app/render/components/list_plain.rs`
  (`render_plain_rows`), `list_rows.rs` (`item_cell_spans`), `music_wide.rs`,
  `audiobookshelf_book_browser.rs`, `album.rs`, `album_detail.rs`,
  `selection_modal.rs`, and the palette module (home of the new primitive).
- No protocol, provider, or daemon surface. Pure TUI painting.
- Blocks `audit-hero-on-left-arrangement` (that audit calls this primitive rather
  than re-deriving per-surface selection paints).
