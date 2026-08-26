# Scout — 5.3d Emby browser family (`sync_emby_browser`) from HEAD 2c6bcce5

Emby browser family only. TV/Music/ABS untouched. Scope anchor:
`scoping-5.3d-mirrors.md` lists `sync_emby_browser` as the **last remaining
per-frame surface mirror** for this family (under "Not yet landed").

## Files retrieved
- `src/app/shell_browser.rs` (1-177) — mirror + mount gate + render seam; tests (182-766).
- `src/app/components/browser.rs` (26-640) — `BrowserComponent` field/interaction state.
- `src/app/shell.rs` (470-560, 738-808, 940-1020) — Legacy bridge, Browser Msg routing, per-frame sync, draw closure.
- `src/app/render/components/list.rs` (55-175), `movies_wide.rs` (86,140), `list_context.rs` (5-30) — legacy renderer + `library_list_render_ctx`.
- `src/app/input_lib_keys.rs` (93-246), `input_browse_dispatch.rs` (40-155) — legacy key path.
- `src/app/layout.rs` (186-202), `panel_focus_state.rs` (24) — `is_wide_movies_active`, `effective_panel_focus`.
- `src/app/components/msg.rs` (359-532) — `ShellRequest::Browser*`, `BrowserHitRegion`.
- `docs/architecture/interactive-surface-ledger.md` (64) — Emby generic/Movies row = `component`.

## Key code
Mirror (`shell_browser.rs:132-167`), the only per-frame surface mirror left in this family. Production caller is **only** `shell.rs:958` (event loop, every frame). It does three jobs in one call:
1. **Mount lifecycle** `132-144`: reconcile `emby_browser_id` against `emby_browser_component_id()` (`110-129`, gate = active `TabSelection::EmbyLibrary` + `is_podcast_library`/`is_feed_home_video_group_view` + `BrowserKind::Generic|Movies|HomeVideos`); mount/unmount/`active`.
2. **Content projection** `152-162`: `set_content(library_list_render_ctx(index,false), focused)`.
3. **Layout-derived adapter** `162-165`: `set_wide_movies(App::layout.main.is_wide_movies_active())`.

App fields the mirror reads: `app.tab`, `app.libs` (via `library_list_render_ctx` → `nav_stack.last()` items/cursor/scroll/total/letter_filter/loading, `list_context.rs:5-30`), `effective_panel_focus()` (`panel_focus_state.rs:24`), `is_wide_movies_active()` (`layout.rs:193` — derived from `movies_wide_right_area` set by the **legacy wide renderer** each draw).

`BrowserComponent` owns: local `cursor/scroll`; `set_content` mirror-pins only when unchanged (`browser.rs:62-84`, `last_mirrored_cursor`/`scroll`/`initialized`) — the classic Home-style mirror-pinning. `wide_movies` field (`browser.rs:44`) feeds `columns()` (`browser.rs:321-331`). Keyboard effects already type-routed: `handle_crossterm_key` (`browser.rs:100`) emits `ShellRequest::Browser{MoveRows,MoveColumn,JumpCursor,Activate,Play,Enqueue,ToggleWatched,ContextMenu,Shuffle,Refresh,Rescan,Back,CycleLetterPill}`; shell routes to `handle_browser_request` (`shell_browser.rs:17-108`). Raw fallthrough `!self.focused` and unclaimed keys → `Msg::Legacy` (`browser.rs:295`); mouse same (`browser.rs:548`). Legacy bridge → `handle_key_with_home_context` → `CONTEXT_STACK` → `handle_key_emby_library` (`input_browse_dispatch.rs:86`) → `handle_lib_key` (`input_lib_keys.rs:93`).

Renderer is double-painted: legacy base frame `render_list` (`list.rs:55`, generic/wide-movies branches `list.rs:105-172`) paints AND the component overdraws at `rendering_seam` in draw closure (`shell.rs:1012`); component `view` paints via `render_generic_movies_home_video_rows_with_ctx` (`browser.rs:601-612`) at `app.layout.main.left_area`. Legacy underpaint still sets `movies_wide_right_area` + notification-independent layout the mirror reads next frame.

## Start here
Open `src/app/shell_browser.rs:132` (`sync_emby_browser`) and the two commits that deleted `sync_home` (`7c1168a2` prep, `d2b24d0c` typed effects) — this is the exact template (phase-1 prep → push at writers, App-field re-home scheduled separately). Then `browser.rs:62-165`.

## Smallest safe implementation units (dependency order)

