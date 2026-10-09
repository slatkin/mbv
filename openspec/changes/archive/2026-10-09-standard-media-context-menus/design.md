# Design

## Context

See proposal.md (Why) for motivation. These parts of the current code shape
the approach:

- Inventory of library menus before this change:

  ```
  Surface                       Emitted targets   Builder (menu.rs)              Entries today
  ----------------------------  ----------------  -----------------------------  -------------------------------------------
  Movies / Generic leaf         Browser(ids)      push_leaf_context_actions      Play, Add, Mark Watched|Unwatched
  Emby folder / BoxSet          Browser(ids)      push_folder_context_actions    Play All, Shuffle, Add, Mark Watched|Unwatched
  TV series / season / episode  Emby(items)       folder / leaf                  as above
  Music artist / album          Emby(items)       push_folder_context_actions    Play All, Shuffle, Add, Mark Watched
  Music track                   Emby(items)       push_leaf_context_actions      Play, Add (Audio skips mark)
  YouTube (homevideos feed)     Browser(ids)      none resolves (below)          no menu
  Continue Watching             Home(targets)     leaf + CW remove               Play, Add, Mark, Remove from CW
  Feeds                         Feeds(entries)    open_feeds_context_menu        Play, Add, Mark Played, Mark Unplayed
  ABS episode / book            none              none                           no menu
  Any Emby multi-selection      Emby / Browser    open_context_menu_for_selection Play, Shuffle, Add, Remove, Mark Played, Mark Unplayed
  ```

- `EmbyLibraryContent` sends `ContextMenuTargets::Browser(ids)` from `.`
  (`emby_library_content/input.rs:204`), from right-click (`panel.rs:319`),
  and from inline search (`input.rs:88`). The shell resolves the IDs against
  `libs[i].nav_stack.last().items` (`matching_context_items`,
  `src/app/shell/messages.rs:573`). The rows themselves come from somewhere
  else, `push_emby_library_owner_content`
  (`src/app/shell/emby_library_content.rs:118`):
  - In a homevideos feed view, from `feed_home_video_selected_items`. Here the
    nav level holds the channel folders, not the videos.
  - In Latest mode, from `tv_latest_snapshots`.

  So the lookup misses those rows. TV and Music already send
  `ContextMenuTargets::Emby(items)` straight from their own rows.
- `EmbyItem::is_music()` (`crates/mbv-emby-model/src/lib.rs:208`) documents
  that music played and resume facts mean nothing in mbv. Today the menu
  checks only `is_audio`, in `push_leaf_context_actions` and
  `emby_item_capabilities`, so albums and artists still get a mark entry.
- Menus are built in `src/app/dispatch/context_menu/menu.rs` and executed in
  `actions.rs`. `resolve_context_menu_target` returns `None` for
  `TabSelection::AudiobookshelfLibrary(_)` and `TabSelection::Feeds`.
- Feeds opens its menu outside that resolver. `FeedsContent` handles `.`
  (`open_selected_feed_context`) and right-click (`on_context_click`). Both
  emit `ShellRequest::RowContextMenu(ContextMenuTargets::Feeds(entries),
  anchor)`, and the shell routes that to `App::open_feeds_context_menu`
  (`src/app/shell/messages.rs:599`). The Feeds menu carries resolved
  `FeedEntry` values in each `ContextAction`.
- `PodcastContent` (`MediaListCarrier<PodcastEpisodeTarget>`) and
  `BookContent` (`MediaListCarrier<String>`, keyed by book `libraryItemId`)
  use the same carrier as Feeds. Both already support Visual multi-selection
  through `handle_visual_key`.
- ABS browse progress is stored in
  `audiobookshelf_browse[i].progress: HashMap<(libraryItemId, episodeId), _>`
  and `audiobookshelf_book_browse[i].progress: HashMap<libraryItemId, _>`.
  Each entry has an `is_finished` field.
  `reconcile_audiobookshelf_progress` and
  `reconcile_audiobookshelf_book_progress`
  (`src/app/dispatch/library/event_reconcile.rs`) write them. The socket path
  (`apply_audiobookshelf_socket_progress`) relays queue progress through
  `QueueOp::ApplyProgress`, and it skips the active Player-owned slot
  (`player_owns_active_match`).
- ABS REST calls run on worker threads. They use `audiobookshelf_client(&config)`
  and a `*_bounded` method with `REQUEST_HARD_BOUND`, and they return a
  `LibEvent` that is checked against the setup generation
  (`src/app/dispatch/session/service_startup.rs`). The Emby mark path calls
  synchronously instead. ABS keeps its own pattern.
- ABS server behavior, verified in the `advplyr/audiobookshelf` source
  (`MeController`, `MediaProgress.applyProgressUpdate`):
  - `PATCH /api/me/progress/:libraryItemId/:episodeId?` with `{isFinished}`.
  - `PATCH /api/me/progress/batch/update` with
    `[{libraryItemId, episodeId?, isFinished}]`. It returns 200 even when some
    entries fail, and logs the failures on the server.
  - Unfinishing a finished item sets `currentTime = 0`. `isFinished: false` on
    an item that is not finished changes nothing. Finishing keeps
    `currentTime`.
  - Both endpoints send only the `user_updated` socket event, never
    `user_item_progress_updated`.

