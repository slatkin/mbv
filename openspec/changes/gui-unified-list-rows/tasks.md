# Tasks

Each unit may leave dependent callers or the build broken for the next unit. A unit records that state in its handoff. Tests follow `docs/invariants/14-test-ownership.md`: add a test only for a contract no other test owns; delete the tests of removed behaviour with that behaviour. Every unit runs `cargo fmt` and stages only its own files (`git commit -- <paths>`).

## 1. Row primitives (design D1, D2, D3)

- [ ] 1.1 Add `crates/mbv-gui/src/window/components/list_row.rs` (registered in `components.rs`): `RowState { play: Option<GuiHomeRowStateFrame>, selected: bool }` and the private look derivation of design D3 (title tone and weight, subtitle tone, trailing tone, thumb dim, mark kind), with the D3 constants. Move the Recently added constants and `RowPlayedMark` logic of `library_item_row.rs` here. Verify: `cargo check -p mbv-gui` passes.
- [ ] 1.2 Add `list_row/mark.rs`: the 12 px mark slot with the 8 px `Green` circle (unplayed, partly played), the 12 px Lucide `Check` in `Grey1` (played), or an empty slot (not tracked); `role="img"`, `aria-label`, and the `data-row-played-mark="Unplayed|Played"` hook. Verify: `cargo check -p mbv-gui` passes.
- [ ] 1.3 Add `list_row/flat.rs`: `FlatRow`, ported from `LibraryItemRow` and its `paint_thumb`, with the props of design D1 and `state: RowState` in place of `title_color`, `background`, `selected`, `progress`, and `played_mark`. The thumb dims at 0.35 on played and paints the progress run on partly played. The mark slot sits before the trailing text. Verify: `cargo check -p mbv-gui` passes.
- [ ] 1.4 Add `list_row/tree.rs`: `TreeRow` with `attr`, `kind`, `id`, `height`, `inset`, `number: Option<String>`, `title`, `trail: Option<String>`, `state: RowState`, `tree_open: Option<bool>`, `tail: bool`, `guides: Element`, `hooks: Vec<Attribute>`, `on_press`, `on_play`, `on_toggle: Option<Callback<()>>`, `on_context: Option<Callback<(f32, f32)>>`. It paints the D3 look, the `library-raise` or `tree-subtree` fill, `tail_divider_style(tail)`, the mark slot before the trailing text, the bottom-edge progress run on partly played, and `TreeHint` on a parent. Verify: `cargo check -p mbv-gui` passes.
- [ ] 1.5 Add `list_row/tests.rs`: one `#[case]` table over (`FlatRow`, `TreeRow`) x (not tracked, unplayed, partly played, played) asserting the mark hook and label, the thumb dim on played (flat), the progress run on partly played, and an equal trailing-text x position across the four states. Verify: `cargo nextest run -p mbv-gui list_row` passes.

## 2. Core play state for every list (design D4)

- [ ] 2.1 `src/app/gui_library/emby_projection.rs`: add `state: Option<MediaSemanticState>` to `GuiEmbyRow`, filled with `MediaSemanticState::from_emby(item)`, `None` for a folder, box set, or `item.is_music()`. Mirror it in `GuiEmbyRowFrame` (`crates/mbv-gui/src/bridge/frame/library/emby.rs`) through the existing `GuiHomeRowStateFrame` mapping. Fix the fixtures `cargo check -p mbv -p mbv-gui --all-targets` names. Verify: that command passes.
- [ ] 2.2 `src/app/gui_library/shows_projection.rs`: add the same field to `GuiShowsRow` (`from_emby` for a New episode, `None` for an Upcoming premiere) and `GuiShowsRowFrame`. Verify: `cargo check -p mbv -p mbv-gui --all-targets` passes.
- [ ] 2.3 `src/app/gui_library/audiobooks_projection.rs`: add the field to `GuiAudiobooksRow`, filled in `book_row` with `MediaSemanticState::from_audiobookshelf` from the library's stored book progress for `book.library_item_id` (the map `apply_audiobookshelf_catalog_progress` fills), `Ordinary` when none. Mirror it in `GuiAudiobooksRowFrame`. Add one unit test that a finished book projects `Played`. Verify: `cargo nextest run -p mbv audiobooks_projection` passes.
- [ ] 2.4 `src/app/gui_playlists.rs`: add the field to `GuiPlaylistItemProjection` (`from_emby(item)`, `None` for a music item) and `GuiPlaylistItem` (`crates/mbv-gui/src/bridge/frame/playlists.rs`). Add `state: None` to `GuiMusicTrackFrame`'s painter input (music is not tracked; no core field). Verify: `cargo check -p mbv -p mbv-gui --all-targets` passes.

