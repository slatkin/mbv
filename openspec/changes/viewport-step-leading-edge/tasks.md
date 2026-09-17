# Tasks: viewport-step-leading-edge

Reference: `design.md` decisions D1–D3; spec deltas in `specs/canonical-media-lists/` and
`specs/mouse-input/`; standing rule `docs/invariants/06-viewport-step-leading-edge.md`.
Run `cargo nextest run -p mbv` after each task; keep it green.

## 1. Owner: the leading-edge drag (D1)

- [ ] 1.1 Resolve the drag target by step direction in `MediaList::drag_selection_into_window`
  (`src/app/components/media_list/mod.rs`): a step toward the preceding display row drags to the first
  selectable row of the new window, a step toward the following row drags to the last selectable row of the
  new window. Keep the selectable-only resolution (skip `Heading`/`Spacer`), the "no selectable row in the
  window leaves the selection untouched" fallback, and the existing `target == base` boundary no-op.
  (Verify: owner unit tests both directions — the landing row is the leading edge, not the edge the
  selection left; a structural row at the leading edge resolves to the nearest selectable row at that edge.)
- [ ] 1.2 Reconcile the owner tests that pinned the nearest-row landing
  (`viewport_step_at_the_window_edge_drags_the_selection_both_ways`, its sibling structural-row and
  no-selectable-row cases, and the task-state of `viewport_step_inside_the_window_moves_only_the_window`,
  which must not change), keeping the containment and content-end cases intact.
  (Verify: `cargo nextest run -p mbv -- components::media_list`.)
- [ ] 1.3 Add the named guard test that is Invariant 6's enforcement point: a step that drags never parks
  the selection on the edge it left, for the one-row form and the page form, in both directions; it must
  fail if the landing reverts to the trailing edge.
  (Verify: the guard test, plus a mutation probe — restoring the trailing-edge resolution makes it fail.)

## 2. Surface evidence (D2)

- [ ] 2.1 Update the per-surface drag-at-edge assertions to the leading-edge landing: Emby library, Music
  album rail and track list, Queue, TV rail and overlay episodes, podcast show and episode lists, book and
  chapter lists, Feeds, Inline Search.
  (Verify: the affected component and key/tick suites green with the new expected rows.)
- [ ] 2.2 Update the grouped-list tick-integration viewport-step evidence
  (`viewport_step_inputs_walk_a_grouped_list_to_display_row_0_wide_and_narrow`) so its per-step
  inside/outside predicate and dragged-selection row assertions state the leading-edge landing, at Wide and
  non-Wide heights, for the wheel and the chord.
  (Verify: `src/app/tests_tick_integration_library_scroll.rs` green through `Application::tick()`.)

## 3. Standing rule (Invariant 6)

- [ ] 3.1 Confirm the standing artifacts name the leading-edge rule and the recurrence history:
  `docs/invariants/06-viewport-step-leading-edge.md` and the "Scrolling and selection" section of
  `.agents/skills/mbv-frontend/SKILL.md` (both landed with this plan; update only if the implementation
  contradicts a named symbol or test).
  (Verify: named symbols and tests in the invariant resolve in the tree.)

## 4. Gates

- [ ] 4.1 Full gates: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo nextest run -p mbv` and `-p mbv-core`; `openspec validate --all`. Update the
  `docs/architecture/interactive-surface-ledger.md` verification note if it names the landing edge.
  (Verify: all gates clean.)
