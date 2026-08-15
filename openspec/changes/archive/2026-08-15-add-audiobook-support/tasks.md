## 1. Lift the library filter and add book catalog fetching

- [x] 1.1 Remove the `media_type == "podcast"` filter at `src/app/run_loop_drains.rs:60`; route `book` libraries to a new book-discovery path instead of dropping them.
- [x] 1.2 Add book catalog types and fetch to `crates/mbv-core/src/audiobookshelf_catalog.rs`: `AudiobookshelfBook` (library_item_id, title, author credit, cover_path, chapters, audio_files), a paginated book-list fetch, and a chapters/audio-files detail fetch, following the existing `AudiobookshelfShow`/`podcast_shows`/`podcast_detail` pattern.
- [x] 1.3 Add `human_name` to `crates/mbv-core/Cargo.toml`; implement surname extraction (`Name::parse(name).map(|n| n.surname().to_owned()).unwrap_or(name)`), first-listed author only for multi-author credits.
- [x] 1.4 Compute `author_display` and `author_sort_key` once at catalog-build time, mirroring `music_group.rs::build_grouped_album_catalog`.
- [x] 1.5 Add a book progress fetch keyed by `library_item_id` only (ABS `episodeId: null` case), distinct from the existing episode-keyed `AudiobookshelfProgress` map.

## 2. Fork browse dispatch by media_type

- [x] 2.1 Resolve `media_type` once at `TabSelection::AudiobookshelfLibrary(usize)` resolution (`src/app/types_tab_selection.rs`); add a book/podcast kind alongside it.
- [x] 2.2 Route book-tab input, rendering, help, refresh, and context-menu dispatch through the resolved kind in `src/app/input_browse_dispatch.rs` and `src/app/audiobookshelf_browse_actions.rs`, without re-reading `media_type` per action.
- [x] 2.3 Update the `service-browse-dispatch` help text and context-menu suppression to include the book kind alongside podcast, Emby, and Feeds.

## 3. Book tab layout

- [x] 3.1 Add the book tab's Music-style two-column composition to `src/app/layout.rs`, reusing the existing `TWO_COLUMN_THRESHOLD` breakpoint and hero-on-left/hero-on-top fallback.
- [x] 3.2 Substitute book/chapter/author for album/track/artist per the substitution table in the `audiobookshelf-book-browsing` spec.
- [x] 3.3 Add the inline progress `%`/`Finished` span to the book hero meta, matching the podcast tab's progress-span style.
- [x] 3.4 Render chapter rows (or `audioFiles` rows when `chapters[]` is empty) in the persistent list area with provider-native identity.
- [x] 3.5 Wire book cover artwork fetch/cache through the existing Service-scoped, credential-redacted artwork path, isolated from podcast cover cache keys.

## 4. Book queue identity and playback

- [x] 4.1 Add `QueueItemKind::AudiobookshelfBook` and a book-shaped `QueueItemContentId` variant (keyed by `library_item_id` only) to `crates/mbv-core/src/playback_queue_items.rs`, alongside the existing episode-shaped `Audiobookshelf` variant.
- [x] 4.2 Add an `AudiobookshelfBookQueueItem` struct mirroring `AudiobookshelfQueueItem`'s presentation/progress/completion fields, minus `episode_id`.
- [x] 4.3 Wire ordinary play/enqueue actions for a selected book to the new queue-item kind.
- [x] 4.4 Implement merged multi-file mpv projection for a book's `audioFiles` (resolve the `loadfile`-with-shared-header vs `edl://` question from design.md's Open Questions against a real multi-file book).
- [x] 4.5 Implement chapter-row activation as one absolute seek to `chapters[].start` on the merged timeline, without stopping or reopening the queue slot/session.
- [x] 4.6 Add the book playback endpoint call (ABS `/api/items/{library_item_id}/play`, no episode segment) to `crates/mbv-core/src/audiobookshelf_playback.rs`, alongside the existing episode play call.

## 5. Book progress synchronization and reconciliation

- [x] 5.1 Add a book-shaped progress-sync request/response path (position, duration, listening time; no `episodeId`) to `player_sources.rs` / `audiobookshelf_playback.rs`, reusing the existing paused-time/seek-distance exclusion logic.
- [x] 5.2 Add a book-shaped `AudiobookshelfBookProgressEvent` to `crates/mbv-core/src/ctrl.rs`, distinct from `AudiobookshelfProgressEvent`, gated by the same capability-negotiation pattern.
- [x] 5.3 Reuse the existing generation-gated apply path to reconcile acknowledged book progress into matching queue and browse state by `library_item_id`.
- [x] 5.4 Finalize book playback sessions on natural completion, stop/skip/queue-replace, and teardown, reusing the existing bounded finalization lifecycle.

## 6. Documentation and verification

- [x] 6.1 Add audiobook library/book/chapter vocabulary to `CONTEXT.md` alongside the existing Audiobookshelf podcast glossary entries.
- [x] 6.2 Add/extend tests for: filter removal and book tab exposure, author-surname grouping (including multi-author and parse-failure fallback), book/podcast browse-dispatch isolation, merged-timeline chapter seeking, and book-vs-episode progress isolation (a progress event for one must not affect the other).
- [x] 6.3 Run `rtk cargo check -p mbv-core` and `rtk cargo check -p mbv` (or the workspace crate names in use).
- [x] 6.4 Run `rtk cargo nextest run -p mbv-core` and the `src/` crate's test suite.
- [x] 6.5 Run `rtk cargo clippy --workspace --all-targets`.
- [x] 6.6 Run `rtk make check-code-file-lines`; split any file that crosses the 800-line cap in the same PR.