## Goals / Non-Goals

**Goals:**
- One standard action set across every library list. Each departure from it
  is a named exception in the `context-menu` spec, with a stated reason.
- Every list's menu targets come from the rows that list paints. The shell
  never re-derives them from a different item source.
- ABS rows open menus through the same path as Feeds: component →
  `RowContextMenu` → App builder.
- Each menu carries resolved targets, so a later cursor or refresh change
  cannot redirect the action.
- After a 200, local progress matches what the server now holds, with no
  re-fetch.

**Non-Goals:**
- Handling the `user_updated` socket event. The `audiobookshelf-progress-refresh`
  rule that only `user_item_progress_updated` counts as listening progress is
  unchanged.
- Played-state for Audiobookshelf or Feed items in the Queue panel. The
  mixed-kind Queue rule in `media-list-multi-select` is unchanged.
- The Queue panel menu (Play, Remove from Queue, Go to Library), the
  Playlists sidebar (where right-click goes back), and the Search sidebar.
  None of them is a library list.
- Keyboard shortcuts such as Ctrl+W toggle-watched. Only menus change.
- Adding a populated-queue confirmation gate for Feeds or ABS Play/Shuffle.
  Both follow the existing Feeds path (`submit_queue_item(first, true)`, then
  append the rest).

## Decisions

### D1. A dedicated ABS target type, not `QueueItemContentId`

Add to `mbv-ui-model/src/context_menu.rs`:

```rust
pub enum AudiobookshelfMenuTarget {
    Episode { library_item_id: String, episode_id: String },
    Book { library_item_id: String },
}
```

Also add `ContextMenuTargets::Audiobookshelf(Vec<AudiobookshelfMenuTarget>)`.
`QueueItemContentId` has the same two shapes, but it also has `Emby` and
`Feed` variants, so an Emby identity could reach an ABS action. That state
should be impossible to represent. `PodcastEpisodeTarget` lives in
`mbv-ui-msg`, which depends on `mbv-ui-model`, so it can't be used here. The
component converts its carrier key into this type.

### D2. One set of ABS `ContextAction` variants for single rows and selections

Add these variants:

- `AudiobookshelfPlay(Vec<AudiobookshelfMenuTarget>)`
- `AudiobookshelfShuffle(Vec<_>)`
- `AudiobookshelfEnqueue(Vec<_>)`
- `AudiobookshelfMarkPlayed(Vec<_>)`
- `AudiobookshelfMarkUnplayed(Vec<_>)`
- `FeedsShuffle(Vec<FeedEntry>)`

A single row passes a one-element vector. This matches the existing Feeds
variants. Emby keeps separate single and selection variants, but only because
its single-row actions carry no values.

