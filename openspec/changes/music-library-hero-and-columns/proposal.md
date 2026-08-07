## Why

The music library with levels (group → album) takes a separate rendering path (`render_power_music_group_view` in `music.rs`) that bypasses `render_power_list` entirely. It missed the two-column layout and hero-on-top changes that landed for movies, TV shows, and podcasts. The album's expanded block (art + tracks) currently renders inline below the selected album row; moving it to the hero position at the top brings music in line with the other libraries and frees the album list to use two-column packing.

## What Changes

- Route the music-group view through `render_power_list` instead of the separate `render_power_music_group_view` path, so it inherits the hero area, two-column layout, search box, scroll persistence, and prefetch machinery.
- Move the selected album's expanded block (album art, metadata, track list) from its current inline position into the hero panel at the top of the content area.
- Teach the hero sizing and painting logic (`list.rs`) to handle a selected album item alongside the existing movie and series branches.
- Teach `render_power_grouped_album_rows` to accept a column count and pack album rows two-per-line within each artist group, with artist headers spanning full width — the same pattern letter headers already use in `list_letter_groups.rs`.
- Remove the inline track expansion from the album list (it moves to the hero). The list becomes a compact album browser with artist-group headers.
- Delete `render_power_music_group_view` from `music.rs`; keep `render_power_music_group_pills_row` (the genre/mood pill selector, rendered by `mod.rs` above the library).
- Update the dispatch in `power_widgets.rs` so `is_album_folders && is_music_group` falls through to `render_power_list` instead of routing to `music.rs`.

## Capabilities

### New Capabilities

- `music-library-hero`: The music library's selected album detail (art, metadata, track list) renders in the hero panel at the top of the content area, and the album list supports two-column packing with artist-group headers.

### Modified Capabilities

- `library-list-hero`: The hero area gains a third content branch for selected albums (alongside movies and series), with sizing derived from the album's expanded block height.
- `stable-music-library-grouping`: The grouped album renderer operates within `render_power_list`'s hero + list split rather than its own top-level view, and produces two-column rows. Grouping stability requirements are unchanged.

## Impact

- **Code**: `src/app/render/list.rs` (hero sizing/painting for albums), `src/app/render/album.rs` + `album_plan.rs` (column-aware packing, no inline expansion), `src/app/render/album_cursor.rs` (cursor movement with columns), `src/app/render/music.rs` (delete `render_power_music_group_view`), `src/app/render/power_widgets.rs` (dispatch change), `src/app/render/mod.rs` (music-group pills positioning relative to hero), `src/app/input_lib_power_keys.rs` (cursor deltas for two-column grouped view).
- **Behavior**: Music library with levels gets the same hero-on-top + two-column layout as movies and TV shows. The genre/mood pill selector stays in its current position above the library. Album browsing becomes a compact list; track browsing happens in the hero.
- **Data/API**: None.
- **Risk**: Medium-high. The grouped album renderer (`album.rs`, `album_plan.rs`, `album_cursor.rs` — ~1700 lines) has its own scroll/layout engine that predates the column machinery. Teaching it about columns while removing inline expansion is a significant refactor. The artist-header selection and track-focus interactions need careful re-routing to work within the hero context. Visual verification in a real terminal is required.

## Non-Goals

- Changing the home video list to use `render_power_list` (separate change).
- Changing the feed home video group view.
- Modifying the genre/mood pill selector's behaviour or appearance.
- Adding hero support for non-album music items (individual tracks, artists).
- Changing how `music_levels` configuration works.
