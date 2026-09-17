## Why

Library browse rows currently spend a right-aligned time column on every row and paint their
split-row item title in the now-playing aqua. The user wants the time out of the browsing lists —
the queue and the sessions modal stay the only surfaces carrying a time — and the split-row item
title to read sage.

## What Changes

- Every library list row (Home, Podcast episodes, TV episodes, music tracks, book chapters/parts,
  feed entries) projects no duration; the shared row control keeps its duration slot for the Queue
  list only. No list-row duration string is computed for a library owner anymore.
- The split-row item title (the item's own name after the container/context) paints in a new sage
  (`SPLIT_ROW_TITLE_FG`) role instead of the playback-title aqua (`PLAYBACK_TITLE_FG`). The
  now-playing playback strip keeps its aqua title; only the media-list painter moves.
- Queue list rows keep their total duration and the sessions modal keeps its `pos / dur` time.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `canonical-media-lists`: the duration slot becomes queue-only — a library list row SHALL NOT carry
  a duration — and the split-row secondary title role changes from the playback-title aqua to the
  split-row sage role.
- `audiobookshelf-podcast-browsing`: podcast episode rows no longer carry a duration.
- `audiobookshelf-book-browsing`: book chapter and audio-part rows no longer carry a duration.

## Impact

- `src/app/render/theme/mod.rs` — new `SPLIT_ROW_TITLE_FG` role.
- `src/app/render/components/media_list/row.rs` — split-row secondary colour resolves the new role.
- `src/app/components/home_content.rs`, `podcast_content.rs`, `tv_content/mod.rs`,
  `music_content.rs`, `book_content.rs`, `feeds_content.rs` — drop the projected duration.
- Tests pinning library-row durations or the split-row aqua role.
