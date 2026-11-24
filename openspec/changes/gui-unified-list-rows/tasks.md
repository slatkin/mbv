# Tasks

Each unit may leave dependent callers or the build broken for the next unit. A unit records that state in its handoff. Tests follow `docs/invariants/14-test-ownership.md` and design D5. Every unit runs `cargo fmt` and commits only its own files (`git commit -- <paths>`).

## 1. Row parts (design D1–D4)

- [x] 1.1 Add `crates/mbv-gui/src/window/components/list_row.rs` (registered in `components.rs`): the title-tone and selection rules and the context-press handler of D4, and the constants from `library_item_row.rs`. Point `tree_guide.rs` at them. Verify: `cargo check -p mbv-gui`.
- [x] 1.2 Add `list_row/thumb.rs`: move `LibraryRowThumb`, `paint_thumb`, and its helpers, without the played dim. Verify: `cargo check -p mbv-gui`.
- [x] 1.3 Add `list_row/flat.rs`: `FlatRow` with the props and look of D2. Verify: `cargo check -p mbv-gui`.
- [x] 1.4 Add `list_row/tree.rs`: `TreeRow`, `TreeLevel`, `TreeDepth`, and `TreeTrail` with the props and look of D3. Verify: `cargo check -p mbv-gui`.
- [x] 1.5 Add `list_row/tests.rs`: move `library_item_row/tests.rs` onto `FlatRow`, and add the one `TreeRow` case table of D5. Verify: `cargo nextest run -p mbv-gui list_row`.

## 2. Flat rows onto `FlatRow`

- [x] 2.1 `library_home_list.rs`: both sections paint `FlatRow` with `play: Some(item.state)`. Delete `row_progress`, `recent_played_mark`, and the Recently added mark test. Verify: `cargo nextest run -p mbv-gui library_home_list`.
- [x] 2.2 `library/lettered/adapters.rs`: Podcasts and Feeds paint `FlatRow` with `play: Some(entry.state)` and replace `paint_item_row`. Emby and Audiobooks paint `play: None`. Verify: `cargo nextest run -p mbv-gui lettered podcasts feeds emby audiobooks`.
- [x] 2.3 `library_shows_list.rs`: series, New, and Upcoming rows paint `FlatRow` with `play: None`. Verify: `cargo check -p mbv-gui`.
- [x] 2.4 `library_list.rs`: artist rows paint `FlatRow` with `play: None`. Verify: `cargo check -p mbv-gui`.

## 3. Tree rows onto `TreeRow`

- [x] 3.1 `library_list.rs`: albums paint `TreeRow` (Parent, One) and tracks paint `TreeRow` (Leaf, Two). Delete their style code. Verify: `cargo nextest run -p mbv-gui library_list`.
- [x] 3.2 `library_shows_list.rs`: seasons paint `TreeRow` (Parent, One) and episodes paint `TreeRow` (Leaf, Two, `play` from `episode_state`). Delete `row_name_color`, `row_background`, `row_inset`, and `episode_played`. Update the Shows tests for the `data-library-shows-*` part hooks and the Music look. Verify: `cargo nextest run -p mbv-gui library_shows_list`.
- [x] 3.3 `playlists_panel.rs`: playlist rows paint `FlatRow` (`attr: "playlists"`, `thumb: None`, the count as trailing text) and item rows paint `TreeRow` (Leaf, One). Delete `row_style`, `item_row_style`, `row_fill`, `PLAYLIST_ROW_HEIGHT_PX`, and the Playlists guide constants; the rename field's guide follows `TOP_GUIDE_X`. `open_menu` becomes the row's `on_context`. Update the Playlists tests for the `data-library-playlists-*` hooks and the Music look. Verify: `cargo nextest run -p mbv-gui playlists_panel`.

## 4. Remove the bespoke paths

- [ ] 4.1 Delete `library_item_row.rs` and its directory, `RowPlayedMark`, `name_color`, `paint_item_row`, `ItemRowPaint`, and `mark` once no caller remains. Verify: `cargo clippy -p mbv-gui --all-targets -- -D warnings` passes (a surviving caller fails to compile, and a leftover helper fails as dead code).

## 5. Integration

- [ ] 5.1 In the foreground: `cargo build -p mbv-tui -p mbv-gui`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-client-core -p mbv-gui`, and `cargo fmt --all -- --check`. Fix every failure. Verify: all pass.
- [ ] 5.2 Give the change to the user for a live look check of every list.
