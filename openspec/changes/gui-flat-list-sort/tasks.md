# Tasks

Add no new tests in this change (standing user rule). Where a task removes or renames behavior
that an existing test asserts, port that test to the new behavior or delete it when another test
already owns the contract.

## 1. Data at the edges

- [x] 1.1 In `crates/mbv-config/src/gui_launch_state.rs`, add `GuiLaunchSort` (snake_case: `title`, `author`, `newest`, `oldest`), `GuiLaunchListSort { destination: TabIdentity, sort: GuiLaunchSort }`, and `#[serde(default)] sorts: Vec<GuiLaunchListSort>` on `GuiLaunchState` (design D10). Give `GuiDestinationCollections.order` `skip_serializing_if = "Option::is_none"`. Set `sorts: Vec::new()` in `first_launch`. Keep the version at 1. Verify with `cargo nextest run -p mbv-config`.
- [x] 1.2 In `crates/mbv-audiobookshelf/src/catalog_books.rs`, parse the item DTO's `addedAt` into `AudiobookshelfBook.added_at_ms: Option<u64>`, beside `updated_at_ms`. A missing value or 0 is `None` (design D9). Fix every constructor of `AudiobookshelfBook` in the workspace. Verify with `cargo check --workspace --all-targets`.

## 2. Core sort type, store, and command

- [x] 2.1 Add `src/app/gui_library/list_sort.rs` with `GuiListSort` (`label`, `date_order`, `is_name_sort`), `GuiSortTarget` (`Library`, `Audiobooks`, `Podcasts`, `Feeds`, `ShowsNew`), the choices and default for each target from design D2, and `GuiListSortFrame { target, active, choices }`. Export these types where `GuiDateOrder` is exported today. Verify with `cargo check -p mbv`.
- [x] 2.2 Add the store `App.gui_list_sorts: Vec<(TabIdentity, GuiListSort)>`, `App::gui_list_sort(target)` (the stored sort when the target offers it, else the default), and `App::gui_list_sort_frame(target)` (design D2, D3). Resolve a target to its identity through `gui_destination_identity`. `ShowsNew` uses the Shows library identity. Verify with `cargo check -p mbv`.
- [x] 2.3 Replace `GuiLibraryCommand::DateOrder` with `ListSort { target, sort }` in `src/app/gui_library.rs`. The arm sets the store, and it ignores (through `log_ignored_library_command`) a target that names no presented flat list, a `ShowsNew` on a library that is not a TV library, and a sort outside the target's choices (design D5). Use no wildcard arm. Verify with `cargo check -p mbv`.
- [x] 2.4 Remove the date order from the collection path (design D4): `GuiCollectionSelection.order`, `CollectionOp::Order`, the order half of `CollectionOp::Restore`, the sites' `has_order`/`stored_order`/`install_order`, `gui_podcast_date_order`, `gui_feeds_date_order`, `gui_emby_feed_order`, and `GuiCollections.order` with its `new` parameter. Port or delete the existing tests that assert the date order in `src/app/gui_library/collection/tests.rs`, `src/app/tests/gui_library_feeds.rs`, `src/app/tests/gui_library_podcasts.rs`, and `src/app/tests/gui_library_emby.rs`. Verify with `cargo nextest run -p mbv`.
- [x] 2.5 Add the *List sort* term to `CONTEXT.md`, and update its *Letter rail* entry (design D11). Verify that both entries read as the design states.

## 3. Projections sort by the active sort

