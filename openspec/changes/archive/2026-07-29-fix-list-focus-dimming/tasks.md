## 1. Extract Focus-Aware Color Utilities

- [x] 1.1 Add `focused_or_subtle(focused: bool) -> Color` to `list_rows.rs` — returns `palette::WHITE` when focused, `palette::SUBTLE` when not
- [x] 1.2 Add `focused_or_muted(focused: bool) -> Color` to `list_rows.rs` — returns `palette::YELLOW` when focused, `palette::MUTED` when not
- [x] 1.3 Add `focused_aqua_or_muted(focused: bool) -> Color` to `list_rows.rs` — returns `palette::AQUA` when focused, `palette::MUTED` when not
- [x] 1.4 Adopt the new utilities in `list_plain.rs` (replace inline `if focused { WHITE } else { SUBTLE }` at line ~190)
- [x] 1.5 Adopt the new utilities in `list_letter_groups.rs` (replace inline `if focused { WHITE } else { SUBTLE }` at line ~259)
- [x] 1.6 Adopt the new utilities in `home.rs` (replace inline `if focused { WHITE } else { SUBTLE }` at line ~258)

## 2. Fix Home Video Focus Dimming

- [x] 2.1 In `home_video.rs` `render_home_video_item` (line ~116), replace the `palette::TEXT` fallback with `focused_or_subtle(focused)`

## 3. Decompose album.rs Into Helper Functions

- [x] 3.1 Extract `GroupedAlbumDisplayRow::ArtistHeader` match arm into `render_artist_header_row` function
- [x] 3.2 Extract `GroupedAlbumDisplayRow::Album` match arm into `render_album_row` function
- [x] 3.3 Extract `GroupedAlbumDisplayRow::AlbumActionHint` match arm into `render_album_action_hint` function
- [x] 3.4 Extract `GroupedAlbumDisplayRow::ArtistActionHint` match arm into `render_artist_action_hint` function
- [x] 3.5 Verify the decomposition compiles without errors and passes visual smoke test

## 4. Apply Focus Dimming to album.rs

- [x] 4.1 In `render_album_row`: fix the non-grouped album title path — change the condition from `selected && focused` to just `selected` for the FOAM+BOLD branch, and replace the hardcoded `WHITE` else-branch with `focused_or_subtle(focused)` (must match the grouped-block path pattern)
- [x] 4.2 In `render_album_row`: apply `focused_or_muted` to " • " separator color (currently hardcoded `YELLOW` at lines ~269, ~380)
- [x] 4.3 In `render_album_row`: apply `focused_aqua_or_muted` to year label color (currently hardcoded `AQUA` at lines ~273, ~383)
- [x] 4.4 Verify artist header label dimming already works correctly (lines ~194-202 already use `SUBTLE` when unfocused) — no change expected, confirm only

## 5. Verification

- [x] 5.1 Run `cargo build` to confirm zero compilation errors
- [x] 5.2 Run `cargo clippy` to confirm no new warnings
- [ ] 5.3 Visual check: music library panel — titles, years, and separators dim when focus moves away
- [ ] 5.4 Visual check: home video panel — item titles dim when focus moves away
- [ ] 5.5 Visual check: all other list panels (home, plain, letter-grouped) — no regression in existing focus dimming behavior
- [ ] 5.6 Visual check: selected items in all panels retain their highlight colors regardless of focus state
- [ ] 5.7 Visual check: non-grouped album list (plain folder browsing, no artist headers) — titles, years, and separators dim when focus moves away
- [ ] 5.8 Visual check: selected-but-unfocused items in non-grouped album list retain FOAM + BOLD styling
- [ ] 5.9 Visual check: inline album art rendering is unaffected by focus-dimming changes
- [ ] 5.10 Visual check: track detail (expanded) row rendering is unaffected by focus-dimming changes
