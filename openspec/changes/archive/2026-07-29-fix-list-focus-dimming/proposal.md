## Why

The music library list (`album.rs`) doesn't dim its text when the right panel loses focus, unlike every other list renderer. This is a visible bug: album titles stay bright white, year labels stay aqua, and separators stay yellow while the rest of the UI correctly dims to grey. A secondary inconsistency exists in `home_video.rs` which uses `TEXT` (near-white) instead of `SUBTLE` (dimmed grey) for unfocused items. Both issues stem from ad-hoc inline styling that bypasses the focus-aware pattern used by the rest of the codebase.

## What Changes

- **Fix music library focus dimming** (`album.rs`): Album titles, year labels, and " • " separators will dim to `SUBTLE`/`MUTED` when the panel is unfocused, matching the behavior of `list_plain.rs`, `list_letter_groups.rs`, and `home.rs`.
- **Fix home video focus dimming** (`home_video.rs`): Unfocused item titles will use `SUBTLE` instead of `TEXT`, consistent with all other list renderers.
- **Extract focus-aware color helpers** (`list_rows.rs`): Add small utility functions (`focused_or_subtle`, `focused_or_muted`, `focused_aqua_or_muted`) that centralize the focused/unfocused color selection pattern currently duplicated across 4+ files.
- **Decompose `render_power_grouped_album_rows`** (`album.rs`): Break the 600-line monolith into focused helper functions (artist header row, album title row, year/separator spans, action hints) to improve readability and reduce the risk of future styling inconsistencies.

## Capabilities

### New Capabilities
- `focus-aware-list-styling`: Shared color utility functions for consistent focus-dimming behavior across all list renderers.

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **Files modified**: `src/app/render/album.rs`, `src/app/render/home_video.rs`, `src/app/render/list_rows.rs`
- **No API changes**: All modifications are internal to the rendering layer; no public interfaces change.
- **No new dependencies**: Uses existing `palette` constants.
- **Risk**: Low. The color changes are purely visual. The `album.rs` decomposition is a pure refactor with no behavioral changes beyond the focus-dimming fix itself. Existing rendering tests (if any) and manual visual verification cover the changes.
