# Proposal: skip-empty-music-folders

## Why

Grouped music views enumerate every folder child of a level, including
folders with no contents. The user's live music library has one such folder
(`Downloads` at the library root); it appears as a browsable group containing
nothing, and the artist warm-up spends a level fill on it that can only come
back empty. A folder with no child items is never a meaningful destination in
a grouped music view.

## What Changes

- Parse the `ChildCount` field the listing request already asks for
  (`get_items_sorted_ranged`, crates/mbv-core/src/api_client_library.rs) into
  `EmbyItem` as `child_count: Option<u32>` (`None` = field absent/unknown).
- In grouped music views (same gate as `is_music_group_view`: music library
  whose configured levels start with `"group"`), do not enumerate folders
  with no child items: `is_folder && child_count == Some(0)` is dropped from
  level listings (`spawn_browse`, `spawn_browse_page`) and from the artist
  warm-up's group-level listing (no fill is spent on an empty level).
- Only explicitly-zero child counts filter. An item whose payload omits
  `ChildCount` (`None`) is kept — a server that does not send the field
  must not blank a level. Non-folder items (tracks) are never filtered, even
  if a payload sends `ChildCount: 0`.

Explicitly **not** in scope: other libraries' browse lists (TV, movies,
Feeds), recursive emptiness (a folder containing only empty folders is still
enumerated), the grouping layer, `music_levels` configuration, and any
server-side query change.

## Capabilities

### Modified Capabilities

- `stable-music-library-grouping`: adds a requirement that grouped music
  levels do not enumerate folders without child items.

## Impact

- `crates/mbv-core/src/api_types.rs` — `EmbyItem.child_count` field.
- `crates/mbv-core/src/...` item parse — `ChildCount` extraction.
- `src/app/library_browse_actions.rs` — empty-folder filter in
  `spawn_browse`/`spawn_browse_page`, gated to grouped music views.
- `src/app/types_browse.rs` + persistence — `BrowseLevel.fetched_rows`
  (server rows consumed; equals `items.len()` when nothing is filtered) so
  page `StartIndex` and pagination exhaustion stay correct once rows are
  dropped; `LibraryPositionLevel` persists it as `Option<usize>` (`None` =
  pre-existing snapshot → `items.len()`).
- `src/app/image_fetch.rs` — same filter in `spawn_music_group_warmup`'s
  group listing.
- Tests: parse unit tests + filtered-listing tests (mock client/JSON, no
  live servers).
- No dependency or config changes.

## Risks

- [Filtering desyncs pagination] → Mitigated by design: page offsets and
  the exhaustion guard read `fetched_rows` (server rows consumed), not the
  filtered row count; unchanged for every unfiltered library.
- [A page whose rows are all empty folders] → Exhaustion is driven by
  `fetched_rows >= total_count`, so a fully filtered page terminates
  pagination instead of refetch-looping.
- [`total_count` still counts dropped folders] → Counts stay server-truth;
  only rows are filtered. Accepted.
