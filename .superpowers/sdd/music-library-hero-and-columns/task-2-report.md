# Task 2 Report: Album Hero Content Painting (2.1-2.3)

## Status

DONE

## What I changed

### `src/app/render/list.rs`

- Added `use super::album_art::INLINE_ALBUM_ART_RESERVED;` (line 1).
- Added the album branch to the hero content painting, after the series branch
  (around line 590). When `selected_album_item` is present:
  - Computed `art_reserved_w` = `INLINE_ALBUM_ART_RESERVED` when images are
    enabled and `content_rect.width >= INLINE_ALBUM_ART_RESERVED + 20`, else 0
    (same gate as `album.rs`).
  - If tracks are cached (`album_tracks_cache.get(&item.id).cloned()`, cloned
    so the `&mut self` call to `render_power_album_detail` doesn't fight the
    borrow), calls
    `render_power_album_detail(f, content_rect, &tracks, track_cursor, focused,
    true, true, false, true, art_reserved_w, None, layout)` — `show_title`
    true (no Album(idx) row above in the hero), `selected_region_gutter` true
    (hero block context), `flush_left` false, `show_hint` true.
  - If tracks are not loaded yet, calls `fetch_album_tracks(item.id.clone())`
    and paints a ` Loading…` placeholder via `render_power_placeholder`, width
    reduced by `art_reserved_w` so it never overlaps the art.
  - When `art_reserved_w > 0`, builds `art_rect` right-aligned within
    `content_rect` and calls `render_inline_album_art(f, art_rect, item,
    layout)` (which also sets `layout.inline_image_rect`).
- `track_cursor` comes from `self.libs[lib_idx].album_track_focus.unwrap_or(0)`.

### `openspec/changes/music-library-hero-and-columns/tasks.md`

- Marked 2.1 and 2.2 as `[x]`. 2.3 left unchecked — manual verification only.

## cargo check output

Both the mbv-core library and the `mbv` binary package compile:

```
$ rtk cargo check -p mbv-core
cargo check: 0 errors, 1 warnings (0 crates)

$ rtk cargo check -p mbv
cargo check: 0 errors, 1 warnings (1 crates)
```

The single warning (`method is_dismissed is never used`, in
`crates/mbv-core/src/player_run_state.rs:152`) is pre-existing and unrelated.

`rtk cargo clippy -p mbv` is also clean (0 errors, same pre-existing warning).

Note: the brief said to run `cargo check -p mbv-core`, but `src/app/render/list.rs`
lives in the `mbv` binary package, not `mbv-core`, so `-p mbv-core` alone would
not compile the change. I ran both.

## Task 2.3 (manual verification)

Not implemented, as instructed. I confirmed the code paths exist and are
reachable:

- **Enter** on the selected album → `activate_album_folder_row`
  (`src/app/actions.rs:532`) sets `album_track_focus = Some(0)`; Enter is
  routed through `input_queue_keys.rs:182-187`.
- **Up/Down** while `album_track_focus.is_some()` moves the cursor within the
  cached tracks (`input_queue_keys.rs:194-210`), and
  `active_power_album_track_lib_idx` (`input_lib_power_keys.rs:8`) is what the
  input resolver keys off for track mode.
- **Escape/Backspace** clears `album_track_focus` (`input_queue_keys.rs:188-193`).

The hero renders through the same `render_power_album_detail` function as the
inline block, so the cursor/highlight logic applies unchanged.

## Concerns / deviations

1. **`focused` parameter**: per the brief I passed the hero's panel-focus
   `focused` to `render_power_album_detail`. The doc comment at
   `album_detail.rs:82-84` says `focused` "mirrors `album_track_focus.is_some()`
   at the call site" (the `album.rs` inline call passes
   `album_track_focus.is_some()`). With panel `focused`, the hint may keep
   reading "ENTER: Show tracks" instead of flipping to "BACK: Exit" once track
   mode is entered. This is exactly what task 2.3 should verify manually; if it
   looks wrong, swap the 5th argument to
   `self.libs[lib_idx].album_track_focus.is_some()`.
2. The brief's task 2.3 text references `input_lib_power_keys.rs`, but the
   Enter/Escape/Up/Down track-mode handling actually lives in
   `input_queue_keys.rs` (confirmed above).
3. Kept a ` Loading…` placeholder (not in the brief verbatim) so the hero has
   something visible while tracks fetch; art still renders on the right.
