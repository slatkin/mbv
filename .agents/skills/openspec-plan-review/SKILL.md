---
name: openspec-plan-review
description: Adversarial review of an OpenSpec change's planning artifacts (proposal, design, spec deltas, tasks) to decide whether it is ready for implementation. Use whenever the user asks to review, critique, red-team, or "sanity check" an OpenSpec change or plan, asks "is this ready to implement", wants a gate before starting the apply workflow, or asks to find holes, gaps, contradictions, or risks in a proposal/design/tasks — even if they just say "review the plan", "check the change", or "is this plan good".
---

# Adversarial plan review for OpenSpec changes

Review an OpenSpec change's planning artifacts as a hostile expert reviewer whose job is to
find how the plan fails during implementation — not to confirm it. Read-only: this workflow
never edits the change's artifacts or any project code. Findings go to the user; folding them
into the artifacts is a separate workflow (`openspec-update-change`).

## Why adversarial

A plan that survives a friendly read still fails during apply: an unverifiable premise, a
requirement with two readings, a task whose verification can't run, a delta that contradicts
the main spec it overlays. The implementing agent cannot ask the author questions — every
ambiguity becomes an invented decision. Your review must simulate the worst reader: an agent
on a fresh session implementing `tasks.md` with only the artifacts on disk.

## Steps

1. **Load the full change and its surroundings.**
   - `openspec status --change "<name>" --json` for artifact inventory, then read every
     artifact in full from disk (proposal, design, each spec delta, tasks). Partial reads
     miss cross-references, which is where plans fail.
   - `openspec validate --all` (or the change) — a structural failure is a finding, not a
     footnote.
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

5. **Classify and evidence every finding.**
   - **BLOCKER** — would produce a wrong, broken, or stalled implementation: contradiction,
     unverifiable premise a task depends on, untestable requirement, missing required
     artifact, convention violation, dependency-ordering trap.
   - **NOTE** — worth fixing, does not block: weaker wording, missing alternative, thin
     verification, opportunistic cleanup.
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

   READY only when there are zero blockers; READY WITH NOTES when blockers are absent but
   notes are material; NOT READY otherwise. If a required artifact is missing or the change
   fails `openspec validate`, the verdict is NOT READY by definition.

7. **Stop after the verdict.** Offer the follow-up path: "Address findings via
   `openspec-update-change`, then re-run this review." Do not edit artifacts, do not start
   apply. If the user wants a second opinion after fixes, re-review only the findings'
   artifacts plus anything their fix touched.
