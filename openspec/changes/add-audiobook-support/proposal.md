## Why

Audiobookshelf `media_type == "book"` libraries are currently filtered out at `run_loop_drains.rs`, so users with audiobook libraries see no tab for them even though Audiobookshelf podcast support (roadmap #504, milestones 1-5) is complete. Milestone 6 closes that gap. The UI and data-model decisions were worked out ahead of implementation in #536 so this change applies them rather than re-deriving them.

## What Changes

- Lift the `media_type == "podcast"` filter; book and podcast libraries interleave as peer tabs in server order, same as Emby libraries today. No type-partitioning or reordering.
- Add a book browsing surface using the Music wide two-column hero-on-left layout (not the TV/podcast vertical hero), with an inline progress % in the hero meta.
- Group and sort books alphabetically by author surname only (via the `human_name` crate), first-listed author decides sort position for multi-author books. No grouping pills, no series/narrator grouping.
- Fork `Service browse dispatch` once per Audiobookshelf tab by `media_type` (podcast vs book) at `TabSelection::AudiobookshelfLibrary(usize)` resolution; downstream state, renderers, and input handlers never re-check `media_type` per action.
- Add a new `QueueItemKind::AudiobookshelfBook` queue-item, content-identity, and progress-event shape, distinct from the existing episode-shaped `QueueItemKind::Audiobookshelf`. A book queues as one item; its `audioFiles` are handed to mpv as a single merged/EDL timeline (no manual file-offset math in mbv).
- Add chapter rows as first-class browsable/seekable units: each row triggers one absolute seek on the merged timeline using the book-relative `chapters[]` Audiobookshelf already provides.
- No "downloaded" filter concept for books — a book's files exist because they were imported into the library or the item doesn't exist; ABS surfaces missing-file failures as request errors, same as mbv already handles for Emby.

## Capabilities

### New Capabilities
- `audiobookshelf-book-browsing`: Read-only discovery and browsing of Audiobookshelf book libraries — library discovery, author-surname grouping, the Music-style hero-on-left tab composition, chapter list display, and read-only progress display.
- `audiobookshelf-book-playback`: Provider-native queue identity for books (`QueueItemKind::AudiobookshelfBook`), merged-timeline mpv projection from multiple `audioFiles`, chapter-row absolute seeking, and book listening-progress synchronization/reconciliation.

### Modified Capabilities
- `service-browse-dispatch`: Gains an internal book/podcast fork for Audiobookshelf tabs, resolved once from `media_type` and never re-checked per action.
- `audiobookshelf-podcast-browsing`: Lifts the "SHALL NOT expose audiobook libraries during this milestone" restriction on Audiobookshelf library discovery; book libraries now also become tabs, handled by `audiobookshelf-book-browsing`.

## Impact

- `src/app/run_loop_drains.rs`: lift the `media_type == "podcast"` filter.
- `src/app/types_tab_selection.rs`, browse dispatch (`input_browse_dispatch.rs`, `audiobookshelf_browse_actions.rs`): book/podcast fork.
- `src/app/layout.rs`: new Music-style hero-on-left composition for the book tab.
- `crates/mbv-core/src/audiobookshelf_catalog.rs`: book catalog fetch, author-surname grouping (mirrors `music_group.rs::build_grouped_album_catalog`).
- `crates/mbv-core/src/playback_queue_items.rs`: new `QueueItemKind::AudiobookshelfBook` / `QueueItemContentId` variant.
- `crates/mbv-core/src/audiobookshelf_playback.rs`, `player_sources.rs`, `ctrl.rs`: book playback-session resolution, merged-timeline mpv projection, and a book-shaped progress event alongside the existing episode-shaped one.
- New dependency: `human_name` crate (Apache-2.0; adds `unicode-normalization`, `unicode-case-mapping`, `unidecode` as net-new build deps, the remaining 5 runtime deps already in the tree).
- `CONTEXT.md` glossary: add audiobook library/book/chapter vocabulary alongside the existing Audiobookshelf podcast entries.
