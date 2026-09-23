---
name: openspec-plan-review
description: Adversarial review of an OpenSpec change's planning artifacts to decide whether it is ready for implementation. Use when the user asks to review, critique, red-team, or sanity-check an OpenSpec change or plan, including "review the plan", "check the change", or "is this plan good". A repeat review verifies the prior finding list plus text the fix touched; it does not start a fresh hunt. Ready means an implementer will not invent a user-visible behavior.
---

# Adversarial plan review for OpenSpec changes

Review an OpenSpec change's planning artifacts as a hostile expert reviewer whose job is to
find how the plan fails during implementation — not to confirm it. Read-only: this workflow
never edits the change's artifacts or any project code. Findings go to the user; folding them
into the artifacts is a separate workflow (`openspec-update-change`).

## Why adversarial

A plan that survives a friendly read still fails during apply: a false premise, a
requirement with two user-visible readings, a task whose verification can't run, a delta
that contradicts the main spec it overlays. Simulate an agent implementing `tasks.md` with
only the artifacts on disk. That agent may choose functions, callers, and test layout from
the code. It may not invent a user-visible behavior.

## What a blocker is

A **BLOCKER** is only one of these:

- a user-visible behavior with two readings, and the artifacts do not pick one
- a contradiction with another artifact, a main spec, or a documented ADR or naming rule
- a named premise a task depends on that is false in the code or in a cited external source
- a verification that cannot observe the requirement it claims to prove

Anything else is a **NOTE**. Call-site lists, which helper to edit, how to spell a test,
and "strengthen a manual check that already observes the requirement" are notes. If one
reading is the one the existing code already uses, that reading is not a second product
decision — record it as a note only when naming it would save the implementer a wrong turn.

## Repeat reviews

If the user, the session, or the artifacts already carry a verdict or folded review notes,
this is a repeat review. Verify that finding list and the text the fix touched. Do not
re-open a path the prior inventory already checked. A newly discovered issue is a note
unless the fix introduced it, or it is a contradiction with the finding list itself.
"The next review will find something new" is a skill failure, not rigor. A first review
(no prior verdict) is the only full hunt, and it must publish the inventory in the verdict
so the next pass cannot fail the plan for a path this pass did not name.

## Steps

On a repeat review, do not start at step 1. Verify the prior finding list and the text the
fix touched, then deliver the verdict. A full hunt is a first review only.

1. **Load the full change and its surroundings.** First review only.
   - `openspec status --change "<name>" --json` for artifact inventory, then read every
     artifact in full from disk (proposal, design, each spec delta, tasks). Partial reads
     miss cross-references, which is where plans fail.
   - `openspec validate --all` (or the change) — a structural failure is a finding, not a
     footnote. On a repeat review, skip the full surroundings read except where the finding
     list or the fix points.
   - Read the main specs each delta modifies or is adjacent to. A delta is only meaningful
     against the spec it overlays; check for contradictions, duplicate requirements, and
     scenarios the delta silently rewrites.
   - Read the project's `AGENTS.md`/`CONTEXT.md` and any ADR the design touches. A plan
     violating a documented architecture rule or naming convention is a blocker regardless
     of internal coherence.

