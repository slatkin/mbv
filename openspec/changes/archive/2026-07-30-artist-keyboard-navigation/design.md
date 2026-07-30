## Context

The music library grouped album view renders a flat list of `LibraryRowTarget` entries (artist headers and albums) via `grouped_album_navigation_targets()`. Existing navigation moves one step at a time (Up/Down) or by page-sized jumps (PageUp/PageDown). There is no way to jump directly between artist boundaries.

The key handler `handle_lib_key()` in `input_lib_power_keys.rs` already dispatches PageUp/PageDown and catches unmapped Ctrl+key combos as no-ops. The cursor navigation logic lives in `album_cursor.rs`.

## Goals / Non-Goals

**Goals:**
- Ctrl+PageDown jumps the cursor to the next artist header in the grouped album view.
- Ctrl+PageUp jumps the cursor to the previous artist header.
- If no next/previous artist header exists, jump to the last/first item respectively.
- Scroll offset updates to keep the new position visible.

**Non-Goals:**
- No changes to the letter-grouped (non-music) list view.
- No changes to the plain (non-grouped) list view.
- No changes to the pill bar ([/] group switching).
- No changes to track-selection mode within an expanded album.

## Decisions

### 1. New method on `App` in `album_cursor.rs`

Add `jump_power_music_group_display_cursor_to_artist(&mut self, lib_idx: usize, forward: bool) -> bool` that:
1. Builds the display plan and target list (same as existing `move_power_music_group_display_cursor`).
2. Finds the current position in the target list.
3. Scans forward (or backward) for the next `LibraryRowTarget::ArtistHeader`.
4. If found, sets artist header focus. If not found, jumps to the last (or first) target.

**Rationale**: Follows the existing pattern of `move_power_music_group_display_cursor` and `jump_power_music_group_display_cursor`. Reuses the same plan-building and target-resolution logic. Keeps the change localized to `album_cursor.rs`.

**Alternative considered**: Reusing `move_power_music_group_display_cursor` with a large delta. Rejected because it would land on albums between artists, not on the artist header itself.

### 2. Key dispatch in `handle_lib_key`

Add `KeyCode::PageUp`/`PageDown` with `KeyModifiers::CONTROL` arms before the existing unmodified PageUp/PageDown arms. The Ctrl+modifier check must come first since crossterm reports `PageUp` with `CONTROL` modifier as `KeyCode::PageUp` + modifiers, not as a `Char`.

**Rationale**: crossterm encodes Ctrl+PageUp as `KeyEvent { code: PageUp, modifiers: CONTROL }`, so the match must check modifiers on the `PageUp`/`PageDown` arms rather than using `KeyCode::Char(_)`.

### 3. Boundary behavior

When pressing Ctrl+PageDown on the last artist header, jump to the last album in the list. When pressing Ctrl+PageUp on the first artist header (or before it), jump to the first item. This matches the existing `jump_power_music_group_display_cursor(to_end)` boundary behavior.

## Risks / Trade-offs

- [Risk] Ctrl+PageUp/PageDown may not be reported correctly by all terminal emulators. → Mitigation: crossterm handles this consistently across modern terminals; no extra work needed.
- [Risk] Large libraries with many artists could feel slow if plan rebuilding is expensive. → Mitigation: Plan building is already done per-frame for rendering; a one-time build on keypress is negligible.
