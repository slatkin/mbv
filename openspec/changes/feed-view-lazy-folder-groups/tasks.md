# Tasks

## 1. Live API probe (design D6, gates the count-source choice)

- [ ] 1.1 With the user's go-ahead, probe the Emby server: fetch the feed-view library's root folder listing with the count fields requested, and record per folder whether counts are populated, what they count (all items vs videos only), and whether any folder has a positive count but no videos. Write the findings into this change directory and record the D1 choice it implies (listing counts vs per-folder count queries). Verify: findings note exists in `openspec/changes/feed-view-lazy-folder-groups/` and names the choice.

## 2. Folder listing carries counts

- [ ] 2.1 Per the probe's D1 choice: either add `ChildCount,RecursiveItemCount` to the ranged listing fetch's `Fields`, or add the per-folder videos-only count query to the Emby client. Verify: the client's parse/request unit test asserts the chosen field survives parsing (extend the existing DTO parse fixture), and `cargo nextest run -p mbv-emby` passes.

## 3. Feed-view state, lazy fetch, and count-driven pills

- [ ] 3.1 Rework `FeedHomeVideoState` per design D3: drop `all_items` and `groups`; hold the current selection (All or a folder id), its loaded items, loading flag, and the existing cursor/scroll/selection fields. Update the affected helpers and the feed-view unit tests that seeded the old shape. Verify: `cargo check -p mbv` and `cargo nextest run -p mbv` (feed-view tests) pass.
- [ ] 3.2 Implement the lazy selection fetch per design D2: selecting All runs the ranged root fetch (same `DateCreated` ascending sort and initial-page-then-total refill, minus the unplayed filter and minus grouping); selecting a folder pill fetches that folder's videos; loaded selections stay cached for the session; refresh drops caches, re-reads the folder listing, and refetches the current selection while unloaded pills stay unloaded. Verify: event-level unit tests assert first selection triggers the fetch, re-selection reuses the cache, and refresh leaves unloaded pills unloaded.
- [ ] 3.3 Drive the pill row from the folder listing and counts: projection derives pills from root-listing folders with a positive video count (watched or not; no pill for folders with no videos), and group selection resolves the folder id from the listing. Verify: unit tests assert a watched-only folder keeps its pill, an empty folder gets none, and no full-library fetch precedes the pill row.

## 4. Marking played keeps rows

- [ ] 4.1 Remove the feed-view branches from `remove_watched_item_from_library` and `remove_played_item_from_library`, drop `refresh_completed_feed_home_videos`'s feed-view cache stripping, and delete `remove_item_from_feed_home_video_cache` once orphaned (design D4). Verify: a unit test asserts marking played in the feed view leaves the selection's items in place, and `cargo nextest run -p mbv` passes.

## 5. Delete the aggregate machinery

- [ ] 5.1 Delete `spawn_feed_home_video_aggregate`, the `FeedHomeVideoAggregated` event with its exhaustive dispatch arm, the `maybe_aggregate_feed_*` and `maybe_refresh_feed_groups_after_refresh` re-spawn chain, and stale aggregate-only helpers; refresh the `is_feed_home_video_group_view` comment (design D5). Verify: `cargo check -p mbv` shows no orphaned variants (dispatch arms stay exhaustive) and the aggregate has no remaining references.

## 6. Integration checks

- [ ] 6.1 Run `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt`, `cargo nextest run -p mbv`, and `make check-code-file-lines`; fix or split anything they surface. Verify: all four pass.

## Workflow follow-up

- When the user approves the PR for merge: sync the delta into `openspec/specs/feed-home-video-group-view/spec.md` and archive the change as the last commit before the merge. Do not sync or archive when only opening the PR.
