# Tasks

## 1. Preconditions and inventory

- [ ] 1.1 Confirm the clean accepted starting point and PR `feat/migrate-tui-to-tuirealm` targeting/rollback boundary at implementation time; record accepted SHAs for the canonical foundation and all three sibling destination slices in the implementation issue, not in this plan.
- [ ] 1.2 Confirm dependency order: canonical foundation first; Home/Feeds, Music/Audiobookshelf, and Queue are independent siblings; cleanup waits for all four accepted slices.
- [ ] 1.3 Inventory exact obsolete `render_*_rows` loops and bespoke painters; old selection/scroll/cursor geometry; and `AppLayout::main` left/hero/selector/wide-family fields. Record ast-grep/grep callers, readers, and writers.
- [ ] 1.4 Confirm #640 is superseded; preserve the Feeds Service/tab versus Emby homevideos feed-view distinction. Do not edit umbrella artifacts.

## 2. Zero-reference cleanup

- [ ] 2.1 Prove zero production callers/readers/writers for each obsolete loop/painter, tracking test and docs references separately, then delete only cross-family obsolete loops and painters.
- [ ] 2.2 Prove zero production consumers for obsolete selection, scroll, and cursor geometry, then delete it; retain component-owned viewport geometry and retain all row-hit / `*HitRegion` geometry for `restore-mouse-support` (#638).
- [ ] 2.3 Prove zero production readers/writers/geometry-dependent callers for `AppLayout::main` left/hero/selector/wide-family fields, then delete them.
- [ ] 2.4 Preserve Queue fixed-row-only behavior and make no destination-family, Service, Player, provider, protocol, persistence, dependency, or visual corrections. Route defects to the owning slice.

## 4. Acceptance evidence and gates

- [ ] 4.1 Obtain explicit user live visual verification at narrow 60x20 and Wide 120x40/140x30 before changing UI tests/docs; verify no underpaint, stable geometry, two-column carve-out, Queue fixed rows, and both feed surfaces.
- [ ] 4.2 After approval, reconcile ADR 0022 with final `WideMediaList`/`InlineMediaBrowser` ownership and this downstream cleanup boundary; update `docs/architecture/interactive-surface-ledger.md`, `CONTEXT.md`, and frontend guidance only where obsolete bespoke-list terminology or ownership conflicts, preserving authoritative vocabulary and no-exception rules.
- [ ] 4.3 After approval, update stale UI tests/docs; then run final whole-tree zero-reference checks, with production and test/docs results reported separately.
- [ ] 4.4 Run ast-grep symbol/caller checks and `rtk grep` zero-reference checks for every deleted candidate; attach inventory and results.
- [ ] 4.5 Run `rtk make check-code-file-lines`, `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and relevant `rtk cargo nextest run` suites.
- [ ] 4.6 Run `rtk openspec validate remove-bespoke-media-list-loops --strict` and confirm this change contains only its cleanup/docs/validation scope. Umbrella 4.x/5.x final gates remain in the umbrella; do not mark them here.
