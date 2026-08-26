# Orchestrator handoff — 5.3d.10e blocked on a parity decision, then fix landed

Date: 2026-08-26. Worktree `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`, branch `feat/migrate-tui-to-tuirealm`.

## Where things stand (commits, oldest → newest)

- `2e9090e5` — accepted base (5.3d.10c)
- `28c01af2` — 5.3d.10d (podcast shell geometry projection)
- `5ffc3a2c` — correction: strengthen 5.3d.10d shell projection coverage
- `f0f984c3` — **NEW (this session): fix podcast component hero parity with legacy renderer**

## What happened this session

1. The 5.3d.10e writer (hy3) **timed out at 30 min** without committing, leaving entangled dirty state. Its test repoint **inverted three assertions** instead of stopping. All of it was reverted to the clean baseline.

2. Investigation of the timeout's salvage revealed the underpaint deletion (10e) is **not** a pure mechanical no-op — the component renderer diverges from the legacy renderer in two visible ways:
   - **A (show_title swapped):** legacy wide hero passes `show_title=false` (title lives in the right list), narrow passes `show_title=true` (inline detail shows title). The component had it **inverted**: wide=`true`, narrow=`false`.
   - **B (pills/table gate missing):** legacy gates episode filter pills + episode table on `persistent` (wide-only). The component rendered them whenever `episode_selection.is_some()` with no mode gate.

3. Maintainer chose **option 1: fix parity, then 10e is mechanical.** The parity fix is committed as `f0f984c3` (one file, `src/app/render/components/audiobookshelf_podcast.rs`, 19 insertions / 3 deletions):
   - swapped `show_title`: wide passes `false`, narrow passes `true`
   - added `wide: bool` param to `render_podcast_hero`; gates the pills+table block on `wide && episode_selection.is_some()`

   Verified: `cargo check -p mbv` 0 errors; `cargo test -p mbv abs_podcast_component` 5 passed; `cargo nextest run -p mbv -- tests_audiobookshelf_podcasts` 9 passed.

## Next orchestrator must do

**Do NOT implement 5.3d.10e yourself.** Delegate it as a single bounded writer unit. The writer must start from `f0f984c3` and produce exactly one commit, no amend/push. If a worker rows (returns without a commit / tries to delegate), kill it and relaunch; do not fall back to self-implementation.

The 10e contract (already verified correct, ready to hand to a writer):

1. `rm src/app/render/components/audiobookshelf.rs` (legacy renderer; only export is `render_audiobookshelf_podcasts` on `impl App`, all else private)
2. `src/app/render/components/mod.rs`: remove `pub(super) mod audiobookshelf;`
3. `src/app/render/components/widgets.rs` `render_audiobookshelf_library` (~549): replace the Book `if` + trailing `self.render_audiobookshelf_podcasts(...)` call with a `match self.audiobookshelf_kind_at(index)` that sets `audiobookshelf_book_area` (Book) / `audiobookshelf_podcast_area` (Podcast), `_ => {}`; nothing painted; `let _ = (f, focused);`
4. `src/app/render/tests_audiobookshelf_podcasts.rs`: repoint every test from `render_library_to_string_sized` / `render_library_to_terminal_focused` to a component/shell helper.

**Key test-repoint facts the writer needs (verified this session):**

- `Model` is re-exported at `crate::app::Model` (`src/app/mod.rs:126`).
- Shell seams are `pub(super)` and reachable from `crate::app::render::tests_*`: `sync_audiobookshelf_podcast()`, `push_audiobookshelf_podcast_content()`, `render_audiobookshelf_podcast_component(frame)`, field `abs_podcast_id`. `render_audiobookshelf_podcast_component` paints and projects `right_area/list_area/hero_area/inline_hero_area/selected_item_rect/selector_tabs` into `app.layout.main`.
- Component accessors: `geometry() -> &AudiobookshelfPodcastGeometry` and `take_image_paint()` are `pub(in crate::app)` (in `src/app/components/audiobookshelf_podcast.rs`).
- **Two fields are NOT projected into `LayoutMain`** and must be read from component geometry: `layout.audiobookshelf_episode_rows` → `geometry().episode_rows`, `layout.left_item_rows` → `geometry().show_rows` (both `Vec<(Rect, usize)>`). These are the only non-trivial repoints.
- Focus: the component derives `focused` from `app.effective_panel_focus()` (via `push_audiobookshelf_podcast_content` → `set_content(snapshot, focused, images_enabled)`), which reads `app.panel_focus` when `app.terminal_width >= MINI_VIEW_THRESHOLD`, else `app.mini_view_focus`. `audiobookshelf_app()` already sets `panel_focus = Library`. To control `focused` in the helper, set `app.terminal_width` and `app.panel_focus`/`app.mini_view_focus` before `push_audiobookshelf_podcast_content()`.
- Writer gates: `cargo check -p mbv`, focused nextest on `tests_audiobookshelf_podcasts`, full nextest, clippy for touched files, `cargo fmt` on touched files only. **DO NOT reformat `browser.rs`, `hero.rs`, `list.rs`, `tests_library_characterization.rs`** (pre-existing fmt dirt). Skip `make check-code-file-lines` (deferred to 5.6). Do not touch `tasks.md`/handoffs/`.pi`. Stage only named files, never `git add -A`.

## Serial queue after 10e (unchanged)

- Review 10e (one pass; correction commit in parent if findings, no second review)
- 5.3d.11 U0 → U1 → U2 → U3 → U5 → U6 (U4 parent-scoped, exceeds 3 files)
- then TV 18a, Music 19a, Inline 20a (folded rows live in `tasks.md`, unstaged)

## Intentional worktree dirt (do not touch)

Deleted handoff files, `.pi/messenger/feed.jsonl`, untracked `.pi/messenger/crew/`, and my unstaged `tasks.md` edits.
