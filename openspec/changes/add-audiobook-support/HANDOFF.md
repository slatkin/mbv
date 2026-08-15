# Handoff — add-audiobook-support (reboot)

Worktree: `/home/slatkin/.local/share/opencode/worktree/b2b3f48801e7cfc551acb5ced180d0e1dcac19a3/feat-536-add-audiobook-support`
Branch: `feat/536-add-audiobook-support` (upstream `[gone]` — do not push/rewrite)

## State at pause

Workspace is fully green:

```
rtk cargo check --workspace                 # clean
rtk cargo nextest run -p mbv-core           # 534 passed
rtk cargo nextest run -p mbv                # 788 passed
rtk cargo clippy --workspace --all-targets  # no issues
```

All task checkboxes in `tasks.md` are still `- [ ]` — I deliberately leave them
unchecked until the whole change is wired and verified (see "Marking tasks done"
below).

## What is done (mbv-core, the whole playback/progress backbone)

The audiobook **data model + playback + progress** path is complete and tested:

1. **Catalog** (`crates/mbv-core/src/audiobookshelf_catalog.rs`):
   `AudiobookshelfBook`, `AudiobookshelfChapter` (id/start/end/title),
   `AudiobookshelfAudioFile`, `AudiobookshelfBookPage`, `AudiobookshelfBookProgress`
   (keyed by `library_item_id` only). Fetches: `books_bounded`, `book_detail_bounded`,
   `book_progress_bounded`. `audiobook_author_sort_key` uses `human_name` (2.0.4)
   surname, raw-credit fallback; `first_listed_author_sort_key` = first comma-listed
   author only. 797 lines — near the 800 cap; do not grow it further.

2. **Queue identity** (`playback_queue_items.rs`): `QueueItemKind::AudiobookshelfBook`,
   `QueueItemContentId::AudiobookshelfBook { library_item_id }`,
   `AudiobookshelfBookQueueItem` (no `episode_id`), tagged serde round-trip.
   Sibling to the episode `Audiobookshelf` family — deliberately NOT `Option<String>`.

3. **Playback endpoint** (`audiobookshelf_playback.rs`):
   `create_book_playback_session_bounded` → `POST /api/items/{library_item_id}/play`
   (no episode segment). `AudiobookshelfBookPlaybackSession` carries
   `sources: Vec<AudiobookshelfAudioSource>` (one per audio file). Reuses the
   existing `sync_playback_session_bounded` / `close_playback_session_bounded`
   (session sync/close endpoints are shared with episodes).

4. **Merged mpv projection** (`player_sources.rs` + `player_run_queue.rs`):
   `prepare_book_source` opens the book session; `install_active_projection` loads
   the sources with `merge-files=yes`, first `loadfile replace`, rest
   `loadfile append-play`, per-file `http-header-fields` bearer header (direct),
   and applies the resume position as one `seek absolute` on the merged timeline.

5. **Book lifecycle** (`player_reporting.rs`): `AudiobookshelfBookPlaybackLifecycle`
   (sibling to `AudiobookshelfPlaybackLifecycle`) syncs/finalizes the book session
   and emits `AudiobookshelfBookProgressUpdate` (no episode). `PreparedLifecycle`
   enum wraps episode vs book. `ActiveItemLifecycle` gained
   `AudiobookshelfBook`; `observe`/`sync`/`close` handle both.

6. **Ctrl capabilities + events** (`ctrl.rs`, `player_types.rs`,
   `remote_player_connect.rs`, `daemon_core.rs`, `daemon_control_queue.rs`,
   `daemon_control.rs`, `daemon_audiobookshelf.rs`, `daemon_run.rs`,
   `daemon_core_ctrl_spawn.rs`):
   - New caps `abs-book-queue` + `abs-book-progress` (additive, no version bump).
   - `CtrlEvent::AudiobookshelfBookProgress` / `AudiobookshelfBookProgressEvent`
     (no episode) and `PlayerEvent::AudiobookshelfBookProgress`.
   - Queue gating: book slots are filtered by `abs-book-queue`, episodes by
     `abs-queue`, independently. `broadcast_state_gated` now takes 6 variants.
   - Daemon applies book progress via `apply_audiobookshelf_book_progress`
     (matches `as_audiobookshelf_book()` by `library_item_id`), broadcasts gated
     by `abs-book-progress`.

