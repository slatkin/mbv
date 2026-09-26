# Invariant 14 — Every test owns a contract at exactly one layer

**Scope:** the whole `mbv` test suite (unit, component, tick-integration),
and every plan, review, or agent instruction that causes tests to be written.

## The invariant

1. Every test names one contract — a behaviour whose loss would be a real
   bug — and is the sole test owning it. If two tests fail for the same
   production change, one of them should not exist.
2. A test lives at the narrowest layer that can regress its contract. For
   presentation code the four-layer matrix in `.agents/skills/mbv-frontend/
   SKILL.md` (§Tests) assigns ownership: arrangements own relational
   placement and breakpoints; render components own painted content and
   paint-local geometry; interactive components own local state transitions,
   semantic requests, viewport, and retained hit resolution; shell tick
   integration owns mount/focus/routing, latest-frame delivery, projection,
   and cross-boundary effects. Outside the TUI the same rule applies
   without the matrix: narrowest owning layer, one owner.
3. A regression test carries its provenance — the issue or commit it guards,
   in its name or a comment. Provenance is what makes "delete by default,
   keep by exception" auditable: an uncited test asserting the same thing as
   another test is deletable; a cited one is a guard.
4. `#[case]` tables enumerate only cases whose expected outcomes differ.
   Identical-outcome cases are one case.

## Why it matters

The suite has been pruned wholesale three times in two weeks:
#708 (`bf55fcfbd`, 2026-09-14, "right-size TUI presentation tests"), #801
(2026-09-25, render-suite pruning), and the `prune-tui-test-suite` change
(~2,096 tests → ≤800). Each prune deleted hundreds of tests that cost real
effort to write, review, and keep compiling — and each deletion was safe
only because ownership could be reconstructed by hand, test by test.

The churn recurs because tests are authored by mirroring code structure —
one per breakpoint, one per screen, one per plan item — instead of by
contract. A test whose only justification is "the task said add tests" or
"the code has a narrow mode" duplicates a contract owned elsewhere and will
be deleted in the next prune. The waste is not the deletion; it is the
writing.

## How the code maintains it today

- **AGENTS.md (Tooling):** the authoring gate — name the contract and its
  owning layer before writing; extend an existing test rather than adding a
  duplicate; `#[case]` tables keep only outcome-differing cases; regression
  tests cite their issue/commit; "add tests" without a named contract is
  not a valid plan item; test count and coverage are never goals.
- **mbv-frontend skill (§Tests):** the layer-ownership matrix, consulted
  before adding, narrowing, or deleting any presentation assertion.
- **writing-tests skill:** the standing question — "what realistic problem
  would this test catch that another test would not?"
- **Review skills** (`review`, `code-review`): diff-added tests that
  duplicate an existing contract or enumerate identical outcomes are
  findings, not neutral additions.

## Where it currently fails / how it could regress

Nothing mechanical enforces any of this; the guardrails are prose at
authoring and review time.

- **Plans are the biggest source.** A task that says "add tests for X"
  without naming the contract produces exactly the tests the prunes
  deleted. Task authors must name the contract, or the implementer must
  push back.
- **Skills go unread.** The matrix lives in a skill triggered by TUI work;
  an agent writing non-TUI tests never sees it. For non-TUI code the rule
  reduces to "one contract, one owner, narrowest layer" — that sentence is
  in AGENTS.md precisely so it does not depend on a skill firing.
- **A prune that ignores provenance** can delete the only regression guard
  for a fixed bug. Keep-by-exception depends on the citation being present
  and truthful; a false citation is worse than none, because it launders a
  duplicate into a "guard".
- **Helper/seam residue.** Pruning tests orphans `test_*` seams and fixture
  helpers in production files. Delete them with the tests (AGENTS.md
  forbids `allow`/`expect`, so they surface as `dead_code` — that warning
  is the sweep mechanism, not a nuisance).
