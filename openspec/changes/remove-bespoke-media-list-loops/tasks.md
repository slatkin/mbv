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
reused it rather than orphaning it. No `AppLayout::main` field is dead: every
`left_*` / `hero_*` / `selector_*` / `wide_*` field is written by a live
canonical painter and/or read by keyboard-nav or render-geometry code.
`left_row_map` and `left_row_targets` are the only fields with any
mouse-specific readers (`browser/mod.rs::click_item_at`,
`tv_workspace::hit_at`, `feeds::handle_mouse_click`); those readers and the
fields' eventual removal are reassigned to `restore-mouse-support` (#638),
which owns hit-region migration and lands after this slice.

## 2. Zero-reference cleanup

- [ ] 2.1 Delete the five test-only-reachable album symbols from §1.3 (`render_grouped_album_rows`, `render_grouped_album_rows_with_ctx`, `render_grouped_album_rows_inline_plan`, `AlbumRowsCursorCtx`, `GroupedAlbumRenderCtx`) plus the tests that exist only to exercise them and any `GroupedAlbumDisplayRow` match arms left unreachable. Re-run the ast-grep/grep inventory after deletion to confirm zero references remain.
- [x] 2.2 Prove zero production consumers for obsolete selection, scroll, and cursor geometry. Result: none exists — nothing to delete. Component-owned viewport geometry and all row-hit / `*HitRegion` geometry retained for `restore-mouse-support` (#638).
- [x] 2.3 Prove zero production readers/writers/geometry-dependent callers for `AppLayout::main` left/hero/selector/wide-family fields. Result: every field is still consumed by canonical painters, keyboard nav, or render geometry. `left_row_map` / `left_row_targets` mouse-reader cleanup reassigned to #638. Nothing deleted here.
- [ ] 2.4 Preserve Queue fixed-row-only behavior and make no destination-family, Service, Player, provider, protocol, persistence, dependency, or visual corrections. Route defects to the owning slice.

## 4. Evidence, gates, review, and acceptance

- [ ] 4.1 Reconcile ADR 0022 with final `WideMediaList`/`InlineMediaBrowser` ownership and this downstream cleanup boundary; update `docs/architecture/interactive-surface-ledger.md`, `CONTEXT.md`, frontend guidance, and stale UI tests/docs only where obsolete bespoke-list terminology, ownership, or references conflict. Preserve authoritative vocabulary and no-exception rules.
- [ ] 4.2 Run final whole-tree zero-reference checks for the five deleted symbols, reporting production and test/docs results separately; attach ast-grep/grep inventory and results.
- [ ] 4.3 Run `make check-code-file-lines`, `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and relevant `cargo nextest run` suites.
- [ ] 4.4 Run `openspec validate remove-bespoke-media-list-loops --strict` and confirm this change contains only its cleanup/docs/validation scope. Umbrella 4.x/5.x final gates remain in the umbrella; do not mark them here.
- [ ] 4.5 Review the complete cleanup and evidence, then perform live acceptance at narrow 60x20 and Wide 120x40/140x30; verify no underpaint, stable geometry, two-column carve-out, Queue fixed rows, and both feed surfaces. Route any defect to its owning slice and rerun affected cleanup gates before acceptance.
