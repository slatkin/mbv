# Handoff — add-audiobook-support

Worktree: `/home/slatkin/.local/share/opencode/worktree/b2b3f48801e7cfc551acb5ced180d0e1dcac19a3/feat-536-add-audiobook-support`
Branch: `feat/536-add-audiobook-support` (upstream `[gone]` — do not push/rewrite)

## State at pause

Workspace is fully green:

```
rtk cargo check --workspace                 # clean
rtk cargo nextest run -p mbv -p mbv-core    # 1299 passed
rtk cargo clippy --workspace --all-targets  # no issues
rtk make check-code-file-lines              # all governed files ≤ 800 lines
```

Branch is fully updated with `origin/main` (two merges landed, 0 behind). Commits on top of origin/main:

```
dd7ba7bb feat: Music-style two-column book tab composition (task 3.1)
aa4b7727 wip: audiobook support (core backbone + TUI dispatch fork, tasks 1.x-2.x)
```

## Task status

Checked in `tasks.md`: **1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 3.1**.

Remaining (in dependency order):

1. **3.2** — substitute book/chapter/author for album/track/artist. Mostly done in
   the wide hero already (title/author/progress); confirm the right-pane book
   browser mirrors the Music right pane (pills row is N/A — no group pills for
   books; the author-surname grouping is already the browser). Verify against the
   `audiobookshelf-book-browsing` spec substitution table, then check the box.
2. **3.3** — inline `%`/`Finished` progress span in hero meta. **Already
   implemented** in `src/app/render/audiobookshelf_books.rs::render_audiobookshelf_book_hero`
   (Finished / `{pct}%` / "Not started"). Verify the style matches the podcast
   tab's progress span, check the box.
3. **3.4** — chapter rows (or `audioFiles` rows when `chapters[]` empty) in the
   persistent list. **Already implemented** in `render_audiobookshelf_book_rows`
   (Table with marker, truncated title, `fmt_duration_approx` duration, `Part N`
   for audio-files fallback). Verify + check the box.
4. **3.5** — book cover artwork via the Service-scoped, credential-redacted
   path, isolated cache keys. **Already implemented**: `:bookcover:` cache key
   prefix (`AUDIOBOOKSHELF_CACHE_KEY_PREFIX`), `fetch_audiobookshelf_book_cover`,
   shared `fetch_audiobookshelf_image` in `src/app/images.rs`. Verify + check.
5. **4.x / 5.x (mbv-core backbone)** — implemented in the pre-merge core work
   (commit `aa4b7727`): `QueueItemKind::AudiobookshelfBook`,
   `AudiobookshelfBookQueueItem`, book playback endpoint
   (`POST /api/items/{library_item_id}/play`), merged multi-file mpv projection
   (`merge-files=yes` + per-file `loadfile append-play`, resume as absolute seek),
   `AudiobookshelfBookProgressEvent`, book lifecycle/finalization. See the
   "Key decisions" section and prior HANDOFF for the catalog/playback details.
   The checked-in core work already includes 4.1/4.2/4.4/4.6 and
   5.1/5.2/5.3/5.4 **functionally**, but boxes were left unchecked pending the
   honest sweep (see "Marking tasks done").
6. **4.3** — wire play/enqueue for a selected book to `AudiobookshelfBookQueueItem`.
   `play_selected_audiobookshelf_book` / `enqueue_selected_audiobookshelf_book`
   exist in `src/app/audiobookshelf_browse_actions.rs`; verify they construct the
   book queue item (not the episode one) and check the box.
7. **4.5** — chapter-row activation → `PlayerCommand::SeekAbsolute(chapters[].start)`.
   `activate_audiobookshelf_book_row` exists; verify it seeks on the merged
   timeline without reopening the slot, check the box.
8. **5.3 remainder** — `reconcile_audiobookshelf_book_progress` in
   `src/app/lib_event_actions.rs` now updates the book browse-state progress map
   too; verify + check.
9. **6.1** — add audiobook vocabulary to `CONTEXT.md`.
10. **6.2** — tests: filter removal + book tab exposure, author-surname grouping
    (multi-author + parse-failure fallback), book/podcast browse-dispatch
    isolation, merged-timeline chapter seeking, book-vs-episode progress isolation.
11. **6.3–6.6** — the verification commands; run them before marking anything done.

## Key decisions already made (do not re-litigate without cause)

