## 1. Lock Planner Behavior With Tests

- [ ] 1.1 Add failing display-plan tests for one artist-scoped block when either its header or an album is targeted, including one artist row, one hint row, sibling albums, and shared block bounds.
- [ ] 1.2 Add failing album-window tests for fitting and overflowing discographies, whole-entry shifts for wrapped titles, range metadata, and first-eight-line clipping for a single oversized album.
- [ ] 1.3 Add failing navigation tests proving hidden albums remain selectable, the marker crosses artist boundaries, and album actions still resolve to the focused album.
- [ ] 1.4 Add failing render tests for fixed marker alignment, target-specific hints, range feedback, artist-versus-album artwork, retained album marker/cover during track focus, artwork anchoring and filler, and full-width wrapping below artwork.
- [ ] 1.5 Add failing integration tests for loaded and loading track expansion, sibling preservation, content-driven block growth, stable outer offset with cursor-follow fallback and no sticky header, and mouse targets in a shifted album window.

## 2. Restructure Grouped Album Planning

- [ ] 2.1 Refactor grouped album planning to identify artist ranges and emit one selected artist-group frame for header, collapsed album, loading album, and expanded album targets.
- [ ] 2.2 Implement the stateless eight-terminal-row trailing album window and expose its visible album range for hint rendering.
- [ ] 2.3 Keep the complete artist-header-plus-album selectable sequence independent of the selected group's visible display rows.
- [ ] 2.4 Append loading or track-detail rows below the album window inside the shared block and preserve visible sibling albums.

## 3. Render Marker, Hint, And Artwork

- [ ] 3.1 Render the fixed two-column target gutter, AQUA marker, bold white focused title, and artist-versus-album action hints without shifting unfocused album text.
- [ ] 3.2 Render `first-last/total` in the pinned hint row only when the selected artist's albums overflow the visible window.
- [ ] 3.3 Replace constant block wrap widths with top-down row-aware measurement so only rows overlapping the 12-row artwork zone are narrowed.
- [ ] 3.4 Switch between artist collage and album cover from the active marker, and retain enough filler rows for the complete artwork box.

## 4. Integrate Navigation And Outer Scrolling

- [ ] 4.1 Make artist-header focus park the marker on the shared artist row and remove the separate selected-header block construction.
- [ ] 4.2 Preserve the outer viewport offset while targets remain visible within one artist block, with cursor-follow fallback for short viewports and expanded track focus.
- [ ] 4.3 Update mouse row targets and display-cursor calculations for the shared block and derived album window.

## 5. Remove Duplication And Verify Compatibility

- [ ] 5.1 Add failing regression tests that plain/search album selections retain per-album framing without a duplicated artist-name row.
- [ ] 5.2 Remove `GroupedAlbumDisplayRow::AlbumArtist`, its producers and renderer, and simplify artwork offsets that included selected artist-line height.
- [ ] 5.3 Run targeted album planner, rendering, navigation, and action tests; then run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and the relevant workspace test suite.
