## Context

The music library grouped album view builds a flat display plan (`Vec<GroupedAlbumDisplayRow>`) each frame in `build_grouped_album_display_plan()` (`album_plan.rs`). In the selected-group branch (lines 201–296), the plan currently:

1. Pushes all visible album rows (lines 241–247)
2. Then pushes track detail rows after all albums (lines 249–280)

This causes tracks to appear at the bottom of the colored block, disconnected from their parent album. The rendering code in `album.rs` iterates the flat row list and handles each variant generically — it does not assume tracks come after all albums.

## Goals / Non-Goals

**Goals:**
- Track detail rows for the focused album appear immediately after that album's row (and its wrapped continuation rows) within the selected group block.
- Albums after the cursor album shift below the track detail rows.
- Colored block framing (borders, padding, album art reservation) continues to work correctly.
- Cursor navigation and scrolling remain correct.

**Non-Goals:**
- No changes to the non-grouped (plain) album view — it already renders tracks inline.
- No changes to the artist-header-selected mode.
- No changes to input handling or track playback actions.
- No new dependencies (tui-tree-widget considered and rejected — see Decisions).

## Decisions

### 1. Move track insertion inside the album loop

In the `selected_group` branch of `build_grouped_album_display_plan` (album_plan.rs:241–280), move the track detail row insertion from after the album loop into the loop body, gated on `idx == cursor`.

**Before** (simplified):
```rust
for &idx in visible_group_indices {
    rows.push(Album(idx));
    rows.extend(wrapped_continuations(idx));
}
if !header_selected && expand_selected && group_contains_cursor {
    // push track rows
}
```

**After** (simplified):
```rust
for &idx in visible_group_indices {
    rows.push(Album(idx));
    rows.extend(wrapped_continuations(idx));
    if !header_selected && expand_selected && idx == cursor {
        // push track rows (same logic as before)
    }
}
```

**Rationale**: Minimal diff. The track detail logic is unchanged — only its position in the row list moves. The rendering code in `album.rs` already handles `AlbumDetailStart`/`AlbumDetailContinuation` rows generically by position, so no rendering changes are needed.

**Alternative considered**: Using `tui-tree-widget` to model the artist → album → track hierarchy as a tree. Rejected because: (a) the existing flat display-plan architecture maps directly to terminal rows and is well-tested; (b) a tree widget would require restructuring the plan into a tree model, rewriting the rendering loop, and reworking cursor navigation — a large refactor for a visual reordering; (c) the flat plan already handles all the edge cases (wrapping, art reservation, block bounds, scrolling) that a tree widget would need to replicate.

### 2. Block height and padding

The art padding logic (album_plan.rs:282–292) fills remaining rows to reach `art_bottom`. With tracks inserted inline, the total row count may exceed `art_bottom` earlier. The `saturating_sub` guard already handles this — no padding rows are added when content exceeds the art height. No change needed.

### 3. Block bounds

`selected_block_bounds` is set from `top_idx` (after the top border) to `bottom_idx` (before the bottom border). These indices are computed from `rows.len()` after all content is pushed. Since the total row count is the same (same albums + same tracks, just reordered), the bounds remain correct. No change needed.

### 4. Cursor navigation

`display_cursor` finds the position of `Album(cursor)` in the row list. The album row is still present at a different index. Navigation code that uses `display_cursor` for scroll clamping will still work. Track-mode cursor movement (`album_track_focus`) operates on track indices, not display row indices, so it is unaffected.

**Risk**: If any code assumes track rows always come after all album rows (e.g., for scroll-to-track), it may need adjustment. → Mitigation: The rendering code uses row positions from the display plan directly; no such assumption exists in the current code.

## Risks / Trade-offs

- [Risk] With tracks inline, the cursor album may be pushed above the visible area if many tracks are shown. → Mitigation: The existing `SELECTED_ALBUM_WINDOW` (12 albums) and scroll clamping already handle this. The display cursor tracks the album row position, and scroll offset adjusts to keep it visible.
- [Risk] The colored block may become very tall with many tracks, pushing album art below the visible area. → Mitigation: This is acceptable — the user explicitly entered track-selection mode and expects to see tracks. The art area is a secondary visual element.
- [Risk] Wrapped album title rows between the cursor album and the track list may create visual ambiguity. → Mitigation: The `AlbumDetailStart` row renders the track table with its own formatting, providing clear visual separation.
