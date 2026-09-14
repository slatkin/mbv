## Context

See `proposal.md` for motivation. The archived `audit-remove-fragile-ui-render-tests` change classified tests as cosmetic or behavioral, but its broad rule to preserve geometry tests did not assign each geometry fact to one owner. Presentation assertions subsequently regrew across the current four relevant layers:

- arrangements own pane placement and breakpoint decisions;
- Render Components own painted cells and paint-local geometry;
- Interactive Components own local state, input interpretation, viewport, and retained hit geometry;
- shell tick integration owns mounting, focus, subscriptions, routing, projection, and cross-boundary effects.

The current tree still contains eleven ignored obsolete presentation tests, many whole-buffer render helpers, glyph assertions at multiple layers, and terminal dimensions that sometimes select a presentation accidentally. Existing project policy requires mounted `Application::tick()` integration tests for mounting, focus, subscription, or routing changes; those tests cannot simply be replaced by direct component tests. Tests must remain hermetic and use conventional Rust test tooling only.

This is a cross-cutting test-only change, so `design.md` is warranted. It deliberately has no delta spec because product behavior does not change.

## Goals / Non-Goals

**Goals:**

- Make the asserted fact determine the test layer, fixture, and oracle.
- Leave each presentation contract with the narrowest owner-level proof that would catch a regression in that owner.
- Preserve integration coverage for composition and interaction while removing repeated appearance claims.
- Make terminal dimensions communicate whether they are boundary inputs or merely sufficient fixture capacity.
- Reduce obsolete test and helper code without introducing replacement infrastructure.

**Non-Goals:**

- Raising or preserving a line-coverage percentage.
- Auditing all 213 test-bearing `src/app` files or all literal `TestBackend` dimensions.
- Adding shell/effect/run-loop coverage from #697.
- Changing production rendering, layout, hit behavior, or visible output.
- Replacing the test framework, introducing snapshots, or adding lint/check scripts.

## Decisions

### 1. Classify assertions by owning layer, not by a cosmetic/behavioral binary

Each affected test is reduced to its independent claims, and each claim is kept only at the narrowest layer that owns it:

| Layer | Owned proof | Claims removed from this layer |
|---|---|---|
| Arrangement | One relational placement or breakpoint decision | Painted glyphs, shell state, absolute coordinates |
| Render Component | Focused buffer content and paint-local geometry | Whole application frames, parent placement |
| Interactive Component | Local state transition, semantic request, viewport, retained hit resolution | Root placement and shell effects |
| Shell tick integration | Mount/focus/subscription/routing, latest-frame delivery, projection, external request dispatch | Glyph choice, spacing, repeated row arithmetic |

A test may remain at more than one layer only when the layers prove different contracts. For example, a Render Component test may prove a row paints, while a mounted mouse test proves a click on the latest painted row reaches the correct stable target; the mouse test must not restate the row's decoration or spacing.

Alternative: continue the prior cosmetic-versus-behavioral classification. Rejected because a behavioral geometry assertion can still be redundant when repeated above its owner.

### 2. Audit a bounded failure-derived set

The audit begins with known debt from #707 and #697:

- the eleven ignored obsolete presentation tests;
- remaining hero/list frame-glyph assertions and helpers used as locators;
- whole-frame equality snapshots;
- row-sensitive hero/list tests named in #707;
- mounted tick/mouse tests that repeat spacing, breakpoint, or appearance facts;
- helpers made unused by those removals.

A temporary one-row spacer perturbation may be used to reveal additional row-budget coupling. The perturbation is never committed, is restored before editing tests, and produces an inventory rather than a deletion list. Every discovered failure receives one disposition: keep unchanged, rewrite at its owner, narrow to its integration claim, or delete as duplicate/churn.

Alternative: inspect every literal terminal size or every TUI test. Rejected as an unbounded audit whose cost could exceed the debt it removes.

### 3. Treat terminal dimensions according to intent

Literal dimensions are allowed when dimension is the input under test, including exact breakpoint boundaries and undersized-area behavior. Such tests name or derive the relevant boundary directly.

When size is only fixture capacity, the test derives it from a named requirement or shared production geometry constant plus explicit slack. It must not depend on an unexplained terminal size that happens to make a hero, list, or status region fit. No general test-size abstraction is introduced unless an existing helper already expresses the needed calculation.

