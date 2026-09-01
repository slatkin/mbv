## 1. Preconditions and characterization

- [ ] 1.1 At implementation start, record the accepted canonical-foundation merge SHA; confirm PR #606 feature-branch stacking and canonical foundation availability, and record the distinct Queue rollback boundary.
- [ ] 1.2 Characterize existing Queue rendering and interactions before UI test edits using only source trace, existing unchanged evidence, and manual observation with metadata-bearing Local and Remote fixtures: title, focus, active progress, reorder affordances, scope controls, and remote state; make no UI test changes and do not use test-driven appearance.
- [ ] 1.3 Characterize supported Wide/Normal and narrow/mini geometry, scrolling, selection, active row, and one-painter execution path; identify the legacy Queue body painter to remove or bypass.

## 2. Canonical Queue composition

- [ ] 2.1 Define the Queue prepared projection in the canonical `Item` vocabulary with stable `QueueSlotId` targets, title/metadata, semantic state, and `progress_percent` clamped to `0..=100`; keep domain/effect authority outside the child.
- [ ] 2.2 Embed the canonical fixed-row child in the mounted Queue parent and route local cursor, scroll, viewport, scrollbar, and fixed-row placement through it; preserve refresh target retention and clamping without an App mirror.
- [ ] 2.3 Preserve Queue parent/shell ownership of Local/Remote scope and controls, reorder, playback, title, Player/queue authority, persistence, and active-state policy; carry stable `QueueSlotId` in every slot-targeted effect, allowing a destination position only for reorder and resolving it against the same canonical queue.
- [ ] 2.4 Remove Queue's duplicate list painter, movement/scroll arithmetic, and row coordinate path while retaining parent-owned scope/chrome geometry. Do not introduce InlineMediaBrowser, Hero-on-left, Inline hero, responsive Wide/Inline handoff, Feeds, or Audiobookshelf work.

## 3. Mouse seam and visual correction

- [ ] 3.1 Mount the Queue mouse subscription and private `MouseGestureState`; delegate painted row points to child-owned `HitRegions<QueueSlotId>` and translate results to semantic requests. Keep scope controls parent-owned and do not restore restore-mouse-support's global map or duplicate coordinate path.
- [ ] 3.2 Perform visual correction at supported Wide/Normal and narrow/mini widths, including metadata, active progress, Local/Remote scope, reorder, remote state, focus, and scrolling; obtain explicit user live confirmation before test changes.
- [ ] 3.3 Record one-painter source/execution evidence for every reachable Queue breakpoint and confirm the child rect is non-empty and matches its painted hit regions.

## 4. Tests and gates (after visual approval)

- [ ] 4.1 After explicit user live approval, add/update focused render and geometry tests with metadata-, active-progress-, focus-, scope-, reorder-, remote-, refresh-, and target/scroll-bearing fixtures; cover progress bounds and target retention.
- [ ] 4.2 Verify mouse click resolution, scope-control precedence, focus-following, movement stride, and absence of shell coordinate re-resolution through the real mounted composition where applicable.
- [ ] 4.3 Run `rtk make check-code-file-lines` and ensure every changed source file is ≤800 lines; attach one-painter and source-trace evidence.
- [ ] 4.4 Run `rtk openspec validate migrate-queue-to-canonical-list --strict`.
- [ ] 4.5 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and the relevant `rtk cargo nextest run` suite.
