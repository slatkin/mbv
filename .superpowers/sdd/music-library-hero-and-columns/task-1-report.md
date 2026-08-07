# Task 1 Report: Album Hero Sizing and Helper

## Status

DONE

## What I changed

### `src/app/render/detail.rs`
Added `App::power_selected_album_item` (`pub(crate)`), parallel to
`power_selected_movie_item`/`power_selected_series_item`. Returns the
cursor-selected `MediaItem` only when the library is `"music"`,
`is_viewing_album_folders(lib_idx)` holds, and `is_music_group_view(lib_idx)`
holds. No `is_folder` check (album items at this level are always albums) and
no feed-home-video special case, per the brief. Task 1.1.

### `src/app/render/album_plan.rs`
Added standalone `pub(super) fn album_hero_content_rows(track_count, art_rows,
panel_width, images_enabled) -> u16`. Computes the album hero's content rows:
title row (1) + action-hint row (1) + album art rows (when images enabled) +
track rows. Track rows assume ~60-char names wrapping at `panel_width`
(`track_count * 60 div_ceil panel_width`), which degenerates to one row per
track at reasonable panel widths, per the brief. Does not include
`HERO_BLOCK_EXTRA_ROWS` — the caller adds that. Task 1.2.

### `src/app/render/list.rs`
- Computed `selected_album_item` alongside `selected_movie_item` and
  `selected_series_item` (chain of `is_none()`), using the new helper. Task 1.3.
- Added the `selected_album_item` branch to the hero-height computation after
  the series branch: `album_tracks_cache` track count, `INLINE_ALBUM_ART_ROWS`
  when images are enabled, `panel_width` from `content_area.width` minus
  `2 * SELECTED_BLOCK_SIDE_PADDING`, then
  `album_hero_content_rows(...) + HERO_BLOCK_EXTRA_ROWS`. Task 1.3.
- Placeholder logic (Task 1.4): added `"music"` to the `top_hero_level`
  `matches!`, and added `music_hero_placeholder` for the music-with-levels case
  (hero lives at `nav_stack.len() >= 2`, not `== 1`). Since the later `items`
  binding isn't available at the hero-computation point, emptiness is checked
  directly against `lib.nav_stack.last().items.is_empty()`.

## Verification

- `rtk cargo check -p mbv-core` — 0 errors, 1 pre-existing warning
  (`is_dismissed` never used in `crates/mbv-core/src/player_run_state.rs:152`,
  confirmed present before my changes via stash).
- `rtk cargo check -p mbv` — 0 errors (my changes live in the `mbv` binary
  crate, so this is the crate that actually exercises them).
- `rtk cargo clippy --workspace --all-targets` — 0 errors, 1 pre-existing
  warning (same `is_dismissed`).
- `rtk make check-code-file-lines` — all governed files at or below 800 lines.

## Concerns / deviations

- `music_hero_placeholder` uses `nav_stack.last().items.is_empty()` instead of
  the brief's `items.is_empty()` because `items` is gathered later in
  `render_power_list` (after the hero-row computation). Semantically equivalent:
  the placeholder branch only runs when `selected_album_item` is `None`, so the
  emptiness check distinguishes "music group view with a list still loading"
  from any future non-loading state.
- Track-row wrapping uses `div_ceil` over a fixed 60-char assumption; this is a
  sizing estimate only and matches the brief's "one per track" outcome at
  widths >= 60 cols.
