## Context

See proposal.md for motivation. `rstest 0.27` is already a dev-only workspace dependency with `default-features = false`; its current use is three named-case tables from #710. A read-only follow-up inventory (`candidates.md`) found 21 current case-table candidates (~75 tests in 13 files) and seven eligible same-file fixture helpers. The workspace otherwise has roughly 50 `make_*` helpers, many of which are one-off or cross-module support rather than fixture candidates.

The existing testing policy requires mocks only, forbids live/sleep-based tests, and permits named `#[case]` tables only for fixture-varying families—not mass-generated thin tests.

## Goals / Non-Goals

**Goals:**

- Turn the remaining audit opportunity into a finite, reviewable candidate ledger before edits begin.
- Use `#[case]` only where a table preserves a fixture-varying assertion set and keeps named, filterable failures.
- Use `#[fixture]` only to replace existing same-file shared construction while preserving each test's meaningful setup and assertion.
- Keep per-file commits and count-parity evidence so the change is packaging-only.

**Non-Goals:**

- A blind conversion of every `make_*` helper or every superficially similar test.
- Cross-file fixture frameworks, shared fixture registries, parameter generation, async rstest features, new dependencies, or production-code changes.
- New coverage, changed test semantics, or conversion of a family that no longer meets the selection rule.

## Decisions

**D1 — Freeze the discovered candidates in a ledger before conversion.** `openspec/changes/expand-rstest-adoption/candidates.md` lists every selected `#[case]` family, every selected fixture helper, old test names/count, owner file, and deferred categories. It is the acceptance baseline; implementation confirms it against the live source before each file edit and records a deferral rather than extending scope.

Alternative: convert all 13 historical families by count. Rejected: the historical audit did not enumerate locations and a count alone is not sufficient evidence that a current family remains uniform.

**D2 — Two independent eligibility rules.** A case-table candidate requires at least two existing tests in one file whose body differs only in named fixture values/expectations, with one assertion shape and a meaningful case name derived from each old test. A fixture candidate requires one existing `make_*` helper used by at least two tests in the same file; converting it must delete repeated test setup or make injection clearer without hiding varying inputs. A candidate that matches neither remains unchanged and is documented as deferred.

Alternative: use `#[fixture]` for all helpers or use `#[values]` to enumerate inputs. Rejected: both add indirection or generate thin tests contrary to `AGENTS.md`.

**D3 — Preserve test identity and assertions mechanically.** Each table uses `#[rstest]` with named `#[case::<old_name>]` rows. Each converted helper is marked `#[fixture]` and injected by its existing concept name (or a direct, clear rename). The ledger captures old names; `cargo nextest list` confirms generated names and total count parity. No expectation is encoded implicitly or omitted from a case row.

Alternative: positional cases or a macro. Rejected: positional output loses diagnosability; a macro is bespoke tooling.

**D4 — One file per conversion commit; fixtures and cases can share a commit only when they change the same test module.** This preserves the #710 review model. The candidate-ledger and final verification notes are separate docs commits. `cargo fmt --all`, target-package clippy, and target-package nextest gate each converted file; the final workspace gate verifies the complete set.

Alternative: a single mechanical sweep commit. Rejected: it hides assertion and count changes across unrelated test modules.

**D5 — Keep the dependency boundary unchanged.** Use only the installed `rstest` macros needed for `#[rstest]`, `#[case]`, and `#[fixture]`. The final check repeats `cargo tree -p rstest --target all` and rejects an async runtime or timeout dependency path.

## Risks / Trade-offs

- **[Candidate ledger grows beyond a reviewable batch]** → Split implementation into file-bounded units; defer anything that cannot preserve a one-to-one assertion mapping.
- **[A fixture hides material per-test state]** → Keep the varying input explicit in the test or retain the helper; fixture adoption is not mandatory.
- **[Generated names break filters or count parity]** → Require named-case `nextest list` evidence and unchanged package/workspace totals before acceptance.
- **[Proc-macro ergonomics cost more than duplication]** → Defer the candidate and record why; do not force rstest use to justify the dependency.

## Migration Plan

1. Establish and commit the candidate ledger without source edits.
2. Convert selected case families and fixtures one test module at a time, with isolated commits and per-file gates.
3. Record final selected/deferred disposition and test-count/build-time evidence; run workspace gates.
4. Roll back by reverting the file-isolated commits, then the ledger/docs commits. Runtime artifacts and data require no migration.

## Open Questions

None. The exact current candidate set is intentionally resolved by D1's ledger before any conversion, with D2 defining the acceptance boundary.
