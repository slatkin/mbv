## Why

PR #552 extracted shared painters and colour roles, and #560–#562 replaced separate
detail blocks across library surfaces. #584 settled the hero arrangement
(hero-on-left wide, selected-row replacement narrow). What remains is ownership:
screens still bypass shared geometry, call raw Ratatui APIs, duplicate hit-target
arithmetic, and select raw palette values independently.

The boundary is convention-only, and convention has not held.

## What Changes

The boundary is established mechanically, then surfaces move behind it one at a
time. Each step below is a separate mergeable PR; none depends on a later one.

1. **Split `src/app/render/` into named screen and component/arrangement
   modules.** The classification is the deliverable — a flat list of 45
   non-test modules that both compose and paint gives the rules nothing to
   attach to. The split preserves behaviour; the one-sided modules move
   wholesale, and functions that both read state and paint are extracted at
   that seam.
2. **Make raw palette constants private.** `src/app/palette.rs` currently
   defines semantic roles as aliases beside the raw constants they alias, in the
   same file, all public. Moving primitives behind a private module and exposing
   only roles turns the 520 `palette::` call sites into a compiler-reported
   list; ~130 already use a role, and assigning the remaining ~390 is real
   work — see `design.md` step 2 for why it is mechanical rather than a fresh
   naming exercise.
3. **Add the guidance and the bypass checks.** Mandatory rules in `AGENTS.md`, a
   committed `mbv-frontend` skill, and ast-grep rules scoped to the screen
   modules created in step 1.
4. **Design hit-target ownership against the real consumer before building it.**
   Mouse handling lives in `src/app/input_mouse*.rs` (1,558 lines) and runs on a
   later event against app state, not against render output. A component that
   emits a hit map needs a render→input channel that does not exist. Step 4 is a
   written design against those files with a go/no-go; if the plumbing does not
   fall out cleanly it is deferred rather than half-shipped.
5. **Migrate surfaces individually.** Each surface gets a characterization
   `TestBackend` buffer test landed first, in its own commit, then the migration.
   Output is preserved; the test proves it.

Completion is tracked by a checked-in ledger of unmigrated surfaces
(`openspec/changes/enforce-mbv-ui-design-system/ledger.md`) that may only shrink.
The boundary is enforced immediately for new code and for any surface a change
touches. Surfaces nobody touches are not blocking.

### Why not whole-tree-or-nothing

The earlier draft required every surface classified and enforced before this
change could close, with no grandfathering. Against 21k lines of non-test render
code, 520 palette sites in 56 files, and 11 overlay files, 9 with no buffer
coverage, that is a multi-month rewrite behind a single issue with no merge
points — and it puts the untested overlays in the same batch as the well-tested
list/hero paths. The ledger reaches the same endpoint while every step is
reviewable on its own.

## Capabilities

### New Capabilities

- `ui-design-system`: Defines the mbv TUI component, arrangement, theme, variant,
  interaction, and development-guidance contract.

### Modified Capabilities

- `right-panel-arrangements`: Tightens the arrangement-ownership boundary on the
  post-#584 hero baseline.
- `library-list-hero`: Extends the hero-ownership model; hit-target ownership
  lands only if step 4 clears its go/no-go.
- `ui-design-language`: Makes raw palette primitives private so screen code
  consumes semantic roles, completing the narrowing #552 intended.

## Impact

- `src/app/render/`: module split into screen vs component/arrangement
  responsibilities; per-surface migration thereafter.
- `src/app/palette.rs`: primitives privatised, roles promoted; 520 `palette::`
  call sites touched, ~390 newly assigned a role.
- `src/app/input_mouse*.rs`: read during step 4's design; changed only if step 4
  clears.
- `CONTEXT.md`: new domain terms (component, arrangement, bespoke surface,
  policy, variant) under Presentation — three collide with existing entries and
  are flagged to the user before anything is added (tasks.md 1.5).
- `AGENTS.md`: mandatory UI rules.
- `.opencode/skills/mbv-frontend/` and `.claude/skills/mbv-frontend/`: committed
  agent workflow and review guidance, in both trees.
- ast-grep rules for direct painting and raw-palette access in screen modules.
- No runtime protocol, persisted-data, or user-facing media behaviour changes.

### File-size pressure

`feeds.rs` (792), `mod.rs` (715), `audiobookshelf.rs` (702), and `queue.rs` (697)
sit near the repo's 800-line cap. Step 1 must split them as part of the move, not
discover the overrun later.

## Tracking

- Parent issue: https://github.com/slatkin/mbv/issues/563
- Prerequisite: #584, complete.
- Child issues are per-surface migrations, one surface each. Each names its files,
  requires its characterization test first, and produces a diff. They are the
  mechanism by which the ledger shrinks.
