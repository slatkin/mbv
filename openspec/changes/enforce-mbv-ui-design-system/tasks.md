Steps 1–5 are separate PRs, in order. Step 5 repeats per surface and is tracked in
`ledger.md`. Nothing in step 5 starts before step 1 merges.

## 1. Split the render tree (PR 1 — maintainer-decided, mechanical)

- [ ] 1.1 Create `src/app/render/{screens,arrangements,components,theme}/` and move
      each existing module's functions per the classification table in `design.md`.
      Move only — no logic edits in this PR. Existing render tests pass unchanged.
- [ ] 1.2 Split functions that both read app state and paint at that seam: state half
      to `screens/`, painting half to `components/`.
- [ ] 1.3 Split `feeds.rs` (792), `render/mod.rs` (715), `audiobookshelf.rs` (702),
      and `queue.rs` (697) during the move so nothing crosses the 800-line cap.
- [ ] 1.4 Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk make check-code-file-lines`. No test file edited except
      for module paths.
- [ ] 1.5 Add the new domain terms (component, arrangement, bespoke surface, policy,
      variant) to `CONTEXT.md` under Presentation, per the repo term-coordination rule.

## 2. Make palette primitives private (PR 2 — compiler-driven)

- [ ] 2.1 Move the raw `Color` constants from `src/app/palette.rs` into a private
      module under `render/theme/`; leave only semantic roles public.
- [ ] 2.2 Build, and fix the ~509 call sites the compiler reports. Where no existing
      role fits, add a named role rather than re-exporting the primitive.
- [ ] 2.3 Review the added roles as the substantive output of this step. A role that
      is a rename of one primitive with no semantic meaning is a bypass.
- [ ] 2.4 Verify no behaviour change: existing buffer tests pass with no expected-output
      edits.

## 3. Guidance and bypass checks (PR 3)

- [ ] 3.1 Add mandatory TUI ownership rules to `AGENTS.md`: screens do not call
      Ratatui, construct rects, or compute hit targets; overrides live in the central
      owner; new UI work follows the boundary from this PR forward.
- [ ] 3.2 Add `.opencode/skills/mbv-frontend/SKILL.md` with the reuse workflow, the
      controlled-override decision table, Ratatui patterns, and a completion checklist
      covering component ownership, narrow-width behaviour, interaction targets, and
      buffer tests.
- [ ] 3.3 Include worked examples distinguishing content changes, named policies,
      central variants, new components, and bespoke surfaces — showing that none of
      them permits screen-owned geometry.
- [ ] 3.4 Add ast-grep rules scoped to `render/screens/`: `use ratatui::`,
      `render_widget`, `Layout::`, `Rect` construction, `buffer_mut`.
- [ ] 3.5 State in the skill what the checks do not catch — duplicated arrangement
      geometry, hit targets drifted from painting — so review knows what it owns.
- [ ] 3.6 Run the checks against the post-step-1 tree; the screen modules must be
      clean, since step 1 already moved the painting out.

## 4. Hit-target ownership: design gate (PR 4 — design only)

- [ ] 4.1 Read `src/app/input_mouse.rs`, `input_mouse_dispatch.rs`, and
      `input_mouse_panels.rs` and write down how each surface currently resolves a
      click.
- [ ] 4.2 Answer, in writing: where the hit map is stored between frames; what
      invalidates it (resize, tab switch, scroll, repaint-less state change); what a
      mouse event does before first paint and after resize; whether components publish
      into a shared map or return one arrangements aggregate.
- [ ] 4.3 Go/no-go. On go, add the typed hit-map contract and migrate every surface
      that handles mouse in one PR. On no-go, record the reason, keep the existing
      coordinate arithmetic, and drop the hit-target requirement from the delta spec.
- [ ] 4.4 Do not ship a partial migration. Some surfaces on hit maps and the rest on
      coordinate arithmetic is worse than either alone.

## 5. Per-surface migration (repeats — one PR per surface)

For each surface in `ledger.md`, in two commits:

- [ ] 5.1 Commit 1: characterization `TestBackend` buffer test capturing current
      output — default, focused, narrow-width, and selected states. No production
      code in this commit.
- [ ] 5.2 Commit 2: route the surface through arrangement/component ownership. The
      characterization test is unchanged and still passes.
- [ ] 5.3 Record the surface's hero additional-content style (Movie overview, TV
      seasons/pills and episodes, Music tracks, or a mapped provider-specific style)
      if it is hero-bearing.
- [ ] 5.4 Represent screen differences as typed content models or named central
      policies, not screen-local painter branches.
- [ ] 5.5 If the surface genuinely cannot reuse the vocabulary, register it as a
      named bespoke component with its reason and its own buffer coverage. It still
      obeys ownership, semantic styling, and verification rules.
- [ ] 5.6 Tick the surface off in `ledger.md` in the same PR.

## 6. Sync specs

- [ ] 6.1 Sync `right-panel-arrangements` with the post-#584 hero-on-left-wide /
      selected-row-replacement-narrow baseline.
- [ ] 6.2 Sync `library-list-hero` and `ui-design-language` with the tightened
      ownership and the private-primitive theme API.
- [ ] 6.3 Reconcile the `ui-design-system` delta spec with step 4's outcome.
- [ ] 6.4 `openspec validate` passes.
