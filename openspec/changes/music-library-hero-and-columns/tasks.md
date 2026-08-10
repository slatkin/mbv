## 1. Responsive Music Routing

- [ ] 1.1 Derive grouped Music wide mode from the padded content width using the existing shared 82-column breakpoint.
- [ ] 1.2 Preserve the current narrow full-width pills, hero-above-list, and one-column grouped album path without behavioral changes.
- [ ] 1.3 Add a wide grouped Music coordinator that owns the 40/60 horizontal split and clears/rebuilds layout geometry when crossing the breakpoint.

## 2. Wide Left Album And Track Workspace

- [ ] 2.1 Render the selected album title, metadata, and large Home-style centered artwork in the upper left region using existing album selection and image-cache paths.
- [ ] 2.2 Add content-aware vertical allocation that gives the hero roughly three-fifths of the pane while reserving a persistent, useful track viewport and shrinking artwork first on short terminals.
- [ ] 2.3 Render cached tracks, Loading, and empty states in the lower left region even when `album_track_focus` is `None`, without showing stale album data after selection changes.
- [ ] 2.4 Keep the focused track visible through internal track-table scrolling while preview mode starts from the beginning.

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

- [ ] 6.1 Update existing grouped Music render and input tests for narrow preservation, wide one-column album geometry, reciprocal focus, and responsive identity continuity; avoid pixel-specific snapshots.
- [ ] 6.2 Extend existing mouse coverage for wrapped track hit targets and click/double-click behavior only where the current fixture can express it without a new harness.
- [ ] 6.3 Run formatting checks and the narrowest relevant application tests for grouped Music rendering, navigation, track focus, scope, and mouse dispatch.
- [ ] 6.4 Run `cargo clippy --workspace --all-targets` and `make check-code-file-lines`.
- [ ] 6.5 Visually verify a grouped Music library below 82 columns, at 82 columns, at a wider width, and at short terminal heights; check large artwork, persistent tracks, right-rail pills, pane focus colors, scrolling, resize continuity, and track mouse behavior.
