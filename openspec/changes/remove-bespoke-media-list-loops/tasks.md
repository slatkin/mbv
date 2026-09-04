# Tasks

## 1. Preconditions and inventory

- [x] 1.1 Confirm the clean accepted starting point and PR `feat/migrate-tui-to-tuirealm` targeting/rollback boundary at implementation time; record accepted SHAs for the canonical foundation and all three sibling destination slices in the implementation issue, not in this plan.
- [x] 1.2 Confirm dependency order: canonical foundation first; Home/Feeds, Music/Audiobookshelf, and Queue are independent siblings; cleanup waits for all four accepted slices.
- [x] 1.3 Inventory exact obsolete `render_*_rows` loops and bespoke painters; old selection/scroll/cursor geometry; and `AppLayout::main` left/hero/selector/wide-family fields. Record ast-grep/grep callers, readers, and writers.
- [x] 1.4 Confirm #640 is superseded; preserve the Feeds Service/tab versus Emby homevideos feed-view distinction. Do not edit umbrella artifacts.

### 1.3 inventory result (scouts `aa7d4e8`, 2026-09-04, at accepted HEAD `9feeec54`)

Preconditions: accepted HEAD `9feeec54`; canonical foundation + Home/Feeds
(archived slice 3.2) + Music/Audiobookshelf (archived slice 3.3) + Queue
(`migrate-queue-to-canonical-list` Complete) all accepted. #640 superseded by
the Music/Audiobookshelf slice.

Only obsolete symbols with **zero production references** (test-only reachable,
leftovers from the Music slice), all in `src/app/render/components/`:

| Symbol | Definition |
| --- | --- |
| `render_grouped_album_rows` | `album.rs:57` |
| `render_grouped_album_rows_with_ctx` | `album.rs:166` |
| `render_grouped_album_rows_inline_plan` | `album_inline.rs:18` |
| `AlbumRowsCursorCtx` | `album.rs:33` |
| `GroupedAlbumRenderCtx` | `album.rs:40` |

No dead selection/scroll/cursor geometry exists — the canonical migration
reused it rather than orphaning it. No layout field is dead: every
`left_*` / `hero_*` / `selector_*` / `wide_*` field is written by a live
canonical painter and/or read by keyboard-nav or render-geometry code. These
fields live on component-local `LayoutMain` values (owned by `browser`,
`feeds`, and `music_workspace` via `LayoutMain::default()`), not on a global
`AppLayout::main`; there is no global reader.
`left_row_map` and `left_row_targets` are the only fields with any
mouse-specific readers (`browser/mod.rs::click_item_at`,
`tv_workspace::hit_at`, `feeds::handle_mouse_click`); those readers and the
fields' eventual removal are reassigned to `restore-mouse-support` (#638),
which owns hit-region migration and lands after this slice.

## 2. Zero-reference cleanup

