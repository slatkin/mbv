## Context

The music-group view (`render_power_music_group_view` in `music.rs`) was built before the two-column layout and hero-on-top changes landed. It routes through its own renderer that calls `render_power_grouped_album_rows` directly, bypassing `render_power_list` and all the machinery it gained: hero area split, two-column packing, letter pills below hero, search box, scroll persistence, prefetch.

The dispatch in `power_widgets.rs:558-577` currently branches:
- `is_album_folders && is_music_group` → `music.rs` (separate path)
- `is_album_folders` (no music group) → `list.rs` (shared path, already has `show_grouped` calling the same `render_power_grouped_album_rows`)

The album's expanded block (art + metadata + track list) currently renders inline below the selected album row via `album_plan.rs`'s `AlbumDetailStart`/`AlbumDetailContinuation` rows. Moving it to the hero means the plan builder stops producing those rows, and the hero sizing/painting logic gets a new album branch.

See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**

- Route music-group view through `render_power_list` so it inherits hero, columns, and all shared list infrastructure.
- Move the selected album's expanded block into the hero panel — same content, new position.
- Two-column packing for grouped album rows, with full-width artist headers.
- Preserve all grouping stability guarantees (per `stable-music-library-grouping` spec).

**Non-Goals:**

- Changing the home video or feed home video group renderers.
- Modifying genre/mood pill behavior.
- Adding hero for non-album music items.

## Decisions

### 1. Remove the music.rs dispatch and let list.rs handle it

The dispatch in `power_widgets.rs` changes: the `is_album_folders && is_music_group` branch falls through to `render_power_list` instead of calling `render_power_music_group_view`. `render_power_music_group_view` in `music.rs` is deleted; `render_power_music_group_pills_row` stays (it's called from `mod.rs` before the library renders).

`render_power_list` already has a `show_grouped` branch (line 470) that calls `render_power_grouped_album_rows`. The music-group view falls into exactly this branch. The existing gates — `is_viewing_album_folders(lib_idx) && search.is_none()` — already match the music-group case.

The loading/organizing states currently handled by `music.rs:97-146` move into `render_power_list`'s empty-items handling, gated on `is_music_group_view`.

### 2. Hero sizing: new album branch alongside movie and series

The hero height computation in `list.rs:191-243` gains a third branch. After checking `selected_movie_item` and `selected_series_item`, it checks `selected_album_item`. A new helper `power_selected_album_item` (parallel to `power_selected_movie_item` in `detail.rs`) returns the selected album when the library is a music library at the album-browsing level.

The album hero height is the album's expanded block height: album art rows + metadata rows + track list rows + block chrome (`HERO_BLOCK_EXTRA_ROWS`). This is computed from the same data `album_plan.rs` currently uses for inline expansion sizing — the album's track count and art dimensions — extracted into a standalone sizing function so the hero can call it without building the full display plan.

### 3. Hero content: reuse render_power_album_detail

The hero content painting (list.rs:515-553) gains an album branch. It calls `render_power_album_detail` with the hero's content rect — the same function currently used for inline track rendering, which already takes `area`, `items`, `cursor`, and layout parameters. The function is already designed for reuse (per its doc comment: "can render either the legacy drilled-in nav_stack level or the inline-album-detail cache with the same code path").

The album art for the hero is rendered via the same `inline_album_art` path, now positioned within the hero rect rather than inline with the list.

### 4. Album plan stops producing inline expansion rows

`build_grouped_album_display_plan` gains a parameter (e.g. `hero_handles_detail: bool`) that suppresses `AlbumDetailStart`, `AlbumDetailContinuation`, `AlbumDetailRule`, `AlbumLoading`, and `AlbumActionHint` rows from the plan. When the hero handles the detail, the plan produces only `Album`, `ArtistHeader`, `ArtistGroupSpacer`, and `AlbumWrappedContinuation` rows.

The `selected_block_bounds` and `track_detail_bounds` fields in the plan become `None` when `hero_handles_detail` is true — the hero draws its own block shell via `hero_block_shell`.

### 5. Two-column packing for grouped album rows

`render_power_grouped_album_rows` accepts a `cols` parameter. Albums within each artist group pack row-major: album `i` within a group occupies column `i % cols`. Artist headers and group spacers span full width and start a fresh row, same as letter headers in `list_letter_groups.rs`.

The display plan (`GroupedAlbumDisplayRow`) gains a multi-item variant (or album rows are grouped at render time, matching how `render_power_plain_rows` handles `DisplayRow::Item` with multiple indices). The simpler approach is to keep `Album(usize)` single-item and batch consecutive `Album` rows into column pairs at render time in `album.rs`, avoiding changes to the plan builder's logic.

`album_cursor.rs` cursor movement adapts to columns: up/down moves by `cols` items within a group, left/right moves by 1. At group boundaries, movement wraps to the nearest item in the adjacent group, same as letter-group cursor movement.

### 6. Music-group pills stay in mod.rs, above the hero

The music-group pills are already rendered in `mod.rs:511-528`, before `render_power_library` is called. They carve rows off the top of `lib_area`, and the remaining area is passed to the library renderer. This stays unchanged — the pills render above the content area that `render_power_list` receives, so they naturally sit above the hero.

The pills and the hero are independent: pills select which genre/mood group to browse, the hero shows the selected album within that group.

### 7. Track focus interaction moves to the hero

`album_track_focus` (the track-selection cursor) continues to work the same way: Enter on the selected album activates track focus, the track cursor moves within the track list, Enter on a track plays it. The only change is that the track list renders in the hero panel instead of inline.

The hero's content rect is stored in `layout.hero_area` (already exists). Input handling (`input_lib_power_keys.rs`) uses this to route track-focus keys to the hero's track list. Mouse clicks within the hero's track area are handled the same as clicks within the former inline track area.

### 8. Artist header selection preserved

Artist headers remain selectable in the music-group view (`selectable_headers` flag). The cursor can sit on an artist header, and actions (play all, queue all) operate on the header's group. This is unchanged — the `ArtistHeader` row type stays in the plan, and the selected-artist-header rendering moves from the inline list to the list area below the hero (it's a list feature, not a hero feature).

## Risks / Trade-offs

- **Album plan refactor scope.** `album_plan.rs` (455 lines) builds a complex display plan with inline expansion, art reservation, track detail bounds, and selected block bounds. Adding `hero_handles_detail` to suppress the expansion rows while keeping the rest correct is the riskiest piece. The plan's test coverage (`tests_album_focus.rs`, `tests_album_listing.rs`) must be extended for the hero case.
- **Cursor movement with columns in grouped view.** The grouped album view has its own cursor movement logic (`album_cursor.rs`, 489 lines) that handles artist-header selection, track focus, and group boundaries. Adding column awareness on top of that is complex. The existing `page_power_grouped_album_cursor` and jump helpers all need column-count parameters.
- **Track list in the hero.** The track list in the hero is fixed-position while the album list scrolls below it. If the track list is long (20+ tracks), the hero may consume most of the content area. The existing hero height cap (leaves at least 1 row for the list) applies, but a capped hero may truncate the track list. The track list within the hero should scroll internally when capped.
- **Visual regression.** The music-group view is heavily used. The transition from inline expansion to hero-on-top changes the spatial relationship between the selected album and its context (neighboring albums). Visual verification at multiple terminal widths is required.
