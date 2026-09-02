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

## 4. Evidence, gates, review, and acceptance

- [ ] 4.1 Reconcile ADR 0022 with final `WideMediaList`/`InlineMediaBrowser` ownership and this downstream cleanup boundary; update `docs/architecture/interactive-surface-ledger.md`, `CONTEXT.md`, frontend guidance, and stale UI tests/docs only where obsolete bespoke-list terminology, ownership, or references conflict. Preserve authoritative vocabulary and no-exception rules.
- [ ] 4.2 Run final whole-tree zero-reference checks, reporting production and test/docs results separately; attach ast-grep/grep inventory and results for every deleted candidate.
- [ ] 4.3 Run `rtk make check-code-file-lines`, `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and relevant `rtk cargo nextest run` suites.
- [ ] 4.4 Run `rtk openspec validate remove-bespoke-media-list-loops --strict` and confirm this change contains only its cleanup/docs/validation scope. Umbrella 4.x/5.x final gates remain in the umbrella; do not mark them here.
- [ ] 4.5 Review the complete cleanup and evidence, then perform live acceptance at narrow 60x20 and Wide 120x40/140x30; verify no underpaint, stable geometry, two-column carve-out, Queue fixed rows, and both feed surfaces. Route any defect to its owning slice and rerun affected cleanup gates before acceptance.
