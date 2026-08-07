# Task Brief: Album Hero Content Painting (Tasks 2.1-2.3)

## Project Context

mbv is a Rust TUI client for Emby. The music library is being routed through `render_power_list`
to inherit the hero area, two-column layout, and shared list infrastructure. Batch 1 already added
the album branch to the hero sizing logic. This batch adds the actual content painting.

## Tasks

### Task 2.1: Add album branch to hero content painting in `list.rs`

In `src/app/render/list.rs`, after the hero content painting for movies and series (around line 571-589),
add an album branch. The existing pattern is:

```rust
if selected_movie_item.is_some() {
    self.render_power_compact_detail(f, content_rect, lib_idx, focused, cols > 1, layout);
} else if selected_series_item.is_some() {
    self.render_series_inline_detail(f, content_rect, lib_idx, focused, cols > 1, layout);
}
// ADD HERE:
} else if let Some(item) = &selected_album_item {
    // Render album detail in the hero
}
```

The album hero should:
1. Get the album's tracks from `self.album_tracks_cache.get(&item.id)`
2. If tracks are available, call `self.render_power_album_detail(f, content_rect, tracks, track_cursor, focused, true, true, false, true, art_reserved_w, None, layout)`
3. If tracks are not loaded yet, ensure they're being fetched (call `self.fetch_album_tracks(item.id.clone())`) and render a loading placeholder
4. Render the album art via `self.render_inline_album_art(f, art_rect, item, layout)` in the right portion of content_rect

Parameters for `render_power_album_detail`:
- `show_title: true` — the hero doesn't have an Album(idx) row above showing the title
- `selected_region_gutter: true` — we're in the hero block context
- `flush_left: false`
- `show_hint: true` — show the action hint ("^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks")
- `art_reserved_w`: `INLINE_ALBUM_ART_RESERVED` if images enabled and content_rect is wide enough, else 0
- `active_marker_x: None`
- `track_cursor`: `self.libs[lib_idx].album_track_focus.unwrap_or(0)`

For the album art rect, position it on the right side of content_rect:
```rust
let art_reserved_w = if self.images_enabled() && content_rect.width >= INLINE_ALBUM_ART_RESERVED + 20 {
    INLINE_ALBUM_ART_RESERVED
} else {
    0
};
if art_reserved_w > 0 {
    let art_rect = Rect {
        x: content_rect.x + content_rect.width.saturating_sub(art_reserved_w),
        y: content_rect.y,
        width: art_reserved_w,
        height: content_rect.height,
    };
    self.render_inline_album_art(f, art_rect, item, layout);
}
```

You'll need to import `INLINE_ALBUM_ART_RESERVED` from `super::album_art`.

The `selected_album_item` variable is already computed earlier in the function (Batch 1 added it
around line 160). It's available in scope at the hero painting location.

### Task 2.2: Render album art in the hero

This is part of Task 2.1 — the album art rendering is integrated into the album hero branch.
See the `render_inline_album_art` call above. The function is defined in `album_art.rs:78`.

### Task 2.3: Track focus interaction verification

This is a manual verification task — it requires running the app in a real terminal and testing:
- Enter on the selected album activates track cursor
- Track navigation moves within the hero's track list
- Escape exits track focus

The agent should NOT attempt to implement this. Just note it requires manual testing.

However, verify that the existing track focus logic in `input_lib_power_keys.rs` already handles
this correctly — the `album_track_focus` field and its key handling should work unchanged since
the track list is rendered in the same `render_power_album_detail` function. Just confirm the
code paths exist and are reachable.

## Files to Modify

- `src/app/render/list.rs` — add album branch to hero content painting (after line ~589)

## Constraints

- Follow the existing pattern for movie/series hero content painting
- `render_power_album_detail` is defined in `album_detail.rs` — read it to understand parameters
- `render_inline_album_art` is defined in `album_art.rs` — read it to understand parameters
- `INLINE_ALBUM_ART_RESERVED` and `INLINE_ALBUM_ART_ROWS` are in `album_art.rs`
- `album_tracks_cache` is a field on `App` — `HashMap<String, Vec<MediaItem>>`
- `fetch_album_tracks` is a method on `App`
- Do NOT modify `render_power_album_detail` or `render_inline_album_art` themselves
- Run `cargo check -p mbv-core` after changes
- Commit with message: "feat: render album detail and art in the hero panel"

## Report

Write your report to `.superpowers/sdd/music-library-hero-and-columns/task-2-report.md`
Include:
- Status: DONE, DONE_WITH_CONCERNS, NEEDS_CONTEXT, or BLOCKED
- What you changed
- cargo check output
- Any concerns or deviations