- [x] 3.1 Plain Emby and Emby feed-group projections (`src/app/gui_library/emby_projection.rs`, `emby.rs`): sort per design D6. Set `GuiEmbyRow.sort_letter` only under a name sort, and carry `gui_list_sort_frame(GuiSortTarget::Library(lib_idx))` on the projection. Verify with `cargo nextest run -p mbv`.
- [x] 3.2 Audiobooks projection (`src/app/gui_library/audiobooks_projection.rs`): sort per design D6 for Title, Author, Newest, and Oldest, using `added_at_ms` for the dates. Make `GuiAudiobooksRow.sort_letter` an `Option<char>` that is set only under a name sort, from the compared key. Carry the sort frame. Verify with `cargo nextest run -p mbv`.
- [x] 3.3 Podcasts and Feeds projections (`podcasts_projection.rs`, `feeds_projection.rs`, `entry_list.rs`): sort per design D6. Add `GuiEntryRow.sort_letter: Option<char>`, set only under Title. Carry the sort frame on the entry-list projection. Verify with `cargo nextest run -p mbv`.
- [x] 3.4 Shows New (`shows_projection.rs`, `gui_shows_new_rows`): sort per design D6 before the de-duplication. Add `GuiShowsRow.sort_letter: Option<char>`, set only under Title on New rows and `None` on Upcoming rows. Carry the sort frame on `GuiShowsProjection` only while the view is New. Verify with `cargo nextest run -p mbv`.

## 4. Launch persistence

- [x] 4.1 In `src/app/gui_library/launch.rs`, save the store into `GuiLaunchState.sorts` in store order. Stop writing a collection entry's `order`. Restore each saved sort whose identity the session has not set, then migrate each collection entry's `order: Some(_)` whose destination has no saved sort to `Newest` or `Oldest` (design D10). Port or delete the existing order assertions in `src/app/tests/gui_library/launch.rs` and `crates/mbv-gui/tests/core_thread/launch.rs`. Verify with `cargo nextest run -p mbv -p mbv-gui`.

## 5. Wire and window

- [ ] 5.1 In `crates/mbv-gui/src/bridge/library.rs`, replace the `DateOrder` wire command with `ListSort { target, sort }` and its conversion. In `crates/mbv-gui/src/bridge/frame/library/`, carry `sort: Option<GuiListSortFrame>` on `GuiEmbyList`, `GuiAudiobooksList`, `GuiPodcastsList`, `GuiFeedsList`, and `GuiShowsList`, and carry the optional `sort_letter` on the row frames. Remove `GuiCollectionsFrame::next_order` and `order_label`. Verify with `cargo check -p mbv-gui --all-targets`.
- [ ] 5.2 Add `crates/mbv-gui/src/window/components/library_sort_control.rs`: a prop-driven segmented control over `GuiListSortFrame` that is pointer-only and registers no key (design D7). Add the `sort` prop to `LibraryListHeader`, and paint the control last on the heading line. Pass each flat list's frame `sort` from the five list components. Verify with `cargo check -p mbv-gui --all-targets`.
- [ ] 5.3 Replace `LibraryState::press_date_order` with `select_list_sort(target, sort)`. It sends the command and resets that list's scroll store to the top. The filter text stays (design D7). Verify with `cargo check -p mbv-gui --all-targets`.
- [ ] 5.4 Add the generic lettered-row builder to `crates/mbv-gui/src/window/library/list.rs`, or to a child module `list/lettered.rs` if `list.rs` would pass 800 lines (design D8). Move the Emby, Audiobooks, Podcasts, Feeds, and Shows New list builders onto it. Delete `heading_emby_rows`, the Audiobooks heading code, and the `feed_group` switch in `build_emby_rows`. Each list paints its rail when its rows carry letters. Port or delete the existing window tests that the change breaks (the list builders, `library_list_header`, `library_collection_dropdown`, `library_podcasts_list`, `library_feeds_list`, and the panel fixtures). Verify with `cargo nextest run -p mbv-gui`.
- [ ] 5.5 Update `docs/architecture/gui-client.md`: the date order passages (the dropdown section, the frame field lists, and the `dateOrder` command entries) become the list sort, its frame, the `listSort` command, and the header's sort control. Verify with `rg -n -i "dateOrder|date order" docs/architecture/gui-client.md`, which must show no stale reference.

## 6. Close out

- [ ] 6.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify that all three pass.
- [ ] 6.2 Run `make check-code-file-lines`, and split any file over 800 lines along its responsibility seam. Commit. Verify that `git status` shows a clean worktree.

## Workflow follow-up

- Sync the delta into `openspec/specs/gui-client/spec.md` and archive the change only when the PR is approved for merge, as the last commit before the merge.
