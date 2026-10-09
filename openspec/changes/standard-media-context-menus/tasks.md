# Tasks

## 1. Audiobookshelf finished-state client calls

- [ ] 1.1 In `crates/mbv-audiobookshelf`, add `set_finished_bounded(api_key, library_item_id, episode_id: Option<&str>, finished, hard_bound)`. It sends `PATCH /api/me/progress/{libraryItemId}` with `/{episodeId}` appended for episodes, and the body `{"isFinished": bool}`. Add a `patch_json` helper beside `post_json` in `playback.rs`, reusing `catalog::map_error` and `mbv_net::encode_path_segment` for every path segment. Add `batch_set_finished_bounded(api_key, &[ProgressFinishedUpdate], hard_bound)`, which sends `PATCH /api/me/progress/batch/update` with a JSON array of `{libraryItemId, episodeId?, isFinished}`. Leave `episodeId` out of the JSON for books, using `skip_serializing_if`. Verify with `cargo check -p mbv-audiobookshelf`.
- [ ] 1.2 Add one test file to the crate's existing `tests/` binary, using the `with_test_agent` mock pattern the existing tests use. Contract: a single episode, a single book, and a batch request each send the correct method, path, and body, and a 401 maps to the authentication-rejected error class. Use one `#[case]` table for the three request shapes. Verify with `cargo nextest run -p mbv-audiobookshelf`.

## 2. Feeds menu conforms to the standard entry set

- [ ] 2.1 In `crates/mbv-ui-model/src/context_menu.rs`, add `ContextAction::FeedsShuffle(Vec<FeedEntry>)`. Change `is_bulk_action` so that any `Feeds*` action whose vector has more than one element counts as bulk (design D2). Add an exhaustive arm for `FeedsShuffle` in `execute_context_queue_and_feed_action` (`src/app/dispatch/context_menu/actions.rs`): shuffle the entries with `rand`, then call `play_feed_entries`. Add `FeedsShuffle` to the no-op list in `execute_context_navigation_action`. Verify with `cargo check --workspace`.
- [ ] 2.2 In `open_feeds_context_menu` (`src/app/dispatch/context_menu/menu.rs`), split on the entry count (design D3). One entry gives Play, Add to Queue, then "Mark Unplayed" if `entry.played`, otherwise "Mark Played". More than one gives Play, Shuffle, Add to Queue, Mark Played, Mark Unplayed. Contract (context-menu spec, single-row played-state entry and the Shuffle rule): add a `#[case]` test in the existing Feeds menu tests (`src/app/tests/podcast.rs`, or wherever `open_feeds_context_menu` is exercised today) asserting the label list for an unplayed single entry, a played single entry, and two entries. Verify with `cargo nextest run -p mbv`.

## 3. Audiobookshelf menu: play, shuffle and enqueue

- [ ] 3.1 In `crates/mbv-ui-model/src/context_menu.rs`, add `AudiobookshelfMenuTarget { Episode { library_item_id, episode_id }, Book { library_item_id } }` (design D1) and `ContextMenuTargets::Audiobookshelf(Vec<AudiobookshelfMenuTarget>)`. Add `ContextAction::AudiobookshelfPlay`, `AudiobookshelfShuffle` and `AudiobookshelfEnqueue`, each holding `Vec<AudiobookshelfMenuTarget>`, and add them to the `is_bulk_action` length rule. Verify with `cargo check -p mbv-ui-model`.
- [ ] 3.2 Add an id-taking book resolver beside `audiobookshelf_book_queue_item` (`crates/mbv-ui-model/src/audiobookshelf_browse/books.rs`) that builds the book `QueueItem` for a given `libraryItemId`. Rebuild the existing selected-book function on top of it so the queue-item construction lives in one place. Add an App helper that maps `&[AudiobookshelfMenuTarget]` to `Vec<QueueItem>` for the current Audiobookshelf tab. Episodes go through `selected_audiobookshelf_queue_item_target`, and targets that don't resolve are skipped (design D4). Verify with `cargo check -p mbv`.
- [ ] 3.3 In `actions.rs`, add exhaustive arms for the three new actions (design D4):
  - Play and Shuffle: check `can_admit_audiobookshelf`, using the existing toast text on failure. Shuffle randomizes the order. Then call `submit_queue_item(first, true)` and append the rest with `submit_queue_item(_, false)`.
  - Enqueue: append each item in order, with the same bound-scope admission check as `enqueue_selected_audiobookshelf_episode_target`.

  Add the new variants to the no-op list in `execute_context_navigation_action`. Verify with `cargo check -p mbv`.
- [ ] 3.4 Add `App::open_audiobookshelf_context_menu(targets, anchor)` next to `open_feeds_context_menu` (design D3). For now it emits only Play, Add to Queue, and Shuffle for more than one target. Group 5 adds the mark entries. In `handle_library_row_context_menu` (`src/app/shell/messages.rs`), route `ContextMenuTargets::Audiobookshelf` to it, and replace the `_ => {}` wildcard with explicit arms (AGENTS: no wildcard over request variants). Re-project the active Audiobookshelf owner after opening. Verify with `cargo check -p mbv`.
- [ ] 3.5 Delete the orphaned `AudiobookshelfBookIntent::Enqueue` and `PodcastEpisodeIntent::Enqueue` (`crates/mbv-ui-msg/src/intents.rs`) and their shell arms (`src/app/shell/audiobookshelf_book.rs`, `audiobookshelf_podcast.rs`). Delete `enqueue_selected_audiobookshelf_book` and `enqueue_selected_audiobookshelf_episode_target` if nothing else calls them, and delete any test that covers only the removed intents. Fix the "enqueue stays on the context menu here" comments in `book_content.rs` and `podcast_content/panel.rs` only if they are now wrong. Verify with `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] 3.6 Contract (context-menu spec, standard media actions and the Shuffle rule): one App-level `#[case]` test asserting the entries `open_audiobookshelf_context_menu` builds for one episode, one book, and two episodes. Book has no Shuffle; two episodes include Shuffle. Contract (media-list-multi-select, "Play, Shuffle and Add to Queue use list order"): one test that `AudiobookshelfEnqueue` with two episode targets appends both in the given order. Verify with `cargo nextest run -p mbv`.