- Books are a **sibling kind** (`AudiobookshelfBook`) to episodes, never
  `episode_id: Option<String>`. Progress/queue/event all keyed by `library_item_id` only.
- Merged timeline uses mpv `merge-files=yes` + per-file `loadfile` (append-play);
  resume is an absolute seek, chapter seeks are absolute seeks on the same timeline.
  **Unverified against a real multi-file book** — see the `ponytail:` comment in
  `player_run_queue.rs::install_active_projection`. If it misbehaves, fall back to
  per-entry offsets or `edl://` (design.md Open Questions) before widening.
- Ctrl capability names: `abs-book-queue`, `abs-book-progress` — additive, no
  `CTRL_PROTOCOL_VERSION` bump.

## What changed during the origin/main merges (important for the next agent)

Two merges landed (commits `538aa852`, `48b5090d`). origin/main had removed the
**legacy Ctrl protocol** (ADR 0020): `CtrlState`, `CtrlEvent::State`,
`WireCommand::LoadFeed`, `split_queue_for_legacy`, `legacy_cursor`,
`supports_feed_playback`, `supports_unified_queue`, and the capable/legacy arms
of `broadcast_state_gated`. The book gating was re-applied onto the simplified
architecture:

- `broadcast_state_gated` now takes **4 unified variants** (`unified_full_json`
  → `(abs_queue, abs_book_queue) = (true, true)`, `unified_abs_json` →
  `(true, false)`, `unified_book_json` → `(false, true)`, `unified_json` →
  `(false, false)`) — `crates/mbv-core/src/daemon_core.rs:599`.
- `unified_queue_state_for_peer(status, queue, source, supports_abs_queue,
  supports_abs_book_queue)` gates ABS episode slots by `abs-queue` and book slots
  by `abs-book-queue` independently — `daemon_control_queue.rs`.
- `reject_command` in `daemon_control_queue.rs` echoes the 2-flag projection.
- `daemon_core_ctrl_spawn.rs` reads 4 hello flags (abs_queue, abs_progress,
  abs_book_queue, abs_book_progress); the init event uses the 2-flag projection.
- `CtrlClients::connect(tx, transport, supports_abs_queue, supports_abs_progress,
  supports_abs_book_queue, supports_abs_book_progress)` — 6 args.
- Feed-playback tests and legacy ctrl tests in `ctrl_tests.rs`,
  `daemon_tests_feed.rs` were dropped (they tested removed types); kept the
  book-progress/abs-book-queue wiring in `daemon_tests_abs_queue.rs`,
  `daemon_tests.rs`.
- `src/app/app_struct.rs`: origin/main split its service-completion methods into
  `src/app/app_audiobookshelf_service_completion.rs` and
  `src/app/app_emby_service_completion.rs`. The feature's
  `audiobookshelf_book_browse.clear()` was re-applied to the new file, and the
  duplicate `start_audiobookshelf_detail` (now living in
  `audiobookshelf_browse_actions.rs`) was removed from the new file.

## Book tab layout (task 3.1, just landed)

`src/app/render/audiobookshelf_books.rs`:
- Wide (`area.width >= TWO_COLUMN_THRESHOLD`, i.e. 82): hero column (2/5 width)
  on the left via `render_audiobookshelf_book_hero`, chapter rows on the right via
  `render_audiobookshelf_book_rows`. Sets `layout.hero_area` / `layout.left_area`.
- Narrow: existing `top_hero_layout` hero-on-top path (unchanged).
- The chapter-selection branch is the only wide consumer today; the
  author-surname book browser (`render_audiobookshelf_book_browser`) still fills
  the full area when no book is selected.

## Marking tasks done

Only check a `tasks.md` box once its observable behavior is wired end-to-end and
verified (compiles + tests + clippy + file-lines). Several remaining boxes
(3.3–3.5, 4.3, 4.5, 5.3) are **functionally implemented already** — verify the
behavior against the spec and check them rather than reimplementing.

## Environment notes

- JCodeMunch index lives at `/home/slatkin/Dev/mbv` (same git object store, main
  checkout — NOT this worktree). Edits must land in this worktree path.
- `rtk` prefixes commands; `rtk cargo nextest run -p <pkg>` is the test runner.
- `rtk make check-code-file-lines` enforces the 800-line file cap.
- No OpenSpec stores registered; `openspec status --change "add-audiobook-support"`
  resolves the change without `--store`.
- Work is pushed to no remote; the branch has no upstream. Do not push/PR without
  an explicit request.