## 3. Recently added follows the live state (design D5)

- [ ] 3.1 Add `App::apply_home_recent_played(content, played, position_ticks)` to `src/app/state/app_struct/home_recently_added.rs`, updating every stored item with that content id. Call it from `apply_stopped_slot_progress` (`dispatch/session/player_event.rs`), `context_set_played` and `set_feed_entries_played` (`dispatch/context_menu/actions.rs`), `apply_audiobookshelf_socket_progress` (`dispatch/audiobookshelf/service.rs`), and `apply_audiobookshelf_catalog_progress` (`dispatch/run_loop/drains.rs`). Add one unit test: a stopped-slot progress that marks an item played makes its Recently added row project `Played`. Verify: `cargo nextest run -p mbv home_recent` passes.

## 4. Flat lists onto `FlatRow`

- [ ] 4.1 `library_home_list.rs`: paint every item row through `FlatRow` with `RowState { play: Some(item.state), selected }`. Delete `recent_played_mark` and the `name_color` and `row_progress` uses. Extend `a_recent_row_paints_its_played_mark_and_a_continue_row_none` into one test that a played and an unplayed row of either section paint their marks. Verify: `cargo nextest run -p mbv-gui library_home_list` passes.
- [ ] 4.2 `library/lettered/adapters.rs`: Podcasts and Feeds through `FlatRow` (replace `paint_item_row`), Emby and Audiobooks through `FlatRow` with the new frame state. Extend one existing test in each list's test file with one played and one unplayed row asserting their mark hooks. Verify: `cargo nextest run -p mbv-gui lettered podcasts feeds emby audiobooks` passes.
- [ ] 4.3 `library_shows_list.rs`: the New and Upcoming flat rows and the series rows through `FlatRow` (series with `play: None`, `tree_open`, `tree_guide`). Verify: `cargo check -p mbv-gui` passes.
- [ ] 4.4 `library_list.rs`: the Music artist rows through `FlatRow` with `play: None`. Verify: `cargo check -p mbv-gui` passes.

## 5. Tree rows onto `TreeRow`

- [ ] 5.1 `library_shows_list.rs`: `paint_shows_season` and `paint_shows_episode` through `TreeRow` (season `play: None`; episode `play: Some` from the carried episode state). Delete `row_name_color` and `episode_played`; read the episode state once where the row model is built. Extend one existing Shows test with a played and an unplayed episode asserting their mark hooks. Verify: `cargo nextest run -p mbv-gui library_shows_list` passes.
- [ ] 5.2 `library_list.rs`: `paint_album_row` and `paint_track_row` through `TreeRow` (album `play: None`, year as trailing text; track `play: None`, number cell, duration as trailing text). Verify: `cargo nextest run -p mbv-gui library_list` passes.
- [ ] 5.3 `playlists_panel.rs`: `playlist_node` and `items_block` rows through `TreeRow` (playlist `play: None`; item `play` from the frame). Delete `row_style`. Extend one existing Playlists test with a played and an unplayed item asserting their mark hooks. Verify: `cargo nextest run -p mbv-gui playlists_panel` passes.

## 6. Remove the bespoke paths

- [ ] 6.1 Delete `crates/mbv-gui/src/window/components/library_item_row.rs` and its tests, `name_color`, `paint_item_row`, `ItemRowPaint`, and `mark` once no caller remains; move any test that still owns a contract to `list_row/tests.rs` or the list's own tests. Verify: `rg -n "LibraryItemRow|title_color|played_mark|name_color|row_name_color" crates/mbv-gui/src` finds nothing, and `cargo build -p mbv -p mbv-gui` passes.

## 7. Integration

- [ ] 7.1 Run `cargo build -p mbv -p mbv-gui` (the `make dev` build, without the test feature), `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv -p mbv-gui`, all in the foreground. Fix every failure. Verify: all three pass.
