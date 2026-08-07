# Task Brief: Album Hero Sizing and Helper (Tasks 1.1-1.4)

## Project Context

mbv is a terminal UI client for Emby, written in Rust. It embeds mpv for playback.
The music library with levels (group -> album) currently takes a separate rendering path
that bypasses `render_power_list`. This change routes it through `render_power_list` so it
inherits the hero area, two-column layout, and shared list infrastructure.

**These tasks add the album branch to the hero sizing logic**, parallel to the existing
movie and series branches.

## Tasks

### Task 1.1: Add `power_selected_album_item` helper in `detail.rs`

Add a method `power_selected_album_item` to `impl App` in `src/app/render/detail.rs`,
parallel to `power_selected_movie_item` (line 130) and `power_selected_series_item` (line 157).

Returns `Option<mbv_core::api::MediaItem>` when:
- `collection_type == "music"`
- `is_viewing_album_folders(lib_idx)` is true (the album-browsing level)
- `is_music_group_view(lib_idx)` is true (music with levels enabled)

The selected item is `nav_stack.last().items.get(nav_stack.last().cursor)`.

Pattern to follow: see `power_selected_movie_item` at detail.rs:130-155. The music case
is simpler — no feed home-video special case, no `is_folder` check needed (album items
at this level are always albums, not folders).

### Task 1.2: Extract album hero height computation into a standalone function

In `src/app/render/album_plan.rs`, add a standalone function (not a method on App):

```rust
pub(super) fn album_hero_content_rows(
    track_count: usize,
    art_rows: u16,
    panel_width: u16,
    images_enabled: bool,
) -> u16
```

This computes the total content rows for the album hero: the album art height + metadata
rows (title + action hint) + track list rows. Use the same sizing logic as the
`selected_detail_rows` closure already in `build_grouped_album_display_plan`
(album_plan.rs:151-200), but as a standalone function that doesn't need `&self`.

The function should compute:
- If `images_enabled`: `art_rows` (INLINE_ALBUM_ART_ROWS = 15) for the art area
- Track list rows: approximate based on track_count, wrapping at panel_width
  (use a simplified calculation — assume ~60 char wide track names, wrap to panel_width)
- A title row (1) + action hint row (1) for metadata
- Return total content rows (not including HERO_BLOCK_EXTRA_ROWS — the caller adds that)

Keep it simple — this is a sizing estimate, not pixel-perfect. The actual rendering will
fill the allocated space. A reasonable approximation:
- title row: 1
- hint row: 1  
- art rows: if images_enabled, art_rows, else 0
- track rows: track_count (one per track as a rough estimate, since most track names
  fit on one line at reasonable panel widths)

### Task 1.3: Add album branch to hero height computation in `list.rs:191-243`

In `src/app/render/list.rs`, the hero height computation starts at line 191. After the
`selected_series_item` branch (line 212-221), add a new branch for `selected_album_item`:

```rust
} else if let Some(item) = &selected_album_item {
    // Album hero: art + tracks + metadata + block chrome
    let track_count = self.album_tracks_cache.get(&item.id).map(|t| t.len()).unwrap_or(0);
    let art_rows = if self.images_enabled() {
        super::album_art::INLINE_ALBUM_ART_ROWS
    } else {
        0
    };
    let panel_width = content_area.width.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
    super::album_plan::album_hero_content_rows(track_count, art_rows, panel_width, self.images_enabled())
        + HERO_BLOCK_EXTRA_ROWS
}
```

You'll need to compute `selected_album_item` before the hero_rows computation, alongside
`selected_movie_item` and `selected_series_item` which are already computed above (around
line 130-162). Use the new `power_selected_album_item` helper from Task 1.1.

### Task 1.4: Add `"music"` to `top_hero_level` placeholder match

In `list.rs:230-234`, the placeholder match determines when to show a placeholder hero
while content loads. Add `"music"` to the match:

```rust
let top_hero_level = self.libs[lib_idx].nav_stack.len() == 1
    && matches!(
        self.libs[lib_idx].library.collection_type.as_str(),
        "movies" | "homevideos" | "podcasts" | "tvshows" | "music"
    );
```

But wait — for music, the hero should show at the album-browsing level (nav_stack.len() >= 2
when music_group_view is active). The placeholder should show when we're at the album level
of a music library with levels and the album list is still loading. Adjust the condition:

For music with levels, the hero placeholder should appear when `is_music_group_view(lib_idx)`
is true and items are empty/loading. This may need a separate condition from the existing
`top_hero_level` check, since music's hero level is nav_stack.len() >= 2 (not == 1).

Consider adding:
```rust
let music_hero_placeholder = self.is_music_group_view(lib_idx) && items.is_empty();
```

And include it in the placeholder reservation logic.

## Files to Modify

- `src/app/render/detail.rs` — add `power_selected_album_item`
- `src/app/render/album_plan.rs` — add `album_hero_content_rows` function
- `src/app/render/list.rs` — add album branch to hero sizing, add music to placeholder

## Constraints

- Follow existing code patterns and naming conventions
- The `power_selected_album_item` helper must be `pub(crate)` like the other helpers
- The `album_hero_content_rows` function must be `pub(super)` like other functions in album_plan.rs
- Do NOT modify any rendering logic — only add sizing/helper functions
- Run `cargo check -p mbv-core` after making changes to verify compilation
- Commit all changes as a single commit with message: "feat: add album hero sizing helper and height computation"

## Report

Write your report to `/home/slatkin/Dev/mbv/.superpowers/sdd/music-library-hero-and-columns/task-1-report.md`
Include:
- Status: DONE, DONE_WITH_CONCERNS, NEEDS_CONTEXT, or BLOCKED
- What you changed (file paths and summary)
- cargo check output
- Any concerns or deviations from the brief
