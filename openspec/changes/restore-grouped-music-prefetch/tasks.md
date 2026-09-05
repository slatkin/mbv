## 1. Component read-back

- [x] 1.1 Expose the painted album cursor and display order from `MusicWorkspaceComponent` via a read-only accessor, and verify the existing music workspace tests pass (`cargo nextest run -p mbv -- music_workspace`)
- [x] 1.2 Rewire `render_music_workspace_component` to prewarm from the post-`view` painted cursor/order for both breakpoints with the search-active skip, and verify the existing narrow prefetch test still passes

## 2. Prefetch coverage tests

- [ ] 2.1 Add a wide-breakpoint test mirroring `narrow_grouped_music_prewarms_neighbour_album_images` (wide area geometry, neighbours of the painted cursor in `card_image_loading`, selected album excluded) and verify it passes
- [ ] 2.2 Add a stale-order regression test (painted order diverging from a freshly rebuilt context warms the painted neighbourhood) and verify it passes

## 3. Gates

- [ ] 3.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and the music workspace test set, and verify all are clean
