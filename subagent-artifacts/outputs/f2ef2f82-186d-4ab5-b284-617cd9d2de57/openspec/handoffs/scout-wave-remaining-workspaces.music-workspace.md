# Scout — 5.3d Music workspace family (ledger rows 67 + 87)

Read-only banked discovery from HEAD `2c6bcce5`. Music workspace only; Emby
browser/TV/ABS untouched. Nested row 5.3d's Music scope = the `sync_music_workspace`
per-surface mirror plus its component (`MusicWorkspaceComponent`), owned state,
legacy renderer branch, and shared `BrowseLevel` coupling.

## State: fully interactive; mirror still live

The two ledger rows (67 "Grouped Music workspace", 87 "Inline album-track
interaction") are both `component`. Inline track focus is fully component-owned
(`track_cursor`) — the *Album track focus* unit landed (`6b2977d4`). What remains
for the 5.3d Music family is the **per-frame `sync_music_workspace` mirror** and
its associated framework teardown: App state readers/writers, legacy renderer
branch, raw key forwarding path, and shared-cursor coupling. This is a
`sync_home`-style move (event-driven push + legacy renderer branch removal).

## Files & key symbols

### Mirror + shell (mirror driver)
- `src/app/shell_music_workspace.rs` (entire file) — the mirror and its seam.
  - `Model::sync_music_workspace` (49-98): per-frame. Mounts/umounts
    `MusicWorkspaceComponent` on `music_workspace_id`; through `set_content`
    (wide render ctx), `set_album_columns`, `set_page_rows`, consumes the
    one-shot `music_track_focus_request`, `set_inline_track_focus_enabled(wide)`.
  - `Model::music_workspace_component_id` (30-44): gates on `EmbyLibrary` tab,
    collection_type=="music", `is_music_group_view`, `is_viewing_album_folders`.
  - `Model::focused_music_track` (12-28): resolves track target from the
    component's local `track_cursor()` + `app.album_tracks_cache`.
  - `Model::render_music_workspace_component` (100-122): render seam. Requires
    `layout.main.wide_music_area` (set only by the **legacy** renderer — see the
    renderer seam hazard below); early-returns when 0-size (narrow).
- `src/app/shell.rs`:
  - Per-frame call site `self.sync_music_workspace()` at **960** (in the
    "Apply App-owned effect handoffs to mounted components" block).
  - Draw-closure call `self.render_music_workspace_component(f)` at **1014**.
  - One-shot writer `self.music_track_focus_request = Some(true/false)` at
    **303**/310 (from `LibEvent::RecursiveAlbumActivated`/`RestoreLibraryPosition`).
  - Shell routing arms for component intents at **548-604**:
    `MusicAlbumCursor { target, kind }` → `move_music_group_display_cursor` /
    `jump_music_group_display_cursor` / `page_grouped_album_cursor`;
    `MusicTrackActivate` → `play_album_track`; `MusicTrackEnqueue` →
    `enqueue_lib_item`; `MusicTrackContextMenu` → `open_context_menu_for`.
  - `Model::music_track_focus_request: Option<bool>` field at **69** (101 init).

### Component + local interaction state
- `src/app/components/music_workspace.rs` (425 lines).
  - Component-owned state: `album_cursor`, `album_scroll`, `track_cursor`
    (Option), `album_columns`, `page_rows`, `inline_track_focus_enabled`,
    `last_album_id`, `last_mirrored_cursor/scroll`, `layout`, `image_paint`.
  - `set_content` (98-142): the mirror's push. Has internal "reconcile if not
    changed" logic against `last_mirrored_cursor/scroll` — this is the mirror
    contract that will die with `sync_music_workspace`.
  - `view()` (389-402): paints via `render_wide_music_group_with_ctx`, copies
    result into `self.layout`, captures `image_paint`. Repaints OVER the legacy
    underpaint (wide double-draw).
  - Raw key forwarding: `handle_key` (208-350). Emits typed
    `ShellRequest::{MusicTrackActivate, MusicTrackEnqueue, MusicTrackContextMenu,
    MusicAlbumCursor}`; **all other keys fall through** as
    `Msg::Legacy(LegacyTerminalEvent::Key(to_crossterm_key_event(key)))`.
    Enter-on-album when narrow = NoOp fall-through (opens selection modal legacy).
  - Mouse: `handle_mouse` (354-375) — always forwards
    `Msg::Legacy(LegacyTerminalEvent::Mouse(...))`; mutates local
    album/track_cursor from `self.layout.wide_music_track_at` /
    `wide_music_browser_area` hit-test first.
  - `enter_track_focus`/`clear_track_focus`/`set_inline_track_focus_enabled`
    (shell-driven, consume the one-shot request).

### App state readers/writers (Background-side; the "App field" side)
- Album cursor canonical owner is still the legacy BrowseLevel cursor
  (`libs[j].nav_stack.last().cursor`); the component's `album_cursor` is only a
  display-order mirror.
  - Setters (shared with BrowseLevel): `src/app/render/screens/album_cursor.rs`
    `move_music_group_display_cursor` (4), `jump_music_group_display_cursor`
    (27), `page_grouped_album_cursor` (48). All three write
    `nav_stack.last().cursor` — shared `BrowseLevel` mutation.
  - `src/app/render/components/music_wide.rs` `App::wide_music_render_ctx`
    (110-174): builds the render context; reads `nav_stack` cursor, group
    catalog `music_grouping`, `album_tracks_cache`,
    `current_library_columns`, `effective_panel_focus`.
  - `src/app/lib_cursor_actions.rs`: `is_viewing_album_folders` (287),
    `is_music_group_view` (music_actions.rs 8). These gates decide whether the
    component is mounted at all (wide AND narrow).

### Legacy renderer dependency (PROBLEM: last-mile geometry)
- `src/app/render/screens/root.rs` `render_library` (EmbyLibrary arm, ~413-420)
  → `render_list` at `components/widgets.rs:543`.
- `src/app/render/components/list.rs` `render_list` wide-music branch
  **66-95**: when `is_music_group_view && is_viewing_album_folders &&
  shared_hero_presentation(area).is_some()`, it (a) trigger `fetch_album_tracks`
  for the unfetched album, (b) builds `wide_music_render_ctx`, (c) calls
  `render_wide_music_group_with_ctx(f, area, &ctx, layout)` — which
  **sets `layout.wide_music_area = area`** (music_wide.rs:182) and mutates
  `layout.left_area`, `hero_area`, `wide_music_right_area`, `left_row_targets`.
  Then returns early.
- **This list.rs music branch is both the legacy painter AND the only writer of
  `layout.main.wide_music_area` early in the frame.** The component
  (`render_music_workspace_component`, shell.rs:1014) reads the SAME
  `wide_music_area` to know where to draw. So today wide mode double-paints
  (legacy underpaint then component overpaint) and **the shell needs the legacy
  branch to run first to know the geometry**. Deleting the legacy branch with
  the mirror requires the shell/componenTo compute area itself at the render
  seam (the `sync_inline_search` pattern lapplyed).

### Tests to adapt
- `src/app/input_music_track_navigation_tests.rs` — drives
  `model.sync_music_workspace()` (lines 30/156/178) and asserts component-local
  + legacy forwarding semantics. Differential `grouped_music_cursor_routing_...`
  (shell_music_workspace.rs test) compares legacy vs component — DELETE with the
  legacy second path.
- `src/app/shell_music_workspace.rs tests (target file)` — shell mount/sync,
  narrow, wide, recursive-activation, position-restore tests all call
  `sync_music_workspace`; must switch to whatever `push_*` replaces it.
- `src/app/render/components/list_late_tests.rs` / `list_tests.rs` — legacy
  `render_list` wide-music assertions; adapt/remove when the branch is removed.

## Smallest safe implementation units (dependency order)

Each unit ≤ 3-6 production files, committed green (per the verification policy in
tasks.md §5.3d).

- **Unit 1 — prep: bounded routing at the push seams (mirror-neutral).**
  Add `Model::push_music_workspace_content()` that dispatches to the 
  component's `set_content`/`set_album_columns`/`set_page_rows`/focus-enable at
  the real writers (mount change, resize, cursor-setter routing in shell.rs,
  RecursiveAlbumActivated/RestoreLibraryPosition). `sync_music_workspace` still
  runs, but only for the mount/lifecycle; tests adapt to called-back. Files:
  `shell_music_workspace.rs`, `shell.rs`, tests.
- **Unit 2 — delete the per-frame mirror.** Remove `sync_music_workspace` from
  the per-frame block (shell.rs:960); delete the `last_mirrored_cursor/scroll`
  reconcile in `set_content`. Move mount/lifecycle out of the mirror into
  explicit key seams. Files: `shell.rs`, `shell_music_workspace.rs`,
  `music_workspace.rs`.
- **Unit 3 — render-seam geometry (parse-line hard}).** Make
  `render_music_workspace_component` compute `wide_music_area` at render time
  (from layout/arrangement `wide_library_panes`), not read a legacy-written
  value. Delete the legacy wide-music branch in `list.rs` (with its 
  `render_wide_music_group_with_ctx` call) and `App::wide_music_render_ctx`
  only if the component's own `view` per-shell context. Files:
  `shell_music_workspace.rs`, `list.rs`, `music_wide.rs`.
