# Tasks

## 1. Audiobookshelf finished-state client calls

- [x] 1.1 In `crates/mbv-audiobookshelf`, add `set_finished_bounded(api_key, library_item_id, episode_id: Option<&str>, finished, hard_bound)`. It sends `PATCH /api/me/progress/{libraryItemId}` with `/{episodeId}` appended for episodes, and the body `{"isFinished": bool}`. Add a `patch_json` helper beside `post_json` in `playback.rs`, reusing `catalog::map_error` and `mbv_net::encode_path_segment` for every path segment. Add `batch_set_finished_bounded(api_key, &[ProgressFinishedUpdate], hard_bound)`, which sends `PATCH /api/me/progress/batch/update` with a JSON array of `{libraryItemId, episodeId?, isFinished}`. Leave `episodeId` out of the JSON for books, using `skip_serializing_if`. Verify with `cargo check -p mbv-audiobookshelf`.
- [x] 1.2 Add one test file to the crate's existing `tests/` binary, using the `with_test_agent` mock pattern the existing tests use. Contract: a single episode, a single book, and a batch request each send the correct method, path, and body, and a 401 maps to the authentication-rejected error class. Use one `#[case]` table for the three request shapes. Verify with `cargo nextest run -p mbv-audiobookshelf`.

## 2. Emby builders follow the standard (design D8)

- [x] 2.1 In `src/app/dispatch/context_menu/menu.rs`:
  - `push_play_state_context_action`: change the labels to "Mark Played" and "Mark Unplayed", and return without pushing when `item.is_music()`.
  - `push_leaf_context_actions`: remove the inline `media_type != "Audio" && item_type != "Audio"` guard, since `is_music()` now covers it.
  - `open_context_menu_for_selection`: move the Remove entry after Mark Unplayed.

  Keep "Play All" on folder rows. Verify with `cargo check -p mbv`.
- [x] 2.2 In `src/app/state/context_menu_capabilities.rs`, set `emby_item_capabilities`' `played_state_capable` to `!item.is_music()`. Verify with `cargo check -p mbv`.
- [x] 2.3 Contract (context-menu spec: the standard action set, the Played label, and the music rule): one App-level `#[case]` test over `build_context_menu_for`, asserting the ordered label list for:
  - an unplayed movie: Play, Add to Queue, Mark Played
  - a played movie: Mark Unplayed, with no "Watched" anywhere
  - a TV season with unplayed children: Play All, Shuffle, Add to Queue, Mark Played
  - a music album: Play All, Shuffle, Add to Queue
  - a music track: Play, Add to Queue

  Add one test that a multi-selection of a movie and a music track has no mark entries. Before adding, extend an existing menu-entry test if one exists. Find one with `rg -n "push_leaf_context_actions|build_context_menu_for" src/app/tests`. Update any existing test whose expectation this change replaces (old order, a mark entry on an album), and don't add a parallel one. Verify with `cargo nextest run -p mbv`.

## 3. Emby library lists send the items they paint (design D7)

- [ ] 3.1 In `crates/mbv-components/src/emby_library_content/input.rs` (`.`, around line 204; inline search right-click, around line 88) and `panel.rs` (right-click, around line 319), map each resolved target ID to the `EmbyItem` the component paints:
  - list rows: look up the ID in `self.items()`
  - search results: use `inline_search.item_for_target`

  Emit `ContextMenuTargets::Emby(items)` instead of `Browser(ids)`, keeping list order and dropping an ID that resolves to nothing. Verify with `cargo check -p mbv-components`.
- [ ] 3.2 Delete `ContextMenuTargets::Browser` (`crates/mbv-ui-model/src/context_menu.rs`), its arm in `handle_library_row_context_menu` (`src/app/shell/messages.rs`), and `matching_context_items` if it is now unused. The existing `Emby` arm already handles one item (`open_emby_context_item`) and several (`open_context_menu_for_selection`). Check that the `Emby` arm is right for a non-TV/Music Emby library tab. `open_emby_context_item` calls `focus_emby_context_item`, so confirm it focuses the given item without re-deriving a cursor from `nav_stack`, which a feed or Latest row is not in. Fix it in that function if it does re-derive. Verify with `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 3.3 Contract (context-menu spec, "Every painted library row resolves its own menu"): one component test in `crates/mbv-components/src/emby_library_content/tests.rs`. With content pushed in the homevideos feed-group selector mode, `.` on a video row emits `RowContextMenu(ContextMenuTargets::Emby([that video]))`. One App or shell test: a `RowContextMenu(Emby([item]))` for an item absent from `nav_stack.last().items` on an Emby library tab still opens a menu. Verify with `cargo nextest run -p mbv-components` and `cargo nextest run -p mbv`.

## 4. Feeds menu follows the standard

- [ ] 4.1 In `crates/mbv-ui-model/src/context_menu.rs`, add `ContextAction::FeedsShuffle(Vec<FeedEntry>)`. Change `is_bulk_action` so any `Feeds*` action whose vector has more than one element counts as bulk (design D2). Add an exhaustive `FeedsShuffle` arm in `execute_context_queue_and_feed_action` (`src/app/dispatch/context_menu/actions.rs`) that shuffles the entries with `rand` and calls `play_feed_entries`. Add it to the no-op list in `execute_context_navigation_action`. Verify with `cargo check --workspace`.
- [ ] 4.2 In `open_feeds_context_menu` (`menu.rs`), split on the entry count (design D3):
  - One entry: Play, Add to Queue, then "Mark Unplayed" when `entry.played`, otherwise "Mark Played".
  - More than one: Play, Shuffle, Add to Queue, Mark Played, Mark Unplayed.

  Contract (context-menu spec: the standard set and the state-chosen entry): one `#[case]` test in the existing Feeds menu tests (`src/app/tests/podcast.rs`, or wherever `open_feeds_context_menu` is tested today), asserting the ordered labels for an unplayed single entry, a played single entry, and two entries. Verify with `cargo nextest run -p mbv`.

