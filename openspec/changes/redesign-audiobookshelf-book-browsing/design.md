## Context

See proposal.md for motivation. The shipped book tab (`src/app/render/audiobookshelf_books.rs`) has two disjoint render paths gated by `AudiobookshelfBookBrowseState.chapter_selection: Option<usize>`: a full-width `LIBRARY_MAX_COLUMNS`-column grid when `None`, and a Music-shaped hero+chapter-list when `Some`, entered/left via Enter/Esc. The state already exposes `cursor()`/`selected_book()` independent of `chapter_selection`, and `detail_cache`/`detail_loading` already gate chapter/audio-file fetches by `library_item_id` — the data model does not force the modal split, only the render dispatch does.

The reference implementation is Music's wide composition: `render_wide_music_group` (`src/app/render/music_wide.rs`) always renders a left pane (hero + `compute_wide_left_layout`-sized track list, driven by `selected_album_item`) beside a right pane (`render_music_group_pills_row` + `render_wide_right_album_browser`, `src/app/render/music.rs` / `music_wide_browser.rs`), with pane focus derived from `PanelFocus`/`album_track_focus` rather than a modal state transition. Music's pills are one-per-artist and filter via a `nav_stack` push/pop (`music_group_state` reads `nav_stack[nav_stack.len() - 2]`); the artist-filtered view is one level of that stack.

## Goals / Non-Goals

**Goals:**
- Make the book tab's browse-mode render path structurally match Music's persistent two-pane composition, so the hero is always populated and the right-pane browser is always reachable.
- Add alphabetical author-surname-bucket pill filtering using the same filter-drill mechanism Music's artist pills already use, so the interaction model (not just the visual shape) matches.
- Make chapter/audio-file detail fetch eager on cursor movement, closing the gap between "cursor rests on a book" and "hero shows its chapters."

**Non-Goals:**
- Extracting a shared hero+persistent-browser abstraction used by both Music and Books. Two consumers is not yet a strong case for that generalization (ladder rung: reuse existing patterns, don't build a new abstraction for a 2-consumer case); revisit only if a third provider needs the same composition.
- Any change to book playback, queue identity, or progress-sync (`audiobookshelf-book-playback` capability is untouched).
- Socket.IO live progress refresh for books (already out of scope per the original `add-audiobook-support` design).
- Series/narrator/fiction-non-fiction grouping (still deferred per #536 decision 4).

## Decisions

### Mirror Music's render structure directly in `audiobookshelf_books.rs`, don't extract a shared component
Port the same *shape* — persistent hero left (chapter list as its track-list analog) + persistent single-column right-pane browser — into the book renderer, reusing `compute_wide_left_layout`'s pattern and hero-row helpers where book/album types line up, rather than generalizing `music_wide.rs` behind a trait. Alternative considered: extract a shared `render_wide_hero_browser<T>` used by both tabs. Rejected for now — Music's code isn't broken, and generalizing it risks regressing a working tab to fix a broken one. If a third provider needs this composition, extract then.

### Alphabetical surname buckets replace `chapter_selection`'s `Option<usize>` gate with a nav-stack-shaped filter, parallel to Music's `nav_stack`
`AudiobookshelfBookBrowseState` gains a bucket-grouping structure (sibling to `music_grouping.rs::build_grouped_album_catalog`, not shared with it — the grouping unit differs: fixed alphabetical surname ranges, not runs of identical artist) and a selected-bucket index that filters the right-pane list, mirroring Music's `nav_stack[len-2]` read. `chapter_selection` no longer gates which pane renders; it becomes purely "which chapter row is focused within the always-visible left pane," analogous to Music's `album_track_focus`.

Bucket boundaries are fixed ranges (e.g. 8 buckets, A-C through V-Z) computed against the loaded book list; a bucket with zero books in the current library is omitted from the pill row rather than rendered as an empty, unselectable pill — mirroring how Music never shows a pill for an artist with zero albums (the situation cannot arise there, but the omission principle is the same: pills reflect real content).

### Pane focus derives from existing state, not a new mode
Replace the Enter/Esc modal transition in `input_browse_dispatch.rs` with a left/right toggle that flips which of {hero chapter list, right-pane browser} receives cursor input, matching Music's `left_focused`/`right_focused` derivation from `PanelFocus` + a track-focus flag. No new persisted focus state beyond what Music already needed for the analogous toggle.

### Chapter/audio-file detail fetch triggers on cursor movement, gated by the existing cache
The cursor-move handler that currently updates `selected_id` also checks `detail_cache`/`detail_loading` for the newly-selected book and issues the fetch if neither is populated — the same guard shape `render_wide_music_group` already uses for `fetch_album_tracks` (`!self.album_tracks_cache.contains_key(...) && !self.album_tracks_loading.contains(...)`). No new cache or loading-state type; the existing `HashMap`/fetch-gate on `AudiobookshelfBookBrowseState` already has the right shape.

## Risks / Trade-offs

- [Eager chapter/audio-file fetch on every cursor move could over-fetch when a user scrolls quickly through a long book list] → Mitigated the same way Music already handles rapid album-cursor movement: the fetch is gated by `detail_cache`/`detail_loading`, and a fast scroll past a book before its fetch completes does not cancel or retry it, matching existing Music behavior rather than introducing new debounce logic.
- [Alphabetical bucket boundaries (e.g. 8 fixed ranges) may produce uneven bucket sizes for real author-surname distributions (e.g. heavy in `M-O`)] → Accepted for v1, consistent with #536 decision 4 deferring finer grouping; revisit bucket count/boundaries after real usage, not before.
- [Diverging Music and Books render code (rejected shared-abstraction alternative) means a future Music layout change won't automatically propagate to Books] → Accepted per the Non-Goals rationale; the risk is bounded because both tabs' delta specs now explicitly enumerate the substitution table, so a spec-level check (not just code review) can catch drift going forward — the original bug was a spec gap, not just a code gap.

## Migration Plan

No data migration. This changes rendering and input dispatch only; `AudiobookshelfBookBrowseState`'s existing fields (`books`, `selected_id`, `detail_cache`, `progress`) are extended, not replaced. Rollback is a revert of the renderer/input/state changes; no persisted state or wire format changes.
