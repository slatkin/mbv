## 1. Alphabetical author-surname bucket grouping

- [x] 1.1 Add a bucket-grouping function in `types_audiobookshelf_browse.rs` (sibling to `music_grouping.rs::build_grouped_album_catalog`, not shared): fixed alphabetical surname ranges (e.g. A-C through V-Z) computed over the loaded, surname-sorted book list; omit a range from the result when it has zero books.
- [x] 1.2 Add a `nav_stack`-shaped selected-bucket field to `AudiobookshelfBookBrowseState`, parallel to Music's `nav_stack[len-2]` read, so the right-pane list can be filtered to one bucket.
- [x] 1.3 Recompute buckets when the book list refreshes/pages in, preserving the selected bucket and selected book per the existing "refresh retains selection" behavior.

## 2. Persistent two-pane render composition

- [x] 2.1 Replace `render_audiobookshelf_books`'s `chapter_selection`-gated dispatch (`audiobookshelf_books.rs:40-92`) with a single persistent composition: hero-on-left (always populated from `cursor()`/`selected_book()`) beside browser-on-right, at the same `TWO_COLUMN_THRESHOLD` breakpoint and hero-on-top fallback Music uses.
- [x] 2.2 Add the right-pane pill row (bucket labels) and single-column filtered book list, reusing `render_pill_bar`/`PillBar` per Music's `render_music_group_pills_row` shape.
- [x] 2.3 Keep chapter rows in the left pane below the hero (the persistent list, Music's track-list analog), rendered from `render_audiobookshelf_book_rows`'s existing row logic.
- [x] 2.4 Verify narrow-terminal fallback (hero-on-top) still renders both the hero+chapters and the bucket-filtered browser, not just the hero.

## 3. Focus and input dispatch

- [x] 3.1 Replace the Enter/Esc modal transition in `input_browse_dispatch.rs` (lines ~264-306) with a left/right arrow pane-focus toggle between the hero's chapter list and the right-pane book browser, matching Music's `left_focused`/`right_focused` derivation.
- [x] 3.2 Update up/down navigation so it moves the cursor within whichever pane is focused (chapters vs. book browser), without hiding the unfocused pane.
- [x] 3.3 Update pill-selection input (choosing a different bucket) to filter the right-pane list via the nav-stack-shaped state from task 1.2.
- [x] 3.4 Update mouse dispatch (`input_mouse_dispatch.rs`) for the new two-pane hit-testing, replacing the `chapter_selection.is_some()` branch.
- [x] 3.5 Update context-menu/help text dispatch that referenced the old modal `chapter_selection` gate.

## 4. Eager chapter/audio-file detail fetch

- [x] 4.1 Trigger chapter/audio-file detail fetch when the browser cursor moves onto a book, gated by the existing `detail_cache`/`detail_loading` (mirroring `fetch_album_tracks`'s `!contains_key(...) && !loading.contains(...)` guard).
- [x] 4.2 Confirm a fetch in flight or already cached is not re-requested on rapid cursor movement (no new debounce logic; rely on the existing cache/loading-set guard).

## 5. Spec-drift safeguard and documentation

- [x] 5.1 Confirm the merged `audiobookshelf-book-browsing` spec's substitution table (from this change's delta) explicitly names the right-pane persistent browser and pill-bucket grouping, so a future implementer can't drop them the way the shipped version did.
- [x] 5.2 Update `CONTEXT.md` if any new vocabulary is introduced (e.g. "surname bucket").

## 6. Tests and verification

- [x] 6.1 Add/extend tests for: bucket computation (including empty-bucket omission and boundary cases), bucket-filter selection narrowing the right-pane list, hero tracking the cursor without an open action, left/right focus toggle leaving both panes visible, and eager detail fetch on cursor movement (including the no-refetch-when-cached/loading case).
- [x] 6.2 Run `rtk cargo check -p mbv` (and `-p mbv-core` if touched).
- [x] 6.3 Run `rtk cargo nextest run -p mbv` (and `-p mbv-core` if touched).
- [x] 6.4 Run `rtk cargo clippy --workspace --all-targets`.
- [x] 6.5 Run `rtk make check-code-file-lines`; split `audiobookshelf_books.rs` or any file that crosses the 800-line cap in the same PR.