- [x] 2.1 Delete the five test-only-reachable album symbols from §1.3 (`render_grouped_album_rows`, `render_grouped_album_rows_with_ctx`, `render_grouped_album_rows_inline_plan`, `AlbumRowsCursorCtx`, `GroupedAlbumRenderCtx`) plus the tests that exist only to exercise them and any `GroupedAlbumDisplayRow` match arms left unreachable. Re-run the ast-grep/grep inventory after deletion to confirm zero references remain.
- [x] 2.2 Prove zero production consumers for obsolete selection, scroll, and cursor geometry. Result: none exists — nothing to delete. Component-owned viewport geometry and all row-hit / `*HitRegion` geometry retained for `restore-mouse-support` (#638).
- [x] 2.3 Prove zero production readers/writers/geometry-dependent callers for the left/hero/selector/wide-family layout fields. Result: every field is still consumed by canonical painters, keyboard nav, or render geometry. Note: these are fields of component-local `LayoutMain` values, not a global `app.layout` — `browser` (`src/app/components/browser/mod.rs:53`), `feeds` (`src/app/components/feeds.rs:58`), and `music_workspace` (`src/app/components/music_workspace.rs:36`) each own one via `LayoutMain::default()`. There is no global `AppLayout::main` reader to remove. The `left_row_map` / `left_row_targets` mouse-reader cleanup on those component-local values is reassigned to #638. Nothing deleted here.
- [x] 2.4 Preserve Queue fixed-row-only behavior and make no destination-family, Service, Player, provider, protocol, persistence, dependency, or visual corrections. Route defects to the owning slice. Result: `git diff --stat bb56c6fb..HEAD` = 17 files, dead-code deletion + orphan-import cleanup only. Per-category verdict all UNCHANGED: Queue, destination-family, Service, Player, provider, protocol (`CTRL_PROTOCOL_VERSION` untouched), persistence, dependencies (no `Cargo.toml`/`Cargo.lock`), visuals (live canonical `WideMediaList`/`InlineMediaBrowser` painters unmodified). No stray changes.

## 4. Evidence, gates, review, and acceptance

- [x] 4.1 Reconcile ADR 0022 with final `WideMediaList`/`InlineMediaBrowser` ownership and this downstream cleanup boundary; update `docs/architecture/interactive-surface-ledger.md`, `CONTEXT.md`, frontend guidance, and stale UI tests/docs only where obsolete bespoke-list terminology, ownership, or references conflict. Preserve authoritative vocabulary and no-exception rules. Result: ADR 0022 Completion section reconciled to the D16 authority bar + Residual A/B/C debt subsection (commit `b610258d`); Residual B tracked as issue #643 (`98ef10ef`). tasks.md §1.3 / task 2.3 wording corrected (`LayoutMain` is component-local, not global `AppLayout::main`). Terminology sweep of ledger / `CONTEXT.md` / `mbv-frontend/SKILL.md` / `ui-design-system` + `canonical-media-lists` specs: nothing stale — every "bespoke" mention is authoritative vocabulary or a no-exception SHALL rule, preserved verbatim. `render_book_browser` / `render_show_rows`: zero hits tree-wide. One historical string `render_album_detail` in a `tests_music_characterization.rs:67` doc-comment narrating a completed transition — left as accurate history.
- [x] 4.2 Run final whole-tree zero-reference checks for the five deleted symbols, reporting production and test/docs results separately; attach ast-grep/grep inventory and results. Result: ZERO production-code and ZERO test-code references to any of the 13 deleted symbols (`render_grouped_album_rows{,_with_ctx,_inline_plan}`, `AlbumRowsCursorCtx`, `GroupedAlbumRenderCtx`, `GroupedAlbumDisplayRow`, `ArtistGroupHeader`, `HeaderFocusCtx`, `GroupedAlbumDisplayPlan{,Ctx}`, `build_grouped_album_display_plan_with_ctx`, `render_album_detail`, `AlbumDetailPresentation`). Non-code hits: this change's own §1.3 inventory; a `tests_music_characterization.rs:67` doc-comment; completed plan `docs/plans/368-split-render-view-files.md` — all historical, none current-state.
- [x] 4.3 Run `make check-code-file-lines`, `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and relevant `cargo nextest run` suites. Result: `cargo fmt --all -- --check` clean; `cargo check --workspace --all-targets` clean (4 pre-existing dead-code warnings, unchanged baseline: `movies_wide_area`, `render_home_video_item`, `has_group_pills` ×2); `cargo nextest run -p mbv` 1280/1280. `make check-code-file-lines` FAILS on `src/app/shell_home.rs` (801 > 800) — pre-existing, introduced by earlier canonical slice `003730d9` in a file this slice never touches; deferred to the umbrella pre-PR file-lines gate per campaign constraint (no mid-refactor splits).
- [x] 4.4 Run `openspec validate remove-bespoke-media-list-loops --strict` and confirm this change contains only its cleanup/docs/validation scope. Umbrella 4.x/5.x final gates remain in the umbrella; do not mark them here. Result: `Change 'remove-bespoke-media-list-loops' is valid`. Scope confirmed cleanup/docs/validation only (see 2.4).
- [ ] 4.5 Review the complete cleanup and evidence, then perform live acceptance at narrow 60x20 and Wide 120x40/140x30; verify no underpaint, stable geometry, two-column carve-out, Queue fixed rows, and both feed surfaces. Route any defect to its owning slice and rerun affected cleanup gates before acceptance.
