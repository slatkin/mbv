## 1. Album hero sizing and helper

- [x] 1.1 Add `power_selected_album_item` helper in `detail.rs` — returns the selected album `MediaItem` when `collection_type == "music"` and `is_viewing_album_folders`, parallel to `power_selected_movie_item`
- [x] 1.2 Extract album hero height computation from `album_plan.rs` inline expansion sizing into a standalone function: given track count, art dimensions, and panel width, return the total hero rows needed (art + metadata + tracks + `HERO_BLOCK_EXTRA_ROWS`)
- [x] 1.3 Add the album branch to the hero height computation in `list.rs:191-243` — after `selected_movie_item` and `selected_series_item`, check `selected_album_item` and call the new sizing function
- [x] 1.4 Add `"music"` to the `top_hero_level` placeholder match in `list.rs:232-234` so the hero placeholder renders while the album list is loading

## 2. Album hero content painting

- [x] 2.1 Add the album branch to the hero content painting in `list.rs:515-553` — call `render_power_album_detail` with the hero's content rect when `selected_album_item` is present
- [x] 2.2 Render album art in the hero via the existing `inline_album_art` path, positioned within the hero rect
- [ ] 2.3 Verify track focus interaction works in the hero context: Enter activates track cursor, track navigation moves within the hero's track list, Escape exits track focus

## 3. Album plan: suppress inline expansion

- [x] 3.1 Add `hero_handles_detail: bool` parameter to `build_grouped_album_display_plan` in `album_plan.rs`
- [x] 3.2 When `hero_handles_detail` is true, suppress `AlbumDetailStart`, `AlbumDetailContinuation`, `AlbumDetailRule`, `AlbumLoading`, and `AlbumActionHint` rows from the plan output
- [x] 3.3 Set `selected_block_bounds` and `track_detail_bounds` to `None` when `hero_handles_detail` is true
- [x] 3.4 Update callers of `build_grouped_album_display_plan` to pass `hero_handles_detail: true` when rendering through `render_power_list`, `false` for any remaining non-hero callers

## 4. Two-column packing for grouped album rows

- [x] 4.1 Add `cols` parameter to `render_power_grouped_album_rows` in `album.rs`
- [ ] 4.2 Batch consecutive `Album` rows into column pairs at render time — album `i` within an artist group occupies column `i % cols`, each pair shares a terminal row
- [ ] 4.3 Render `ArtistHeader` and `ArtistGroupSpacer` rows at full width, starting a fresh row (same pattern as letter headers in `list_letter_groups.rs`)
- [ ] 4.4 Each artist group packs independently — a row never mixes albums from two groups; a trailing odd album in a group leaves the partner cell empty
- [x] 4.5 Pass the column count from `render_power_list` through to `render_power_grouped_album_rows` via the `cols` variable already computed in `list.rs:170-174`

## 5. Cursor movement with columns in grouped view

- [ ] 5.1 Update `album_cursor.rs` cursor movement to accept `cols` — up/down moves by `cols` items within a group, left/right moves by 1 item
- [ ] 5.2 Handle group boundaries: down from the last row of a group moves to the first album of the next group; up from the first row moves to the last row of the previous group
- [ ] 5.3 Update `page_power_grouped_album_cursor` to page by `cols × page_rows` items
- [ ] 5.4 Update key handling in `input_lib_power_keys.rs` to pass the column count to grouped-view cursor movement

## 6. Dispatch change and music.rs cleanup

- [ ] 6.1 In `power_widgets.rs:569-570`, remove the `is_album_folders && is_music_group` branch so it falls through to `render_power_list`
- [ ] 6.2 Move the loading/organizing state messages from `music.rs:97-146` into `render_power_list`'s empty-items handling, gated on `is_music_group_view`
- [ ] 6.3 Delete `render_power_music_group_view` from `music.rs`; keep `render_power_music_group_pills_row`
- [ ] 6.4 Verify the music-group pills in `mod.rs:511-528` still render correctly above the hero — the pills carve rows from `lib_area` before `render_power_list` receives it, so no change expected

## 7. Tests and verification

- [ ] 7.1 Update existing album plan tests (`tests_album_focus.rs`, `tests_album_listing.rs`) with `hero_handles_detail: true` variants
- [ ] 7.2 Update music-group rendering tests (`tests_music_groups.rs`) for the new hero + two-column layout
- [ ] 7.3 Add test: album hero sizing matches the expanded block height computed by the standalone sizing function
- [ ] 7.4 Add test: two-column grouped album rows pack correctly with full-width artist headers and independent group packing
- [ ] 7.5 Visual verification in a real terminal at multiple widths (narrow 1-col, threshold, wide 2-col) with a music library with levels
- [ ] 7.6 Verify track focus, artist header selection, and group pill switching all work in the hero context
- [ ] 7.7 Run `cargo test -p mbv-core` and `cargo clippy --workspace --all-targets`
