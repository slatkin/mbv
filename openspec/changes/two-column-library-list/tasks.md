## 1. Column geometry

- [x] 1.1 Add a column-geometry helper module beside `src/app/queue_column_width.rs` defining `LIBRARY_COLUMN_MIN_WIDTH` (40) and `LIBRARY_COLUMN_GAP` (2), and a function returning the column count for a given list pane width, capped at 2.
- [x] 1.2 Add a function returning the cell `Rect` for a given column index within a content area, so the renderer and any hit testing derive cell geometry from one place.
- [x] 1.3 Compute the list pane width in `render_power_list` from the content area it already receives, so `queue_column_collapsed` and queue column resizing feed through without a separate code path.
- [x] 1.4 Unit-test the threshold boundary in both directions and the cell rect arithmetic, including the case where the content width does not divide evenly.

## 2. Multi-item display rows

- [x] 2.1 Replace `DisplayRow::Item(usize)` in `src/app/render/list_rows.rs` with a row carrying the item indices it holds in column order; update the enum's construction and match sites.
- [x] 2.2 Build item rows row-major from the column count, so one-column mode produces single-index rows and shares the same path.
- [x] 2.3 Keep `Spacer`, `LetterHeader`, `BannerFiller`, and `SeriesDetailFiller` full width and unchanged, and keep detail fillers inserted below the row containing the cursor without displacing that row's other item.
- [x] 2.4 Pack each letter bucket independently in `list_letter_groups.rs` so no row mixes items from two buckets.
- [x] 2.5 Retain a row map from item index to display row for both renderers, since independent bucket packing means item index no longer maps to row by division.

## 3. Render items into cells

- [x] 3.1 Render each item into its cell `Rect` rather than the full content rect, reusing the existing marker, title, metadata, and truncation logic against the narrower width.
- [x] 3.2 Render empty trailing cells as plain list background.
- [x] 3.3 Keep the per-frame `ListState` highlight consistent with the new row structure, or replace it where a multi-item row makes single-row highlight meaningless.
- [x] 3.4 Verify the scroll indicator (`render_power_right_scrollbar`) reflects display rows so a list that fits in two columns shows no indicator.

## 4. Notched selected block

- [x] 4.1 Extend `render_selected_block_background` in `src/app/render/power_widgets.rs` to paint a tab region at a supplied slot rect and a panel region at full width, with no seam between them.
- [x] 4.2 Pass the selected cell's slot from both `list_plain.rs` and `list_letter_groups.rs`, covering the top padding row and the item row.
- [x] 4.3 Confirm the top padding row narrows with the tab and does not band across the partner cell.
- [x] 4.4 Confirm one-column mode produces the current rectangular block unchanged.
- [x] 4.5 Confirm the focused and unfocused background colors apply to both regions.

## 5. Cursor movement

- [x] 5.1 Use the column count to choose deltas at the key-handling site in `src/app/input_lib_power_keys.rs`: left/right ∓1/±1, up/down ∓cols/±cols, page up/down by one viewport of item rows.
- [x] 5.2 Move up/down through the row map in letter-grouped mode rather than by adding `cols` to the item index.
- [x] 5.3 Leave `jump_lib_cursor` (home/end) and the music group-view and grouped-album special cases behaving as they do today.
- [x] 5.4 Leave the season grid path (`is_viewing_season_grid`) untouched.
- [x] 5.5 Verify down from the second-to-last row with no item directly below clamps to the last item.

## 6. Scroll and visibility

- [x] 6.1 Substitute the row containing the cursor for `display_cursor` in the scroll clamp and filler walkback in `list_plain.rs`.
- [x] 6.2 Apply the same substitution to the header-aware copy of that logic in `list_letter_groups.rs`.
- [x] 6.3 Verify the tab cannot scroll out of view while its panel remains visible, in both renderers.
- [x] 6.4 Verify the stored scroll write-back in `list.rs` still lands on a valid row after a column count change.

## 7. Tests

- [x] 7.1 Test row-major placement: consecutive items occupy consecutive cells left to right before wrapping.
- [x] 7.2 Test that changing viewport height leaves every item in the same column and relative row.
- [x] 7.3 Test that a selected item's inline banner renders full width below its row and leaves that row's other item in place.
- [x] 7.4 Test that moving the cursor between adjacent items does not change any item's column.
- [x] 7.5 Test independent bucket packing with an odd-sized bucket.
- [x] 7.6 Test the notched block for a left-column and a right-column selection, and the rectangular block in one-column mode.
- [x] 7.7 Test cursor deltas for left/right, up/down, and paging in two-column mode, including the row-boundary wrap and the end-of-list clamp.
- [x] 7.8 Test that crossing the column threshold in both directions preserves the selected item and scrolls it into view.
- [x] 7.9 Test that a narrow pane renders identically to today's single-column output.

## 8. Verify

- [x] 8.1 Run `cargo fmt --all -- --check`.
- [x] 8.2 Run `cargo check --workspace --all-targets`.
- [x] 8.3 Run the library list renderer tests.
- [ ] 8.4 Verify visually in a real terminal at several widths: just below the threshold, just above it, and very wide.
- [ ] 8.5 Verify visually with the queue column collapsed, at default width, and at maximum width.
- [ ] 8.6 Check the named visual risks: the background fill across the inter-column gap, and a right-column tab over a full-width panel.
- [ ] 8.7 Confirm whether `LIBRARY_COLUMN_MIN_WIDTH` should rise from 40 based on observed truncation, and resolve the design's open questions.
