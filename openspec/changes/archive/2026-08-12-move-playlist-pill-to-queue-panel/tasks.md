# Tasks: Move playlist pill to queue panel

## Implementation

- [x] Render `playlist_status_spans()` at the bottom row of the left queue panel in `src/app/render/mod.rs`, left-aligned with 2-col left padding (matching the queue panel's inner padding), against the panel background
- [x] Remove `show_playlist_pill` parameter from `render_status_bar()` and stop rendering `playlist_status_spans()` in the status bar (`src/app/render/chrome_status.rs`)
- [x] Update all `render_status_bar()` call sites to remove the `show_playlist_pill` argument
- [x] Update `src/app/render/tests_queue.rs::power_queue_title_does_not_render_playlist_pill` — expect playlist pill absent from queue header row
- [x] Update `src/app/render/tests_queue.rs::bottom_status_bar_shows_playlist_pill_when_queue_is_a_playlist` — expect playlist pill absent from bottom-right row, present in left panel bottom row
- [x] Update `src/app/tests_status_bar.rs` tests that assert playlist glyph presence in the last line
- [x] Run `cargo test` and fix any failures