## 5. Audiobookshelf menu: play, shuffle and enqueue

- [ ] 5.1 In `crates/mbv-ui-model/src/context_menu.rs`, add `AudiobookshelfMenuTarget { Episode { library_item_id, episode_id }, Book { library_item_id } }` (design D1) and `ContextMenuTargets::Audiobookshelf(Vec<AudiobookshelfMenuTarget>)`. Add `ContextAction::AudiobookshelfPlay`, `AudiobookshelfShuffle`, and `AudiobookshelfEnqueue`, each holding `Vec<AudiobookshelfMenuTarget>`, and include them in the `is_bulk_action` length rule. Verify with `cargo check -p mbv-ui-model`.
- [ ] 5.2 Add an id-taking book resolver beside `audiobookshelf_book_queue_item` (`crates/mbv-ui-model/src/audiobookshelf_browse/books.rs`) that builds the book `QueueItem` for a given `libraryItemId`. Rebuild the existing selected-book function on top of it, so queue-item construction lives in one place. Add an App helper that maps `&[AudiobookshelfMenuTarget]` to `Vec<QueueItem>` for the current Audiobookshelf tab. Episodes go through `selected_audiobookshelf_queue_item_target`. Skip any target that doesn't resolve (design D4). Verify with `cargo check -p mbv`.
- [ ] 5.3 In `actions.rs`, add exhaustive arms for the three new actions (design D4):
  - Play and Shuffle: check `can_admit_audiobookshelf` and show the existing toast text on failure. Shuffle randomizes the order first. Then call `submit_queue_item(first, true)` and append the rest with `submit_queue_item(_, false)`.
  - Enqueue: append the items in order, with the same bound-scope admission check as `enqueue_selected_audiobookshelf_episode_target`.

  Add the new variants to the no-op list in `execute_context_navigation_action`. Verify with `cargo check -p mbv`.
- [ ] 5.4 Add `App::open_audiobookshelf_context_menu(targets, anchor)` next to `open_feeds_context_menu` (design D3), using the standard order. For now it emits Play and Add to Queue, plus Shuffle when there is more than one target. Group 7 adds the mark entries. In `handle_library_row_context_menu` (`src/app/shell/messages.rs`), route `ContextMenuTargets::Audiobookshelf` to it, and replace the `_ => {}` wildcard with explicit arms for every `ContextMenuTargets` variant (AGENTS: no wildcard over request variants). After opening, re-project the active Audiobookshelf owner. Verify with `cargo check -p mbv`.
- [ ] 5.5 Delete the orphaned `AudiobookshelfBookIntent::Enqueue` and `PodcastEpisodeIntent::Enqueue` (`crates/mbv-ui-msg/src/intents.rs`) and their shell arms (`src/app/shell/audiobookshelf_book.rs`, `audiobookshelf_podcast.rs`). Delete `enqueue_selected_audiobookshelf_book` and `enqueue_selected_audiobookshelf_episode_target` if nothing else calls them, along with any test that covers only the removed intents. Fix the "enqueue stays on the context menu here" comments in `book_content.rs` and `podcast_content/panel.rs` only if they are now wrong. Verify with `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 5.6 Contract (context-menu spec: standard set and the Shuffle rule): one App-level `#[case]` test of the entries `open_audiobookshelf_context_menu` builds for one episode, one book, and two episodes. The book has no Shuffle; the two episodes do. Contract (media-list-multi-select, "Play, Shuffle and Add to Queue use list order"): one test that `AudiobookshelfEnqueue` with two episode targets appends both in the given order. Verify with `cargo nextest run -p mbv`.

## 6. Audiobookshelf components open the menu