- **Unit 4 — App/BrowseLevel field teardown tent.** Delete/Home `fields:
  component-local album/track cursor already owned; App fields remaining after
  Units 1-3 are likely `music_track_focus_request` (already Model),
  `session-active` no. Verify precedence with `ast-grep`, then delete legacy
  `pages_legacy` if redundant. (No `App.music_*` cursor field remains — the
  album cursor was NEVER re-homed; it still lives as `BrowseLevel.cursor`.

## Blockers / decisions
- **Blocker 1 (geometry chicken-and-egg):** the shell's component render
  requires `layout.main.wide_music_area`, which only the legacy `render_list`
  music branch writes (music_wide.rs:182). The legacy branch CANNOT be deleted
  in the mirror unit alone; Unit 3 must strand the placement to the shell/C
  seam BEFORE the branch deletion. This mirrors the `sync_inline_search`
  "(component paints the view-time rect, self.area removed)".
- **Decision 1:** Narrow mode stays fully legacy (component mounted, 0-area
  early-return). Confirm "no ledger edits for 2a" — the ledger rows 67/87 stay
  `component` until the framework teardown lands (per sync_home precedent ts
  the ledger Home row moved back to `component`).
- **Decision 2:** The legacy `Enter`-on-album narrow path
  (`handle_key_emby_library` → `activate_album_folder_row`) stays — it is the
  non-component narrow path, unchanged by this family.
- **Decision 3:** `BrowseLevel.cursor` remains the truth for album selection;
  the component's `album_cursor` is a read-only display mirror. Any re-home of
  the Album cursor into the component (full BrowseLevel ownership move) is NOT
  part of row 5.3d's drop-dead Music family intent — the framework teardown
  removes `handle_key`/`CONTEXT_STACK`, but the shared cursor coupling to
  `BrowseLevel` must be preserved; do not re-home `album_cursor` in music (that
  would decouple the shared cursor contract with the Emby browser).
- **Scope guard:** none of Emby `browser.rs`/TV/ABS mirror/byaudited. Do not
  pull those in.

## Tests to adapt
1. `shell_music_workspace.rs` test block (in-file 8 tests) — re-pent beyond
   mirrors to push seams.
2. `input_music_track_navigation_tests.rs` — 3 `sync_music_workspace()` calls ->
   push seams; legacy-forwarding assertions may be `rtk`-deleted once framework
   teardown lands, not before.
3. Differential `grouped_music_cursor_routing_matches_legacy*` — they have to
   DEAD (two-path differencing; delete with legacy path).

## Ready first-unit prompt (Unit 1)
"Add `Model::push_music_workspace_state()` in `shell_music_workspace.rs` that
applies content/columns/page-rows/focus-enable to the mounted
`MusicWorkspaceComponent` from the App writers (album-cursor setter, resize,
`RecursiveAlbumActivated`/`RestoreLibraryPosition`), replacing the parts of
`sync_music_workspace` that only re-apply state each frame. Keep
`sync_music_workspace` for mount/lifecycle only. Update the 3 test call sites
to drive push instead. Gate: `rtk cargo checks p mbv`, `other tests
`rtk cargo nextest run -p mbv music_workspace album_track`, full `mbv` suite,
`rtk ast-grep scan`, `rtk make check-code-moon-lines`. Do NOT delete the legacy
`render_list` wide-music branch (that is Unit 3; the shell + component still
double-paint during Unit 1-2)."

## Severity / risks
- High: the wide `render_list` branch is the SOLE source of `wide_music_area`
  geometry for the frame; premature deletion breaks the component's render
  seam. Sequence Units 3 before deleting branch.
- Medium: component's `album_cursor` mirror vs `BrowseLevel.cursor` truth —
  enforce no divergence (all Album snapshots go through setters).
- Low: narrow double-mode is intentional; keep explicit unfocused narrow default
  (options D14).

## Residual risks
- The per-frame mirror push means component paint reads `layout` rects set by
  the legacy frame earlier in the same terminal cyclization — a real
  coupling.
- Tests drive `sync_music_workspace` in 5 places; mechanical churn.

> Reference: `openspec/.../scoping-5.3d-mirrors.md`; `openspec/.../tasks.md` §5.3d;
> ledgers 67/87 in `dashboard` of the docs; `Album cursor prep` + `Album track
> focus` units landed.