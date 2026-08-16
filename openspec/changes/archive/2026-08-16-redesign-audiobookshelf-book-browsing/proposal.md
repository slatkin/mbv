## Why

#536 (milestone 6) specified that the Audiobookshelf book tab reuse the Music tab's wide composition — a persistent hero-on-left tracking the browse cursor, paired with a persistent single-column browser-on-right — but what shipped (`f7cd2887`) is a different, TV/Emby-grid idiom instead: a full-width two-column book grid with no hero, that only reveals a hero + chapter list after an explicit Enter, replacing the grid rather than sitting beside it. The merged `audiobookshelf-book-browsing` spec's substitution table never named the right-pane browser as a required element, so this gap shipped despite an open PR comment flagging the substitution-table verification as incomplete. Author-surname alphabetical pill grouping (decision 4 in #536) was dropped entirely rather than implemented. This change corrects the shipped composition to match Music's actual behavior, closing the ambiguity in the spec so it can't regress again.

## What Changes

- Replace the book tab's modal grid-then-detail-drilldown with Music's persistent two-pane composition: hero-on-left (always shows the browse cursor's book, cover, author, inline progress, and chapter list) beside a persistent single-column browser-on-right, matching Music at the same `TWO_COLUMN_THRESHOLD` breakpoint and hero-on-top narrow fallback. **BREAKING** (UX): removes the current Enter-to-enter/Esc-to-leave book tab navigation model.
- Add alphabetical author-surname pill grouping (A-C, D-F, ...) to the book tab's right pane, using the same pill-bar rendering Music uses for its artist groups. Selecting a pill filters the right-pane list to that bucket via the same nav-stack-shaped filter drill Music's artist pills use, not a scroll/jump within a flat list.
- Change pane-focus interaction to match Music exactly: left/right arrow toggles focus between the hero's chapter list and the right-pane book browser; both panes remain visible and rendered at all times (no pane replaces the other).
- Fetch a book's chapter/audio-file detail eagerly when the browse cursor moves onto it (mirroring Music's eager `fetch_album_tracks` behavior), rather than only on an explicit book-open action, so the hero's chapter list is populated as soon as a book is highlighted.
- Correct the `audiobookshelf-book-browsing` spec's Music-tab-composition substitution table to explicitly name the right-pane persistent browser and its alphabetical-bucket pill grouping, closing the ambiguity that let the shipped implementation diverge from #536's decisions.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `audiobookshelf-book-browsing`: replaces the "Book libraries use the Music tab composition" requirement's substitution table and layout description with the full persistent two-pane hero+browser composition (including alphabetical-bucket pill filtering and left/right focus semantics); extends the "Books load incrementally, grouped and sorted by author surname" requirement with alphabetical-bucket grouping and pill-filter behavior; extends the "Chapters render as first-class rows in the persistent list" requirement so chapter/audio-file detail is fetched eagerly on cursor navigation rather than only on explicit book selection.

## Impact

- `src/app/render/audiobookshelf_books.rs`: replace the `chapter_selection`-gated modal dispatch with a persistent two-pane render, reusing `music_wide.rs`/`music_wide_browser.rs` structural patterns (`compute_wide_left_layout`, hero-row helpers, `render_wide_right_album_browser` shape) without extracting shared code across tabs.
- `src/app/types_audiobookshelf_browse.rs`: `AudiobookshelfBookBrowseState` gains alphabetical-bucket grouping/filter state (nav-stack-shaped, parallel to Music's `nav_stack`) alongside the existing `cursor()`/`selected_book()`/`detail_cache`.
- `src/app/audiobookshelf_browse_actions.rs` / `src/app/input_browse_dispatch.rs`: replace Enter/Esc modal transitions with left/right focus-toggle input handling; trigger chapter/audio-file detail fetch on cursor movement instead of only on book-open.
- `src/app/music_grouping.rs`: no changes to Music's own grouping; the new book-bucket grouping function is a sibling, not a shared abstraction (two consumers isn't yet a strong case for extraction).
- `openspec/specs/audiobookshelf-book-browsing/spec.md`: substitution table and two requirements corrected per the delta spec.
- No change to `audiobookshelf-book-playback`: play/enqueue/seek actions and queue identity are unaffected by this browsing-composition correction.