7. **Client reconcile (minimal)** (`src/app/lib_event_actions.rs`,
   `src/app/player_event.rs`, `src/app/render/queue.rs`): the three `src/` files
   that broke on the new variants now compile. `reconcile_audiobookshelf_book_progress`
   reconciles **queue slots only**; book browse-state reconciliation is deferred
   (see remaining work). Book queue rows render title+duration like episodes.

## What remains (the entire TUI fork + browse, in dependency order)

The core is done; the **TUI is not started** beyond keeping it compiling.

1. **1.1** — remove `media_type == "podcast"` filter at
   `src/app/run_loop_drains.rs:60`; route `book` libraries to a book discovery path
   (currently dropped).

2. **2.1** — add a `book`/`podcast` kind resolved once at
   `TabSelection::AudiobookshelfLibrary(usize)` in `src/app/types_tab_selection.rs`;
   carry the resolved kind (do NOT re-read `media_type` per action).

3. **2.2 / 2.3** — fork input, rendering, help, refresh, context-menu dispatch by
   the resolved kind in `src/app/input_browse_dispatch.rs` and
   `src/app/audiobookshelf_browse_actions.rs`; include the book kind in the
   `service-browse-dispatch` help text and context-menu suppression.

4. **3.1–3.5** — book tab layout: reuse the Music two-column hero (`layout.rs`,
   `TWO_COLUMN_THRESHOLD`); substitute book/chapter/author for album/track/artist;
   inline `%`/`Finished` progress span in the hero meta; chapter rows (or
   `audioFiles` rows when `chapters[]` empty) in the persistent list; wire book
   cover artwork through the Service-scoped credential-redacted path with keys
   isolated from podcast covers.

5. **4.3** — wire ordinary play/enqueue for a selected book to
   `AudiobookshelfBookQueueItem` (browse → queue item construction).

6. **4.5** — chapter-row activation → `PlayerCommand::SeekAbsolute(chapter.start)`
   (the seek command + merged timeline already exist; only the TUI row handler is
   missing).

7. **5.3 remainder** — extend `reconcile_audiobookshelf_book_progress`
   (`src/app/lib_event_actions.rs`) to update the book browse-state progress map
   once the book tab/browse state exists (mirror the episode
   `audiobookshelf_browse.progress` update; also carry `current_time_seconds`).

8. **6.1** — add audiobook library/book/chapter vocabulary to `CONTEXT.md`.

9. **6.2** — tests: filter removal + book tab exposure; author-surname grouping
   (multi-author + parse-failure fallback); book/podcast browse-dispatch isolation;
   merged-timeline chapter seeking; book-vs-episode progress isolation (a progress
   event for one must not touch the other).

10. **6.6** — `rtk make check-code-file-lines` (new TUI files must stay ≤800).

## Key decisions already made (do not re-litigate without cause)

- Books are a **sibling kind** (`AudiobookshelfBook`) to episodes, never
  `episode_id: Option<String>`. Progress/queue/event all keyed by `library_item_id` only.
- Merged timeline uses mpv `merge-files=yes` + per-file `loadfile` (append-play);
  resume is an absolute seek, chapter seeks are absolute seeks on the same timeline.
  **Unverified against a real multi-file book** — see the `ponytail:` comment in
  `player_run_queue.rs::install_active_projection`. If it misbehaves, fall back to
  per-entry offsets or `edl://` (design.md Open Questions) before widening.
- `AudiobookshelfBookLifecycle`/`AudiobookshelfBookProgressUpdate` are deliberate
  near-duplicates of the episode versions (design decision: distinct identity).
- Ctrl capability names: `abs-book-queue`, `abs-book-progress` — additive, no
  `CTRL_PROTOCOL_VERSION` bump.

## Marking tasks done

Only check a `tasks.md` box once its observable behavior is wired end-to-end and
verified (compiles + tests + clippy + file-lines). Currently everything is
unchecked; the catalog/playback/progress boxes (1.2–1.5, 4.1/4.2/4.4/4.6,
5.1/5.2/5.3/5.4) are functionally complete in mbv-core and can be checked after
the remaining verification pass, but I left them for one honest sweep.

## Environment notes

- JCodeMunch index lives at `/home/slatkin/Dev/mbv` (same git object store, main
  checkout — NOT this worktree). Edits must land in this worktree path.
- `rtk` prefixes commands; `rtk cargo nextest run -p <pkg>` is the test runner.
- No OpenSpec stores registered; `openspec status --change "add-audiobook-support"`
  resolves the change without `--store`.
