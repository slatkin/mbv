## Why

In the music library's grouped album view, when a user enters track-selection mode on an album within the selected artist group block, the track list appears at the bottom of the block — after all visible album rows — rather than directly under the parent album. This makes it unclear which album the tracks belong to, especially when the group has multiple albums visible. Tracks should appear inline, directly under their parent album row, so the visual association is immediate.

## What Changes

- Modify the display plan builder (`build_grouped_album_display_plan`) so that in the selected-group branch, track detail rows for the cursor album are inserted immediately after that album's row (and its wrapped continuation rows), rather than after all album rows.
- Remaining album rows that appear after the cursor album are shifted below the track detail rows.
- The colored block framing (borders, padding, album art reservation) continues to encompass all content.
- No new dependencies. The `tui-tree-widget` crate was considered but rejected — the existing display-plan architecture handles this cleanly with a targeted row-ordering change, and adding a tree widget would require significant refactoring of the rendering pipeline for no structural benefit.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `inline-track-selection-in-group`: Track list for the focused album in a selected artist group block renders inline under the parent album row instead of at the bottom of the block.

## Impact

- Affected code: display plan construction in `src/app/render/album_plan.rs` (row ordering in the `selected_group` branch).
- Rendering code in `src/app/render/album.rs` needs no changes — it already handles `AlbumDetailStart`/`AlbumDetailContinuation` rows generically.
- Cursor navigation in `src/app/render/album_cursor.rs` may need adjustment if track-mode cursor movement assumes the old row layout.
- No changes to state model, input handling, or persistence.
- No new dependencies.