- [ ] 6.1 In `PodcastContent` (`crates/mbv-components/src/podcast_content/panel.rs`), handle `.` (no modifiers, while focused) and right-click the way `FeedsContent` does in `open_selected_feed_context` and `on_context_click`:
  - Delegate `MediaListSurfaceInput::Context` to the carrier.
  - Take `RowIntent::ContextSelection(targets)` when present, otherwise the single selected or clicked target.
  - Map each `PodcastEpisodeTarget` to `AudiobookshelfMenuTarget::Episode`.
  - Emit `ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(..), anchor)`.

  The right-click arm must come before the `_ =>` delegate arm. Show pills open no menu. Verify with `cargo check -p mbv-components`.
- [ ] 6.2 Do the same in `BookContent` (`crates/mbv-components/src/book_content.rs` and `book_content/events.rs`) for the book list, not the chapter list. Map each `String` key to `AudiobookshelfMenuTarget::Book`. `.` while chapter focus is active opens no menu. Verify with `cargo check -p mbv-components`.
- [ ] 6.3 Contract (context-menu spec "Audiobookshelf row is selected", and media-list-multi-select "one entry path for every list"): one component test per owner, in the existing files `podcast_content/tests/interaction.rs` and `book_content/tests.rs`:
  - `.` on a selected row emits `RowContextMenu` with one target and anchor `None`.
  - With a Visual multi-selection of two rows, it emits both targets in list order.

  Verify with `cargo nextest run -p mbv-components`.

## 7. Audiobookshelf Mark Played / Mark Unplayed

- [ ] 7.1 Add `ContextAction::AudiobookshelfMarkPlayed` and `AudiobookshelfMarkUnplayed`, each holding `Vec<AudiobookshelfMenuTarget>`, and include them in the `is_bulk_action` length rule. Extend `open_audiobookshelf_context_menu`:
  - One target: one entry chosen from the cached `is_finished`. A missing progress entry counts as unfinished.
  - More than one target: both entries.

  Extend the 5.6 case table with an expected label list for a finished episode. Verify with `cargo nextest run -p mbv`.
- [ ] 7.2 Add the worker path (design D5):
  - If the Audiobookshelf Service is not Ready, or `audiobookshelf_setup_and_key` returns `None`, show the existing unavailable-style toast and change nothing.
  - Otherwise start a thread modeled on `start_audiobookshelf_shows` (`src/app/dispatch/session/service_startup.rs`). Use `set_finished_bounded` for one target and `batch_set_finished_bounded` for more.
  - The thread sends a new `LibEvent` completion carrying the setup generation, the targets, the finished flag, and the result.
  - Drain it in `src/app/dispatch/run_loop/drains.rs` with a generation gate matching the shows completion.
  - On `Err`, show an error toast, and route a credential rejection through the existing ABS authentication failure classification.

  Verify with `cargo check -p mbv`.
- [ ] 7.3 On `Ok`, apply locally (design D6):
  - Mark Played: call `reconcile_audiobookshelf_progress` or `reconcile_audiobookshelf_book_progress` with `is_finished = true` and the cached time.
  - Mark Unplayed: reset only targets cached as finished, to time 0 with `is_finished = false`. Leave the others unchanged.
  - Send one `QueueOp::ApplyProgress` with the matching `ProgressUpdate`s through `queue_link(QueueScope::Local)`, leaving out the active Player-owned slot. Refactor `player_owns_active_match` into an identity-based check that the socket path and this path share.
  - Push the active Audiobookshelf owner's content.

  Verify with `cargo check -p mbv`.
- [ ] 7.4 Contract (audiobookshelf-played-state, "An accepted mark updates local progress" and "The actively owned session is not modified"): App-level tests that feed the completion event directly, with no network and no thread:
  - Mark Unplayed on a finished episode resets it to unplayed at 0.
  - A bulk Mark Unplayed leaves an in-progress episode's position alone.
  - Mark Played on the active slot's episode updates browse progress and leaves that slot out of the `ApplyProgress` updates.
  - A completion with a superseded generation changes nothing.

  Contract ("A failed mark changes nothing"): an `Err` completion leaves progress unchanged. Use a `#[case]` table only where the expected outcomes differ. Verify with `cargo nextest run -p mbv`.

## 8. Integration checks

- [ ] 8.1 Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo nextest run --workspace`, and confirm all three pass with no new lint suppression.
- [ ] 8.2 Run `openspec validate standard-media-context-menus --strict` and confirm it passes.

## Workflow follow-up

- Ask the user to test live:
  - `.` and right-click on: a movie, a TV season, a music album, a YouTube (homevideos feed) video, an Emby Latest row, Continue Watching, a Feeds entry, an ABS podcast episode, and an ABS book.
  - Confirm every menu reads "Played", never "Watched".
  - Mark Played and Mark Unplayed on one ABS item and on an ABS multi-selection. Confirm the ABS web UI agrees, and that unfinishing resets the position.
  - Shuffle on a Feeds multi-selection and on an ABS multi-selection.
- Run `make check-code-file-lines` before pushing.
- Archive (archive syncs the deltas into `openspec/specs/`).