Alternative: ban all numeric `TestBackend::new` dimensions. Rejected because terminal dimensions are legitimate inputs to responsive TUI behavior and Ratatui fixtures require a concrete area.

### 4. Keep semantic painter signatures only at their owner

A glyph assertion used to locate another region or infer layout is removed. A glyph may remain in the owning Render Component test when that glyph is itself the component's semantic painted output, such as the seek track, but mounted or shell tests assert semantic content or role-rect containment instead.

Negative assertions about removed decoration are retained only when the absence is the owner-level presentation contract and cannot be expressed more directly; duplicate absence checks elsewhere are deleted.

Alternative: ban every glyph assertion. Rejected because that would prevent a painter's own test from proving output it directly owns.

### 5. Replace whole-frame equality, not focused buffer inspection

Whole-frame string equality is removed because it couples unrelated surfaces and blank rows. Retained painter tests inspect only the relevant cells, lines, semantic text, style role, or returned paint-local geometry. `buffer_to_string` remains where it supports focused content checks and is deleted only when unused.

Alternative: introduce approved snapshots with filtering or normalization. Rejected because it adds infrastructure while preserving the same broad oracle.

### 6. Delete obsolete tests and helpers without compensating coverage

Ignored obsolete tests are deleted rather than enabled, rewritten, or left as documentation. Helpers with no callers after the bounded audit are deleted. No replacement test is required when the removed test duplicated an already-owned fact or asserted only deliberate presentation detail.

Alternative: replace each deletion one-for-one to avoid reducing test count or coverage. Rejected because test count and line coverage are not product contracts.

### 7. Put prevention in the existing frontend skill

The `mbv-frontend` Tests section gains the layer ownership matrix, the terminal-size distinction, the glyph-owner rule, and the prohibition on whole-frame equality. Its completion checklist requires reviewers to confirm that mounted tests assert only their integration contract and that an intentional presentation change affects only its owner-level characterization.

No mechanical checker, CI wrapper, custom script, or snapshot harness is added.

Alternative: enforce the policy with source scanning. Rejected by repository policy and because lexical checks cannot distinguish semantic glyph output from a glyph used as a layout locator.

### 8. Keep #697 as a separate follow-up

After deletion, `cargo llvm-cov` may be run once to establish a fresh informational baseline if available. Coverage movement is recorded for #697 but creates no task to backfill presentation coverage. Any future shell/effect tests require their own design around hermetic seams and are outside this change.

## Risks / Trade-offs

- **[A broad test contains one valuable integration assertion beside fragile presentation assertions]** -> Split or narrow the existing test before deleting only the redundant claims; preserve the real `Application::tick()` path when composition is the subject.
- **[A row perturbation exposes a legitimate hit-geometry regression detector]** -> Treat perturbation failures as evidence of coupling, not automatic deletion; keep the behavioral target assertion at its owning component or integration layer.
- **[Derived fixture sizes duplicate production layout logic]** -> Reuse existing named constants or calculate only sufficient capacity; do not recreate the full arrangement algorithm in tests.
- **[Fewer visual assertions allow accidental appearance changes]** -> Keep one focused owner-level characterization per relevant surface and presentation; accept that unowned cosmetic details are reviewed visually rather than frozen at every layer.
- **[The audit expands indefinitely through helper dependencies]** -> Limit edits to the failure-derived families and helpers that become unused; defer unrelated shape-duplicate families and #697 gaps.
- **[Deleting ignored tests changes no execution but creates a large diff]** -> Remove them first as an isolated task so later behavioral changes are reviewable independently.

## Migration Plan

1. Record the bounded baseline and classify the eleven ignored tests.
2. Delete obsolete ignored tests and only the helpers/imports made dead by that deletion.
3. Run and restore the optional one-row perturbation, recording affected tests and dispositions.
4. Process owner families independently: arrangement/Render Component first, then Interactive Component, then mounted tick/mouse assertions.
5. Update the frontend skill after the concrete classifications prove the wording.
6. Run formatting, targeted `mbv` tests for each affected family, the full `mbv` package suite, clippy for the package, and OpenSpec validation.
7. Roll back by reverting individual family commits; no data or compatibility migration exists.