### Mirror removal (this row is a surface mirror; deletion is the deliverable, anything beyond is separate)
- **M1 — Mount lifecycle break-out** `:132-144`. Extract mount/umount/active into event-driven `mount_emby_browser()` (called at tab/library/mount-gate change), finishing when `emby_browser_id` reconciliation no longer rides on the per-frame sync. ~2 files, mechanical.
- **M2 — content/cursor/focus push.** `:147-162`: replace per-frame `set_content` with idempotent `push_emby_browser_content()` driven at **writers** (nav `select`/`go_back`, fetch/`apply_drain`/search completion, `refresh`, next-page, letter-filter, panel-focus change, resize). Smallest green unit: add the push + call seams, keep sync_emby_browser for set_content only as the fallback (or delete sync in the same unit once writer coverage lands). ~3-6 production files (shell_browser, shell, input_browse_dispatch, context/render seams).
- **M3 — `wide_movies` decision (unique to Emby, Home didn't have a layout-derived field).** `browser.rs:162-165` rides on `App::layout.main` computed by the legacy base frame. Two fans: (a) keep a per-draw temporary adapter set in the draw closure (like `dim_backdrop_active`, `shell.rs:999`), or (b) move the "wide-Movies/type × width" derivation into the component. **Blocking M2 green.** Requires a decision (see blockers).

### Interaction/state/renderer/framework teardown (separate, NOT the mirror)
- **I1 — claim remaining generic-browser `Msg::Legacy` fallbacks.** `browser.rs:295/550`: 1-col Left/Right/h/l unbound + Ctrl/Alt-modified keys. Remove `Msg::Legacy` from `BrowserComponent` for the generic family so the raw legacy bridge (`handle_lib_key`) no longer mutates `nav_stack.cursor` outside the typed request. Enables I2.
- **I2 — delete `set_content` mirror-pinning** (`browser.rs:62-84`: `last_mirrored_cursor`/`scroll`/`initialized`); push item/content only, component fully owns cursor. Depends on I1 (cursor can't drift via legacy). ~3 files.
- **R1 — legacy underpaint removal.** Stop base-frame `render_list` painting generic/Movies/home-video rows (`list.rs:137-172`, `movies_wide.rs`); component sole painter. Needs M3 (wide_movies derivation relocated) + `left_area` still provided. ~4 files + render tests.
- **F1 — framework teardown** (deferred under 5.3d umbrella, not Emby-specific): remove generic/movies branches from `handle_key_emby_library`/`handle_lib_key`, then CONTEXT_STACK/LegacyInput removal.

## Blockers / decisions
- **B1 (blocking M2/M3/R1): `wide_movies` per-render coupling.** `is_wide_movies_active()` needs the legacy wide renderer to have populated `movies_wide_right_area` that same frame. A pure event/push move can't know it ahead of draw. Season: keep a tiny per-draw adapter, or compute "wide" from library type + window width. Needs supervisor decision; smallest deferral = option (a).
- **B2: writer enumeration for `push_emby_browser_content` is large** (nav-stack mutation surface). Must confirm every cursor/content writer for the active generic library is covered before declaring the mirror green — otherwise the double-paint/cursor-drift test set leaves regressions. This is larger than Home because Home's writers were already consolidated at `push_home_content`.
- **B3:** `library_list_render_ctx(index, false)` second arg `_display_recursive_albums` is always `false` here; can be dropped from the `Option` when the renderer consumers allow.

## Tests to adapt
- `src/app/shell_browser.rs` tests (all in `mod tests` 182-766, currently assert the mirror): `shell_emby_browser_effects_honor_component_target` (210), `shell_mounts_and_syncs_the_generic_emby_browser` (384), `shell_emby_browser_movement_drives_app_cursor_via_typed_requests` (431). Each calls `model.sync_emby_browser()`; adapt to drive `post_mount`/`push_*`. `render_browser_model` (737) and `drive_browser_key` (510) helpers stay. M1/M2: replace `sync_` calls with mount + push; I1 breaks the one-col "falls through to `Msg::Legacy`" assertions (689-705).
- Component has no unit tests inside `browser.rs` (only `test_layout` 588). Verify command: `rtk cargo nextest run -p mbv emby_browser` + `rtk cargo check -p mbv`.

## Ready first-unit prompt (M1)
Pull the mount/unmount/<active> from M1's `sync_emby_browser` into an idempotent `mount_emby_browser()` invoked at tab/library/gate changes; `sync_emby_browser` delegates to it then keeps `set_content`+`set_wide_movies` (per-frame content still lands in the same commit). Mechanical; no behavior change; green via existing `shell_web_browser` mount tests + full `nextest run -p mbv`.
## M1 landed — 2026-08-26 (5.3d.15/M1, commit `8929248`)

Extracted the mount lifecycle out of `sync_emby_browser` into a new idempotent
`mount_emby_browser()` (in `src/app/shell_browser.rs`); `sync_emby_browser` now
delegates to it first, then keeps the per-frame `set_content` + `set_wide_movies`
exactly as before. Pure mechanical extraction, zero behavior change; single-file
commit `8929248c474e8bf4955cf14947a1022e96dcaa90`.

Gates: fmt/check(0 errors)/focused `nextest run -p mbv emby_browser` (3 passed)/
full `nextest run -p mbv` (1156 passed)/clippy(0 errors, no new warnings in
file)/ast-grep(0 findings on `shell_browser.rs` at HEAD and current, all rules)/
`git diff --check` clean. Planning dirt left uncommitted.

Next: **M2** (`push_emby_browser_content` at proven writers — nav
select/go_back, fetch/apply_drain/search-completion, refresh, next-page,
letter-filter, panel-focus change, resize), AND apply D18 step 1 (move
`set_wide_movies` out of `sync_emby_browser` into `render_emby_browser_component`
as a per-draw adapter) within 5.3d.15. Keep the D17 stage-1/stage-2 sequence.

## M2 landed — 2026-08-26 (5.3d.15/M2, commit `24e645b9`)

Split content projection from mount: `push_emby_browser_content()` is now
event-driven, called at 12 writer sites in `shell.rs` (every `push_home_content`
seam + `handle_browser_request` + `BrowserClick`). `set_wide_movies` moved to
`render_emby_browser_component` as a D18 per-draw adapter (after legacy base
frame populates `movies_wide_right_area`). `sync_emby_browser` reduced to
mount+push delegation for test compatibility.

Gates: check(0 errors)/focused `nextest run -p mbv emby_browser` (3 passed)/
full `nextest run -p mbv` (1156 passed)/clippy(0 errors, no new warnings in
touched files)/ast-grep(0 findings on touched files)/`git diff --check` clean.

Next: **5.3d.16** — claim remaining Emby browser raw-key fallthrough, remove
cursor/scroll mirror-pinning, make component authoritative.

## 5.3d.16 landed — 2026-08-26 (commits `6fa217fb` + fix `4e1e6f67`)

Claimed raw-key/mouse fallthrough (all input now `NoOp`), removed cursor/scroll
mirror-pinning fields, made component cursor authoritative. Reviewer flagged
P1 regression (external cursor jumps broken), fixed by restoring cursor/scroll
sync from context in `set_content`.

Gates: check(0 errors)/focused `nextest run -p mbv emby_browser` (3 passed)/
full `nextest run -p mbv` (1156 passed)/clippy(0 errors)/ast-grep(0 findings).

Next: **5.3d.17** — remove Emby generic/Movies/home-video legacy underpaint
after the wide-layout dependency is detached, then remove its empty sync/mount
adapter.

## 5.3d.17a landed — 2026-08-26 (commits `578ecff0` + fix `e94a9fb5`)

Component now handles wide Movies/home-video layout itself:
- `BrowserComponent::view()` checks `shared_hero_presentation(area)` and paints
  hero card on left, pills on right, list rows in right pane when wide
- Shell passes full `movies_wide_area` (not just `left_area`) when wide
- Component exports `image_paint` for shell to paint via `App::paint_home_image`
- Added fields: `wide_movies_home_video`, `wide_movies_letter_pills`,
  `use_nerd_fonts`, `image_paint`
- P1 fix: reset `image_paint = None` in narrow branch to prevent stale hero
  images after wide→narrow resize

Reviewer verdict: ACCEPT with notes.

**5.3d.17b handoff notes:**
- Scroll write-back is still delegated to legacy renderer (`level.scroll = final_scroll`
  in `render_wide_movies_with_ctx`). 17b must move this into component path or shell.
- `movies_wide_area`/`movies_wide_right_area` population is a 17b dependency. Shell reads
  `layout.main.movies_wide_area` for hand-off area and `is_wide_movies_active()` (gated on
  `movies_wide_right_area`) for the `wide` flag. Both are currently set only inside
  `render_wide_movies_with_ctx`. When 17b removes that renderer it MUST preserve setting
  these two fields elsewhere.
- `self.layout.movies_wide_right_area = right_area` is written but never read inside the
  component. Verify it's actually needed before 17b.

Gates: check(0 errors)/focused `nextest run -p mbv emby_browser` (3 passed)/
full `nextest run -p mbv` (1156 passed)/clippy(0 errors)/ast-grep(0 findings).

Next: **5.3d.17b** — remove legacy underpaint (`render_wide_movies_with_ctx` and the
wide Movies/home-video branch in `render_list`), preserve scroll write-back and
`movies_wide_area`/`movies_wide_right_area` population.
