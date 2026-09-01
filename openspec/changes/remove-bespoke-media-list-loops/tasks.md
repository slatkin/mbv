# Tasks

## 1. Preconditions and inventory

- [ ] 1.1 Confirm clean accepted base `274aa06`, PR #606 feature-branch
  rollback boundary, and dependencies on all four accepted destination slices.
- [ ] 1.2 Inventory exact obsolete `render_*_rows` loops and bespoke painters;
  old selection/scroll/hit geometry; and `AppLayout::main` left/hero/selector/
  wide-family fields. Record ast-grep and grep caller/reference results.
- [ ] 1.3 Confirm #640 is superseded and preserve the Feeds Service versus
  Emby homevideos feed-view distinction; do not edit umbrella artifacts.

## 2. Zero-reference cleanup

- [ ] 2.1 Delete only cross-family bespoke row loops and painters whose exact
  zero-reference gates pass; retain canonical controls and non-hero two-column
  arrangements.
- [ ] 2.2 Delete obsolete selection, scroll, row-hit, and cursor geometry only
  after proving zero consumers; retain component-owned viewport and hit regions.
- [ ] 2.3 Delete `AppLayout::main` left/hero/selector/wide-family fields only
  after proving zero readers, writers, and geometry-dependent callers.
- [ ] 2.4 Preserve Queue fixed-row-only behavior and make no destination-family,
  Service, Player, provider, protocol, persistence, or dependency changes.

## 3. Documentation reconciliation

- [ ] 3.1 Reconcile ADR 0022 with the canonical-list post-slice endpoint and
  cleanup boundary.
- [ ] 3.2 Update `docs/architecture/interactive-surface-ledger.md` to remove
  obsolete loop/geometry-era claims while retaining ownership and verification
  records.
- [ ] 3.3 Update `CONTEXT.md` and frontend guidance only where obsolete bespoke
  list terminology or cleanup-era ownership conflicts; preserve authoritative
  vocabulary and no-exception rules.

## 4. Acceptance evidence and gates

- [ ] 4.1 After cleanup, obtain explicit live user verification at narrow 60x20
  and Wide 120x40/140x30 before modifying UI tests; verify one painter, no
  underpaint, stable geometry, two-column carve-out, Queue fixed rows, and both
  feed surfaces.
- [ ] 4.2 Run ast-grep symbol/caller checks and `rtk grep` zero-reference checks
  for every deleted candidate; attach the inventory and results.
- [ ] 4.3 Run `rtk make check-code-file-lines`, `rtk cargo fmt --all -- --check`,
  `rtk cargo check --workspace --all-targets`, and relevant `rtk cargo nextest
  run` suites.
- [ ] 4.4 Run `rtk openspec validate remove-bespoke-media-list-loops --strict`.
- [ ] 4.5 Confirm changed source files are ≤800 lines, no umbrella checkboxes
  changed, and this cleanup remains independently reversible from PR #606 and
  all four destination slices.