## 4. Audiobookshelf components open the menu

- [ ] 4.1 In `PodcastContent` (`crates/mbv-components/src/podcast_content/panel.rs`), handle `.` (no modifiers, while focused) and right-click the same way `FeedsContent` does in `open_selected_feed_context` and `on_context_click`. Delegate `MediaListSurfaceInput::Context` to the carrier, take `RowIntent::ContextSelection(targets)` when it is present, otherwise use the single selected or clicked target, map each `PodcastEpisodeTarget` to `AudiobookshelfMenuTarget::Episode`, and emit `ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(..), anchor)`. The right-click arm must come before the `_ =>` delegate arm. Verify with `cargo check -p mbv-components`.
- [ ] 4.2 Do the same in `BookContent` (`crates/mbv-components/src/book_content.rs` and `book_content/events.rs`) for the book list, not the chapter list. Map each `String` key to `AudiobookshelfMenuTarget::Book`. `.` while chapter focus is active opens no menu. Verify with `cargo check -p mbv-components`.
- [ ] 4.3 Contract (context-menu spec, "Audiobookshelf row is selected", and media-list-multi-select, one entry path for every list): one component test per owner in the existing component test files (`podcast_content/tests/interaction.rs` and `book_content/tests.rs`). `.` on a selected row emits `RowContextMenu` with one target and anchor `None`. With a Visual multi-selection of two rows, it emits both targets in list order. Verify with `cargo nextest run -p mbv-components`.

## 5. Audiobookshelf Mark Played / Mark Unplayed

- [ ] 5.1 Add `ContextAction::AudiobookshelfMarkPlayed` and `AudiobookshelfMarkUnplayed`, each holding `Vec<AudiobookshelfMenuTarget>`, with the `is_bulk_action` length rule. Extend `open_audiobookshelf_context_menu`. One target gets one entry chosen from cached `is_finished`, where a missing progress entry counts as unfinished. More than one target gets both entries. Verify with `cargo check -p mbv`.
- [ ] 5.2 Add the worker path (design D5). On a mark action, if the Audiobookshelf Service is not Ready or `audiobookshelf_setup_and_key` returns `None`, show the existing unavailable-style toast and change nothing. Otherwise start a thread modeled on `start_audiobookshelf_shows` (`src/app/dispatch/session/service_startup.rs`). One target uses `set_finished_bounded`, more than one uses `batch_set_finished_bounded`. The thread sends a new `LibEvent` completion carrying the setup generation, the targets, the finished flag, and the result. Drain it in `src/app/dispatch/run_loop/drains.rs` with a generation gate that matches the shows completion. On `Err`, show an error toast and route a credential rejection through the existing ABS authentication failure classification. Verify with `cargo check -p mbv`.
- [ ] 5.3 On `Ok`, apply locally (design D6). Mark Played calls `reconcile_audiobookshelf_progress` or `reconcile_audiobookshelf_book_progress` with `is_finished = true` and the cached time. Mark Unplayed resets only targets cached as finished, to time 0 with `is_finished = false`, and leaves other targets unchanged. Send one `QueueOp::ApplyProgress` with matching `ProgressUpdate`s through `queue_link(QueueScope::Local)`, leaving out the active Player-owned slot. Refactor `player_owns_active_match` into an identity-based check that the socket path and this path share. Then push the active Audiobookshelf owner's content. Verify with `cargo check -p mbv`.
- [ ] 5.4 Contract (audiobookshelf-played-state, "An accepted mark updates local progress" and "The actively owned session is not modified"): App-level tests that feed the completion event directly (no network, no thread):
  - Unplayed on a finished episode resets it to unplayed at 0.
  - Bulk Unplayed leaves an in-progress episode's position alone.
  - Played on the active slot's episode updates browse progress and leaves that slot out of the `ApplyProgress` updates.
  - A completion with a superseded generation changes nothing.

  Contract (a failed mark changes nothing): an `Err` completion leaves progress unchanged. Use one `#[case]` table only where the expected outcomes differ. Verify with `cargo nextest run -p mbv`.

## 6. Integration checks

- [ ] 6.1 Run `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo nextest run --workspace`, and confirm all three pass with no new lint suppression.
- [ ] 6.2 Run `openspec validate standard-media-context-menus --strict` and confirm it passes.

## Workflow follow-up

- Ask the user to test this live:
  - `.` and right-click on an ABS podcast episode, an ABS book, and a Feeds entry.
  - Mark Played and Mark Unplayed on one ABS item and on a multi-selection. Confirm the ABS web UI shows the same state and that unfinishing resets the position.
  - Shuffle on a Feeds or ABS multi-selection.
- Run `make check-code-file-lines` before pushing.
- Archive (archive syncs the deltas into `openspec/specs/`).
