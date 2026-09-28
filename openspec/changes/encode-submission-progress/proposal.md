# Proposal

## Why

The Audiobookshelf "Newest episodes" shelf is cached as ready-made
`QueueItem`s whose progress is hard-coded to zero
(`shelf_entry_from_wire`, `crates/mbv-audiobookshelf/src/catalog.rs`), because
the shelf payload carries no per-user progress. Any code that returns a cache
hit verbatim submits a partly played episode from tick 0. 50c4d12d fixed the
one submission path (`selected_audiobookshelf_queue_item_target`) by hand, and
its regression test has since been lost. Nothing stops the next reader of
the cache from repeating the bug (`docs/invariants/12-submission-time-queue-item-progress.md`).
Issue #844, child of #810.

## What Changes

- The shelf cache stores progress-free episode catalog entries instead of
  `QueueItem`s. The only way from a catalog entry to an
  `AudiobookshelfQueueItem` is a constructor that requires the episode's
  resume state, so a cache hit cannot be submitted without progress.
- Both branches of `selected_audiobookshelf_queue_item_target` (cache hit and
  per-show fallback) build through that constructor from the one
  `state.progress` lookup, replacing the patch-after-clone and the hand-built
  literal.
- `PodcastContent` receives catalog entries for its Latest pill and builds its
  Hero item through the same constructor. That removes the component's second
  zero-progress literal.
- A regression test for 50c4d12d is restored.
- The invariant doc is rewritten to cover only what types don't enforce, or
  deleted.

No user-visible behaviour changes. The submission path is already correct,
and the Hero and row projections do not display the item's progress.

Out of scope: Emby. Cached `EmbyItem`s carry the server's UserData from their
fetch; none is built with zeroed progress. Whether an Emby cached list can go
stale after local playback is an invariant 06-style mirror question, not the
zero-construction hazard this change removes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change is a type-level refactor with no behaviour change
(`skip_specs: true`).

## Impact

- `crates/mbv-queue/src/audiobookshelf.rs`: new `AudiobookshelfEpisodeCatalog`,
  `EpisodeResume`, and `AudiobookshelfQueueItem::from_catalog`.
- `crates/mbv-audiobookshelf/src/catalog.rs`:
  `AudiobookshelfShelfEntry::Episode` carries the catalog type, plus its tests.
- `src/app/state/app_struct.rs`: `audiobookshelf_shelf_cache` value type.
  `src/app/dispatch/library/load.rs`: `newest_episodes_items`.
- `src/app/dispatch/audiobookshelf/browse.rs`:
  `selected_audiobookshelf_queue_item_target`.
- `src/app/shell/home_content.rs` (launch-window check) and
  `src/app/shell/audiobookshelf_podcast.rs` (Latest push).
- `crates/mbv-components/src/podcast_content.rs`: `set_latest_items` and
  `selected_episode_item`. `crates/mbv-ui-model/src/home_latest.rs`: a
  timestamp-level launch-window helper.
- No wire or persistence change: `AudiobookshelfQueueItem` keeps its fields
  and serde shape.
