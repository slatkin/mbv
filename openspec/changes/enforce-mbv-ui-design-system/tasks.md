Steps 1–5 are separate PRs, in order. Step 5 repeats per surface and is tracked in
`ledger.md`. Nothing in step 5 starts before step 1 merges.

## 1. Split the render tree (PR 1 — maintainer-decided, mechanical)

- [ ] 1.1 Commit 1: create `src/app/render/{screens,arrangements,components}/` and
      move whole modules/functions that are already one-sided (either read app
      state or paint, not both) per the classification table in `design.md`. No
      behaviour change; existing render tests pass unchanged. `theme/` is not
      created here — nothing is classified into it until step 2 (task 2.1) moves
      `palette.rs`.
- [ ] 1.2 Commit 2: split functions that both read app state and paint at that
      seam — state half to `screens/`, painting half to `components/`. Function
      extraction is expected here; no behaviour change.
- [ ] 1.3 Split `feeds.rs` (792), `render/mod.rs` (715), `audiobookshelf.rs` (702),
      and `queue.rs` (697) during the move so nothing crosses the 800-line cap.
- [ ] 1.4 Verify: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk make check-code-file-lines`. No test file edited except
      for module paths.
- [ ] 1.5 Before adding domain terms, flag three collisions to the user per the
      repo term-coordination rule rather than resolving them unilaterally:
      "arrangement" is already used in `CONTEXT.md:345` and `:365` for a named
      responsive layout, not this change's "module layer that places components
      and owns breakpoints"; "variant" is already used in
      `openspec/specs/ui-design-language/spec.md:51-59` for a colour variant, not
      structural variation; "component" must be defined against **Panel** without
      colliding with it, per archived task 9.2 (unchecked) in
      `openspec/changes/archive/2026-08-17-centralize-ui-design-language/tasks.md`.
      Once resolved, add component, arrangement, bespoke surface, policy, and
      variant to `CONTEXT.md` under Presentation.

## 2. Make palette primitives private (PR 2 — compiler-driven)

- [ ] 2.1 Move the raw `Color` constants from `src/app/palette.rs` into a private
      module under `render/theme/`; leave only semantic roles public.
- [ ] 2.2 Build, and fix the call sites the compiler reports. Of 520 `palette::`
      references total, ~130 already use a role and compile untouched; the
      remaining ~390 use a primitive directly and need a role assigned per the
      archived "Role vocabulary (final)" table
      (`openspec/changes/archive/2026-08-17-centralize-ui-design-language/design.md`,
      starting at line 83) — that table already assigns a role to every
      primitive; this step implements it rather than inventing role names. Where
      a site genuinely isn't covered by the table, add a named role.
- [ ] 2.3 Review the added roles as the substantive output of this step. A 1:1
      rename from the archived table (for example `TEXT_PRIMARY` from `TEXT`,
      `SURFACE_PANEL` from `PANEL_BG`) is correct — the table already assigned
      it meaning. The bypass is a role named after a hue, or invented outside
      the table, at a call site the table does not cover.
- [ ] 2.4 Verify no behaviour change: existing buffer tests pass with no expected-output
      edits.

## 3. Guidance and bypass checks (PR 3)

- [ ] 3.1 Add mandatory TUI ownership rules to `AGENTS.md`: screens do not call
      Ratatui, construct rects, or compute hit targets; overrides live in the central
      owner; new UI work follows the boundary from this PR forward.
- [ ] 3.2 Add `mbv-frontend`'s `SKILL.md` — with the reuse workflow, the
      controlled-override decision table, Ratatui patterns, and a completion
      checklist covering component ownership, narrow-width behaviour, interaction
      targets, and buffer tests — to both `.opencode/skills/mbv-frontend/` and
      `.claude/skills/mbv-frontend/`. A skill in one tree is not discoverable
      from the other harness.
- [ ] 3.3 Include worked examples distinguishing content changes, named policies,
      central variants, new components, and bespoke surfaces — showing that none of
      them permits screen-owned geometry.
- [ ] 3.4 Add ast-grep rules scoped to `render/screens/`: `use ratatui::`,
      `render_widget`, `Layout::`, `Rect` construction, `buffer_mut`.
- [ ] 3.5 State in the skill what the checks do not catch — duplicated arrangement
      geometry, hit targets drifted from painting — so review knows what it owns.
- [ ] 3.6 Run the checks against the post-step-1 tree; the screen modules must be
      clean, since step 1 already moved the painting out.
- [ ] 3.7 Wire the ast-grep rules into a runnable harness, modelled on
      `scripts/check-code-file-lines.sh` + the `check-code-file-lines` Makefile
      target (`Makefile:20`) + `.github/workflows/code-file-lines.yml`: an
      `sgconfig.yml` and a rules directory at the repo root (neither exists
      yet), a `rtk make` target that invokes `ast-grep scan`, and a CI workflow
      that runs that target on every PR.

## 4. Hit-target ownership: design gate (PR 4 — design only)

- [ ] 4.1 Start from archived tasks 8.1-8.5 in
      `openspec/changes/archive/2026-08-17-centralize-ui-design-language/tasks.md`
      (left unchecked when that change archived): 8.1 defines the hit-target
      representation produced by components; 8.2 names the four per-screen row
      representations in `LayoutMain` (`left_item_rows`, ...); 8.3 names the two
      per-screen pane rects (`wide_music_right_area`, ...); 8.4 collapses the
      per-screen branches in `input_mouse.rs`, `input_mouse_panels.rs` and
      `lib_cursor_actions.rs`; 8.5 is the manual verification. Read
      `src/app/input_mouse.rs`, `input_mouse_dispatch.rs`, and
      `input_mouse_panels.rs` against those five tasks and write down how each
      surface currently resolves a click.
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
