# Design

## Context

See proposal.md (Why). The flow today:

- `shelf_entry_from_wire` (`crates/mbv-audiobookshelf/src/catalog.rs`) builds
  `AudiobookshelfShelfEntry::Episode(AudiobookshelfQueueItem { position_ticks: 0, played: false, is_finished: false, .. })`.
- `App::newest_episodes_items` (`src/app/dispatch/library/load.rs`) wraps those
  items as `QueueItem`s into `audiobookshelf_shelf_cache: HashMap<String, Vec<QueueItem>>`.
- The cache has three readers:
  1. `selected_audiobookshelf_queue_item_target`
     (`src/app/dispatch/audiobookshelf/browse.rs`) is the only submission path.
     It clones a hit and patches three fields from `state.progress`. Its
     per-show fallback builds a literal from the same lookup.
  2. `home_content.rs:175`, the launch-window "new content" marker. It reads
     `pub_date_secs` only.
  3. `audiobookshelf_podcast.rs:55` → `PodcastContent::set_latest_items`. Rows
     read catalog fields and take progress from `state.progress`. The Hero
     (`selected_episode_item` → `hero_content_abs_episode`) reads no progress
     field. Its non-Latest branch builds a second zero-progress literal.
- `AudiobookshelfQueueItem` is persisted and sent on the ctrl wire (serde).
  Its public fields are read across the workspace.

## Goals / Non-Goals

**Goals:**
- A cached Audiobookshelf episode has no progress fields, so it cannot be
  submitted without supplying resume state.
- Resume state has one conversion point from Audiobookshelf seconds.

**Non-Goals:**
- Making `AudiobookshelfQueueItem`'s fields private or restructuring its
  serde shape. That touches persistence, the wire, and every reader, far
  beyond this hazard. A struct literal must still name its progress fields
  explicitly. The hazard was a cached value passed through unchanged, not a
  literal.
- Books. `book_queue_item` (`crates/mbv-ui-model/src/audiobookshelf_browse/books.rs`)
  already builds from `state.progress`, and nothing caches books as queue
  items.
- Emby (see proposal Out of scope).

## Decisions

**D1. Catalog and resume types in `mbv-queue`.** In
`crates/mbv-queue/src/audiobookshelf.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AudiobookshelfEpisodeCatalog {
    pub library_item_id, pub episode_id, pub title, pub show_title, pub author,
    pub description, pub duration_ticks, pub pub_date_secs, pub cover_path,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeResume { position_ticks: i64, is_finished: bool }
impl EpisodeResume {
    pub const NOT_STARTED: Self;
    pub fn from_seconds(current_time_seconds: f64, is_finished: bool) -> Self;
}
impl AudiobookshelfQueueItem {
    pub fn from_catalog(catalog: AudiobookshelfEpisodeCatalog, resume: EpisodeResume) -> Self;
}
```

- The catalog has the same field types as the matching
  `AudiobookshelfQueueItem` fields. It has no serde derive, because it is never
  persisted.
- `EpisodeResume` fields are private, so the only ways to build one are the
  named "no known progress" constant and the seconds conversion.
- `from_seconds` uses the same rounding as `browse.rs`'s `seconds_to_ticks`
  (non-finite or negative → 0).
- `from_catalog` sets `played = is_finished = resume.is_finished`, matching
  both current sites.

It lives in `mbv-queue` because `mbv-audiobookshelf`, `mbv-components` and the
TUI already depend on it, and the constructor belongs to the type it builds.

*Alternative:* a trait or a `Progressless<T>` wrapper around the queue item.
Rejected: the wrapper would still hold zeroed fields inside, and a
`.into_inner()` would reintroduce the bypass.

**D2. Only catalog entries cross the cache.**
- `AudiobookshelfShelfEntry::Episode` carries `AudiobookshelfEpisodeCatalog`.
- `newest_episodes_items` returns `Vec<AudiobookshelfEpisodeCatalog>`.
- `audiobookshelf_shelf_cache` becomes
  `HashMap<String, Vec<AudiobookshelfEpisodeCatalog>>`.
- `set_latest_items` takes `&[AudiobookshelfEpisodeCatalog]`, and
  `PodcastContent::latest_items` stores them.

The launch-window marker needs a timestamp without a `QueueItem`. Add
`home_latest::timestamp_in_launch_window(Option<u64>, HomeLatestLaunchWindow)`,
and make `is_new_in_launch_window` delegate to it.

**D3. One construction path at submission.**
`selected_audiobookshelf_queue_item_target` computes
`resume = progress.map_or(EpisodeResume::NOT_STARTED, |p| EpisodeResume::from_seconds(p.current_time_seconds, p.is_finished))`
once. Both branches build an `AudiobookshelfEpisodeCatalog`: the cache hit
clones it, and the fallback assembles it from episode and show. Both return
`AudiobookshelfQueueItem::from_catalog(catalog, resume)`.

`PodcastContent::selected_episode_item` builds a catalog in both branches
and calls `from_catalog` with resume from its own `state.progress`. The Hero
ignores it, but the literal and its zeros are gone.

## Risks / Trade-offs

- [Field-list duplication between the catalog and the queue item.] Adding a
  catalog field later means touching both types. `from_catalog` is an
  exhaustive struct literal, so the compiler flags any field that is missing.
- [`EpisodeResume::NOT_STARTED` can still be passed wrongly.] It is a named
  choice at the call site rather than a silent default of cached data, which
  is the property the invariant asks for.
