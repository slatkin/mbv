# Tasks

## 1. Core selection type

- [x] 1.1 Add `src/app/gui_library/collection.rs` with `GuiCollectionSet { All, Checked(Vec<String>) }`, `GuiDateOrder { Newest, Oldest }`, and `GuiCollectionTarget { Music(usize), Podcasts(usize), Feeds, EmbyFeed(usize) }` (design D1, D5). Implement `toggle`, `all`, `contains`, and `retain_present`, and the one date comparison over `Option<i64>` that keeps undated rows last in both orders (D4). Verify with unit tests in `collection/tests.rs` for one `#[case]` table of the spec "The selection is All or checked collections" transitions and one test for undated-last in each order.

## 2. Core destinations move to the set

- [x] 2.1 Music: change `gui_library_groups` to `HashMap<usize, GuiCollectionSet>`, replace `gui_selected_group` with `gui_selected_groups` (All returns every group), and feed `gui_music_union` from it (D2). Apply the pick rule of D11 (`gui_library_picks` written only for one checked group; a session-only union pick per library). Verify with the existing Music tests in `src/app/tests/gui_library.rs` updated, plus one test for "A second group keeps the Hero".
- [x] 2.2 Music save and restore: write the checked groups to `group_ids` (empty for All) and restore through `retain_present` (D10) in `src/app/gui_library/launch.rs`. Verify with tests for "The GUI starts again" (two groups) and "A saved collection no longer exists" (falls back to All).
- [x] 2.3 Podcasts: replace `GuiPodcastPill` with `shows: GuiCollectionSet` and `order: GuiDateOrder` in `GuiPodcastSelection`; delete the `State` arm; write `committed_show_pill` as `Some(id)` only for one checked show (D2); filter and sort the merged list in `podcasts_projection.rs` (D4). Verify with `src/app/tests/gui_library_podcasts.rs` updated for two checked shows and for Oldest.
- [x] 2.4 Feeds: replace `GuiFeedPill` with `subscriptions: GuiCollectionSet` and `order` in `GuiFeedsSelection`; filter and sort in `feeds_projection.rs` (D4). Verify with `src/app/tests/gui_library_feeds.rs` updated for two checked subscriptions and for Oldest with an undated entry last.
- [x] 2.5 Emby fetch: extract `spawn_feed_home_video_fetch(lib_idx, target)` in `src/app/dispatch/feeds/feed.rs` from the current selection fetch, and make the existing path call it with `state.selection` (D3). Verify the existing feed-group tests pass unchanged.
- [x] 2.6 Emby feed group: add `gui_emby_feed_sets`, delete `GuiEmbyFeedGroup` and the New view, make `ensure_gui_emby_selection` start one missing fetch at a time through 2.5 without writing `FeedHomeVideoState::selection`, and merge, de-duplicate, and sort by `date_added` in `emby_projection.rs` (D2, D3, D4). Verify with `src/app/tests/gui_library_emby.rs` updated for two folders (second fetch starts after the first lands) and for the TUI selection staying unchanged.
- [x] 2.7 Commands: replace `SelectCollection`, `PodcastsSetPill`, `FeedsSetPill`, the Emby set-feed-group command, and `ShowsSetMode` with `CollectionToggle`, `CollectionAll`, and `DateOrder` in `src/app/gui_library.rs` (D5), with exhaustive arms per target, a logged ignore for a stale key, and a documented no-op for `DateOrder` on Music. Verify with one test per command against one target and one stale-key test.

## 3. Shows and Home

- [x] 3.1 Shows: make the GUI Shows destination always use `TvContentMode::All` and delete `GuiShowsMode`, the Shows pill frame, and the Latest and Upcoming flat-row paths in `src/app/gui_library/shows.rs` and `shows_projection.rs` (D8). Verify with `src/app/tests/gui_library_shows.rs` updated so Shows opens on the series list.
- [x] 3.2 Home Latest episodes: add `gui_home_tv_latest`, the launch-time worker fetch per TV library through `get_latest_episodes(parent_id, 30)`, and its new `LibEvent` arm (D9). Verify with a test in `src/app/tests/` that injects the event for two libraries and reads one merged, newest-first section.
- [x] 3.3 Home Upcoming: request the eager Upcoming fetch for each TV library at GUI launch through the existing once-per-session guard, and merge `tv_upcoming_snapshots` soonest first (D9). Verify with a test that two snapshots merge and that no section shows when both are empty.
- [x] 3.4 Home rows: add `GuiHomeRowId::LatestEpisode` and `GuiHomeRowId::Upcoming` in `src/app/gui_library/home.rs`, resolve `LatestEpisode` through the Recently added Emby item path, and make the four Home commands on `Upcoming` documented no-op arms (D9). Verify with a test that `HomeSelect` on an Upcoming row keeps the Hero and that `HomePlay` on it enqueues nothing.

## 4. Bridge frame

- [x] 4.1 Replace `GuiMusicPillFrame`, `GuiPillFrame<P>`, the Emby pill frame, and the Shows pill frame with `GuiCollectionsFrame` and `GuiCollectionEntry` in `crates/mbv-gui/src/bridge/frame/library/` (D6), and the set-pill bridge commands in `bridge/library.rs` with the three D5 commands. Verify with `cargo check -p mbv-gui` and the bridge frame tests updated.

## 5. Window dropdown

- [x] 5.1 Add `LibraryCollectionDropdown` in `crates/mbv-gui/src/window/components/library_collection_dropdown.rs` (D6): the trigger with its label, the Clear control while not All, the date order toggle when the frame carries an order, and the `menu_card` popover with the filter field, the All row, and one checkbox row per entry. Verify with component tests for the trigger label (All, one name, count), Clear shown only when not All, and the toggle absent for Music.
- [x] 5.2 Add `collections_open` and `collections_filter` to `LibraryState` and handle them in the window (D7): open and close on the trigger, close and clear on an outside press, Esc, and a destination change, and give typed keys, Backspace, and Esc to the filter while open. Verify with window tests for "The filter narrows the entries", "A press outside closes the dropdown", and "The dropdown filter keeps the keys".
- [x] 5.3 Mount the dropdown in the Music, Podcasts, Feeds, and Emby feed-group destinations, remove the pill row from Shows, and delete `LibraryPillRow`, `LibraryPillButton`, `library_pills.rs`, the four destination pill components, `window/library/pill_list.rs`, and their tests (D6). Verify with `library_panel/tests` updated so each collection destination paints the dropdown and Shows paints none.
- [x] 5.4 Update the Library Status bar location to name All or the checked collections (spec "The Library Status bar shows hints and the location"). Verify with `library_status_bar/tests.rs` for two checked subscriptions.
- [x] 5.5 Paint the two new Home sections and their rows in the Home list, with Upcoming rows taking no select. Verify with a Home list component test for the section order after Recently added.

## 6. Docs

- [x] 6.1 Update `CONTEXT.md`: replace the *Collection pill* term with *Collection dropdown* (D12) and update the *Library Status bar* term. Verify with `rg -n "Collection pill" CONTEXT.md` returning only the Avoid line.
- [x] 6.2 Update `docs/architecture/gui-client.md` and `docs/architecture/interactive-surface-ledger.md` where they name the GUI pill row. Verify with `rg -n -i "pill row" docs/architecture/gui-client.md` returning no GUI pill-row description.

## 7. Integration

- [x] 7.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv -p mbv-gui`, and fix every failure.