2. **Verify premises in the code before believing them.**
   This is the highest-value step and the most commonly skipped. Plans assert things like
   "X already handles Y", "only one caller exists", "the legacy painter had this bug patched
   in `<sha>`", "this operation never fires when Z". For every factual claim about the
   codebase that a task depends on, check it: the symbol exists, the file is where design.md
   says, the call count matches. A false premise poisons every downstream task. If you cannot
   verify a premise, that itself is a finding ("unverifiable as written; the plan must cite
   the concrete symbol/behavior").
   Concretely: grep/read for each named file, type, and function; for "single writer" or
   "one caller" claims, count callers; for "already does X" claims, read the actual branch.

3. **Establish external ground truth when the plan rests on it.**
   The codebase verifies what the code does; it cannot verify whether an approach is sound.
   When a design decision, safety claim, or "best practice" assertion depends on how a
   library, protocol, or platform actually behaves — rate limits, deprecations, recommended
   patterns, API signatures — consult current external sources instead of trusting the
   plan's (or your own training-data) recollection:
   - Library/API behavior → the library's own documentation first (`ketch_docs`), then its
     release notes or issue tracker for version-specific claims.
   - "Is X the recommended pattern?" → search how real projects call it (`ketch_code`) and
     current discussion (`ketch_search`).
   - Always bound fetches (max_chars / page limits) and cite the source URL in the finding.
   This step is authorized and expected for this review — a premise the plan asserts about
   the outside world gets the same verify-before-believing treatment as a premise about the
   code. Skip it when the plan makes no external claims; research for its own sake is delay,
   not rigor.

4. **Run the attack checklist** over each artifact:

   - **Why (proposal)**: Is the motivating failure observable and reproducible from the text
     alone? Does every "What Changes" bullet trace to it? Anything in scope with no
     motivation? Anything motivated but absent from scope? Are non-goals real boundaries or
     decoration?
   - **Spec deltas**: Each requirement testable and single-reading? Scenarios cover the new
     behavior *and* the regressions the change risks? Any requirement the tasks never
     implement, or task work no requirement covers? Breaking changes declared?
   - **Design**: Are decisions grounded in the code as it exists (step 2)? Material choices
     have stated alternatives and a reason? Are decision IDs (D1, D2…) present and referenced
     by tasks? Does the design contradict the delta specs?
   - **Tasks**: Implementable by an agent with only these artifacts? Ordering respects
     dependencies (types before callers, shared files sequenced)? Every task carries a
     verification that can actually run (named tests/commands, not "verify it works")? Does
     the test strategy obey repo rules (hermetic mocks, no real externals, no forced sleeps)?
     Hidden work missing: deletions, migration of pinned tests, spec sync, archive steps?
   - **Cross-artifact coherence**: The set {proposal ⇒ specs ⇒ design ⇒ tasks} must close.
     The classic failure is an artifact updated after a scope change while its downstream
     siblings still carry the old scope.

5. **Classify and evidence every finding.** Use **What a blocker is**. Do not promote a
   note because the checklist found something. A missing required artifact or a failed
   `openspec validate` is a blocker; a dependency-ordering trap is a blocker only when it
   would make the implementer invent a user-visible behavior or follow a false premise.
   - Every finding cites evidence: `file:line`, a quoted sentence, or the exact command that
     failed. "Feels thin" is not a finding. Before reporting a BLOCKER, re-check its premise
     against the code — a false blocker wastes a whole correction round.

6. **Deliver the verdict.** Use exactly this shape:

   ```
   # Plan review: <change-name>

   Verdict: READY | READY WITH NOTES | NOT READY

   ## BLOCKING findings
   - [B1] <finding> — <evidence> — <smallest fix>

   ## Notes (non-blocking)
   - [N1] <finding> — <evidence>

   ## Coverage
   | Artifact      | Reviewed | Result            |
   |---------------|----------|-------------------|
   | proposal.md   | yes      | clean / see B2    |
   | specs/...     | yes      | ...               |
   ```

   READY means an implementer will not invent a user-visible behavior and will not follow a
   false premise. It does not mean the attack checklist is exhausted. READY only when there
   are zero blockers under **What a blocker is**; READY WITH NOTES when blockers are absent
   but notes are material; NOT READY otherwise. A missing required artifact or a failed
   `openspec validate` is NOT READY by definition.

   On a first review, end the verdict with an **Inventory**: every persistence/restore path
   and every external contract the tasks depend on, each marked checked or unchecked. A
   later review treats that list as closed.

7. **Stop after the verdict.** Offer the follow-up path: "Address findings via
   `openspec-update-change`. A follow-up review verifies that finding list only." Do not
   edit artifacts, do not start apply. Do not invite a fresh hunt.