`is_bulk_action` includes an ABS or Feeds action when its vector holds more
than one target. Its only callers, the two Select paths in
`src/app/shell/overlays/menus.rs`, clear the multi-selection when it returns
true, using the origin that `handle_library_row_context_menu` records. Today
it lists only the `*Selection` variants, so a Feeds bulk action never clears
its selection. That is a gap against `media-list-multi-select` ("Running any
multi-selection action SHALL clear the multi-selection"), and the length
check fixes it for Feeds and ABS together.

The rejected alternative was routing through the orphaned
`AudiobookshelfBookIntent::Enqueue` and `PodcastEpisodeIntent::Enqueue`.
Those resolve the component's current selection when they run, not when the
menu opens, and they cannot carry a selection. Delete both variants and their
shell arms. Nothing sends to them today.

### D3. ABS menu builder lives in the App, next to `open_feeds_context_menu`

Add `App::open_audiobookshelf_context_menu(targets, anchor)`. It reads
finished state from the current tab's browse progress map. A missing entry
counts as unfinished.

- One target: Play, Add to Queue, then Mark Unplayed if it is finished,
  otherwise Mark Played.
- More than one target: Play, Shuffle, Add to Queue, Mark Played, Mark
  Unplayed.

`open_feeds_context_menu` gets the same split:

- One entry: Play, Add to Queue, then one entry chosen from `entry.played`.
- More than one entry: add Shuffle and both mark entries.

The shell arm for `ContextMenuTargets::Audiobookshelf` calls the new builder.
That replaces the `_ => {}` wildcard there with explicit arms, as AGENTS
requires for request variants.

### D4. Resolving ABS targets to queue items by identity

Add id-taking resolvers:

- Episodes: use the existing `selected_audiobookshelf_queue_item_target`.
- Books: add an id-taking variant of `audiobookshelf_book_queue_item`, which
  today reads only the selected book.

Play and Shuffle resolve every target. Shuffle randomizes the order. Then
`submit_queue_item(first, true)` runs, and each remaining item goes to
`append_on_owner`. Add to Queue appends in order. Admission keeps the
existing `can_admit_audiobookshelf` checks and toast. A target that no longer
resolves is skipped.

### D5. Marking runs on a worker; the result comes back as a generation-gated event

Add to `mbv-audiobookshelf`:

- `set_finished_bounded(api_key, library_item_id, episode_id: Option<&str>, finished, hard_bound)`
  sends `PATCH /api/me/progress/{id}[/{ep}]`.
- `batch_set_finished_bounded(api_key, &[ProgressFinishedUpdate], hard_bound)`
  sends `PATCH /api/me/progress/batch/update`.

Both reuse the `post_json` and `map_error` shape with a `patch` verb, and
encode path segments with `mbv_net::encode_path_segment`.

`App` starts a thread with the targets and the current setup generation. One
target uses the single call, more than one uses the batch call. The thread
sends a `LibEvent` completion. On drain, a superseded generation is dropped.

- On `Err`: show the toast, and route a credential rejection through the
  existing ABS authentication failure classification.
- On `Ok`: apply each target locally, as in D6.

The batch endpoint reports 200 even when some entries fail. mbv treats a 200
as success for every target. The server will correct any wrong entry on the
next catalog refresh.

### D6. Local apply mirrors the server rules

For each target, read the current cached progress:

- Mark Played: `reconcile_*` with `is_finished = true` and the cached
  `current_time_seconds`. Use 0 if there is no entry.
- Mark Unplayed on a finished item: `reconcile_*` with `is_finished = false`
  and `current_time_seconds = 0`.
- Mark Unplayed on an item that is not finished: no change. This matches the
  server's no-op.

Queue: build `mbv_ctrl::ProgressUpdate` values for the same targets, leave
out the target that `player_owns_active_match` would match, and send one
`QueueOp::ApplyProgress`. Pull the matching check out of
`player_owns_active_match`, which today takes the socket's progress type, so
both callers share an identity-based check. Episodes use
`QueueItemContentId::Audiobookshelf` and books use `AudiobookshelfBook`.

After applying, re-project with `push_audiobookshelf_podcast_content` or
`push_audiobookshelf_book_content`.

### D7. Emby library lists send the items they paint; `Browser` is removed

`EmbyLibraryContent` resolves its menu targets from its own rows. It uses
`items()` and the carrier's `ContextCurrent` and `ContextClick` targets for
the list, and `inline_search.item_for_target` for search results. It sends
`ContextMenuTargets::Emby(items)`, as TV and Music already do. This removes
the shell-side ID lookup, so menu targets come from the same source as the
painted rows, whether that is the nav level, a homevideos feed group, or a
Latest snapshot. Delete the `Browser` variant, its shell arm, and
`matching_context_items` if nothing else uses it.

The rejected alternative was teaching the shell's Browser arm to pick the
source the projection used, feed group or Latest. That copies the
projection's branching into a second place, and the next row source would
break the menu again.

### D8. One standard in the Emby builders

The standard is defined in the `context-menu` delta.

- `push_play_state_context_action`: labels become "Mark Played" and "Mark
  Unplayed". Return early when `item.is_music()`, which covers albums and
  artists along with tracks. This also replaces the `media_type`/`item_type`
  Audio check in `push_leaf_context_actions`.
- `emby_item_capabilities`: `played_state_capable = !item.is_music()`, so a
  multi-selection that includes any music item drops both mark entries.
- `open_context_menu_for_selection`: move Remove after Mark Unplayed. Remove
  from Continue Watching is already after the mark entry.
- Folder rows keep "Play All". Leaf rows and multi-selections keep "Play".

Feeds (D3) and Audiobookshelf (D3) follow the same order. Removal entries
always come last.

## Risks / Trade-offs

- **Marking the playing item can be undone.** The Player's next session sync
  sends a new `currentTime`. If the playhead is not near the end, the server
  then clears `isFinished`. → This is accepted and matches the ABS web client.
  The spec keeps the active slot untouched (`audiobookshelf-played-state`).
- **A batch 200 can hide per-item failures.** → mbv applies the change
  locally anyway, and the next catalog load corrects it.
- **The Unplayed pill can hold a row that was just marked played.** Marking
  an item played while the `Unplayed` pill is active should drop the row. →
  The component re-filters from pushed progress. Verify this during
  implementation and fix it in the push if it doesn't.
- **Tests pin the old labels and order.** Existing tests that assert "Mark
  Watched", the Remove position, or a mark entry on albums will fail. → They
  assert the behavior this change replaces. Update their expectations, and
  don't add parallel tests.
- **Emby targets carry whole `EmbyItem` values.** This was already true for
  TV, Music, and `EmbyLibraryPlay { item }`, so there's no new pattern.
- **`is_bulk_action` gains a length check.** A one-element Feeds or ABS
  action clears nothing, and a larger one clears the origin list's selection.
  → The two callers in `menus.rs` already handle a missing origin.
