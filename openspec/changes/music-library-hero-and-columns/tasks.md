## 1. Responsive Music Routing

- [ ] 1.1 Derive grouped Music wide mode from the padded content width using the existing shared 82-column breakpoint.
- [ ] 1.2 Preserve the current narrow full-width pills, hero-above-list, and one-column grouped album path without behavioral changes.
- [ ] 1.3 Add a wide grouped Music coordinator that owns the 40/60 horizontal split and clears/rebuilds layout geometry when crossing the breakpoint.

## 2. Wide Left Album And Track Workspace

- [x] 2.1 Add the album branch to the hero content painting in `list.rs:515-553` — call `render_power_album_detail` with the hero's content rect when `selected_album_item` is present
- [x] 2.2 Render album art in the hero via the existing `inline_album_art` path, positioned within the hero rect
- [x] 2.3 Verify track focus interaction works in the hero context: Enter activates track cursor, track navigation moves within the hero's track list, Escape exits track focus

## 3. Wide Right Album Browser

- [ ] 3.1 Move music-group pills into the top of the wide right rail while retaining existing pill selection, overflow, and group-switch behavior.
- [ ] 3.2 Render the settled artist-grouped album browser below the pills with one album per row and full-width artist labels, relying on the prerequisite change for non-selectability.
- [ ] 3.3 Remove Music-only grouped two-column packing and left/right album-cell navigation; retain any generic column machinery that structural search shows is still used elsewhere.
- [ ] 3.4 Preserve album cursor identity, paging, wrapping, loading/organizing messages, and scroll clamping in the narrower right rail.

## 4. Focus And Styling

- [ ] 4.1 Derive internal pane focus from outer `PanelFocus` and `album_track_focus` without adding persisted focus state.
- [ ] 4.2 Apply Home's focused green, playback-panel, selected-row, aqua-marker, yellow-title, and unfocused text treatments reciprocally to the left workspace and right browser.
- [ ] 4.3 Preserve Enter, track movement/playback, current-item scope, Escape/Backspace, album selection, and group-switch semantics while keeping wide geometry fixed during focus changes.
- [ ] 4.4 Preserve selected album and focused track identity when resizing across the responsive breakpoint.

## 5. Wide Track Mouse Interaction

- [ ] 5.1 Record per-track wide-mode hit targets that cover every visible wrapped row and clear them when the wide Music layout is not active.
- [ ] 5.2 Make single-click select the logical track and shift focus left; make double-click select and play that track through the existing playback path.
- [ ] 5.3 Ensure album and group-pill clicks clear track focus and return focus right, while artwork and blank hero space do not activate tracks or playback.

## 6. Tests And Verification

- [x] 6.1 In `power_widgets.rs:569-570`, remove the `is_album_folders && is_music_group` branch so it falls through to `render_power_list`
- [x] 6.2 Move the loading/organizing state messages from `music.rs:97-146` into `render_power_list`'s empty-items handling, gated on `is_music_group_view`
- [x] 6.3 Delete `render_power_music_group_view` from `music.rs`; keep `render_power_music_group_pills_row`
- [x] 6.4 Verify the music-group pills in `mod.rs:511-528` still render correctly above the hero — the pills carve rows from `lib_area` before `render_power_list` receives it, so no change expected

## 7. Tests and verification

- [x] 7.1 Update existing album plan tests (`tests_album_focus.rs`, `tests_album_listing.rs`) with `hero_handles_detail: true` variants
- [x] 7.2 Update music-group rendering tests (`tests_music_groups.rs`) for the new hero + two-column layout
- [x] 7.3 Add test: album hero sizing matches the expanded block height computed by the standalone sizing function
- [x] 7.4 Add test: two-column grouped album rows pack correctly with full-width artist headers and independent group packing
- [ ] 7.5 Visual verification in a real terminal at multiple widths (narrow 1-col, threshold, wide 2-col) with a music library with levels
- [ ] 7.6 Verify track focus, artist header selection, and group pill switching all work in the hero context
- [x] 7.7 Run `cargo test -p mbv-core` and `cargo clippy --workspace --all-targets`
