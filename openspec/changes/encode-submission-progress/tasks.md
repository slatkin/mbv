# Tasks

Groups 1–2 land as ONE commit: group 1 changes public types that group 2's
callers use. No new lint suppression, no behaviour change.

Final gate (end of group 2):
- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run -p mbv-queue -p mbv-audiobookshelf -p mbv-ui-model -p mbv-components -p mbv`

## 1. Types (design D1, D2)

- [ ] 1.1 In `crates/mbv-queue/src/audiobookshelf.rs`, add
  `AudiobookshelfEpisodeCatalog`, `EpisodeResume` (private fields,
  `NOT_STARTED`, `from_seconds`), and `AudiobookshelfQueueItem::from_catalog`
  exactly per design D1. Re-export them from `crates/mbv-queue/src/lib.rs`
  next to `AudiobookshelfQueueItem`.

  Add one named `#[case]` table,
  `from_catalog_applies_resume_state`, with two cases: `NOT_STARTED`, and
  `from_seconds(90.0, true)` → position is 90 s in ticks and
  `played == is_finished == true`. Verify: `cargo nextest run -p mbv-queue`
  passes.
- [ ] 1.2 In `crates/mbv-audiobookshelf/src/catalog.rs`, change
  `AudiobookshelfShelfEntry::Episode` to carry `AudiobookshelfEpisodeCatalog`.
  `shelf_entry_from_wire` builds it and drops the three zero fields. Update
  `src/tests/catalog.rs` to the new shape, keeping its assertions. Verify:
  `cargo nextest run -p mbv-audiobookshelf` passes.
- [ ] 1.3 In `crates/mbv-ui-model/src/home_latest.rs`, add
  `timestamp_in_launch_window(Option<u64>, HomeLatestLaunchWindow) -> bool`
  and make `is_new_in_launch_window` delegate to it. The existing
  `home_latest/tests.rs` table already owns the window arithmetic, so add no
  test. Verify: `cargo nextest run -p mbv-ui-model` passes.

## 2. Cache and callers (design D2, D3)

- [ ] 2.1 Retype `audiobookshelf_shelf_cache` (`src/app/state/app_struct.rs`) to
  `HashMap<String, Vec<mbv_queue::AudiobookshelfEpisodeCatalog>>`. Update
  `newest_episodes_items` (`src/app/dispatch/library/load.rs`) to return that
  type, and `home_content.rs:175` to call `timestamp_in_launch_window(item.pub_date_secs, ..)`.
  Verify: `cargo check -p mbv` shows no errors in those three files.
- [ ] 2.2 Rewrite `selected_audiobookshelf_queue_item_target`
  (`src/app/dispatch/audiobookshelf/browse.rs`) per design D3: one `resume`,
  and both branches build a catalog and return
  `QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem::from_catalog(..)))`.
  Delete the field-patching block and the hand-built literal.

  Add `shelf_cache_hit_carries_browse_progress` (regression, 50c4d12d, issue
  #844) to `src/app/tests/audiobookshelf_browse_actions_sibling_tests.rs`. It
  seeds the shelf cache and `state.progress` for the same episode, then
  asserts the returned item's `position_ticks` and `played`. Verify: the
  test passes and `rg -n "position_ticks\s*=" src/app/dispatch/audiobookshelf/browse.rs`
  is empty.
- [ ] 2.3 `crates/mbv-components/src/podcast_content.rs`:
  - `set_latest_items(&[AudiobookshelfEpisodeCatalog])`, with `latest_items`
    retyped.
  - `active_episodes` and the row show-title fallback read catalog fields.
  - `selected_episode_item` builds a catalog in both branches and returns
    `from_catalog` with resume from `self.state.progress`, converted with
    `EpisodeResume::from_seconds`.

  Update `src/app/shell/audiobookshelf_podcast.rs:55` to push the catalog
  slice, and fix any component test constructing latest items. Then run the
  final gate and commit groups 1–2 as one commit referencing #844. Verify: the
  gate is green, and
  `rg -n "position_ticks: 0" crates/mbv-components/src/podcast_content.rs crates/mbv-audiobookshelf/src/catalog.rs src/app/dispatch/audiobookshelf/browse.rs`
  is empty.

## 3. Docs

- [ ] 3.1 Delete `docs/invariants/12-submission-time-queue-item-progress.md`,
  or reduce it to the residual: a hand-built `AudiobookshelfQueueItem` literal
  must name its progress. Keep it only if that residual is worth a doc; the
  default is delete. Remove its link from invariant 06 if one remains, and
  tick invariant 12 on #810. Verify: `rg -n "12-submission-time" docs openspec`
  is empty after a delete.
