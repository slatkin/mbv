# Design

## Context

See proposal.md — Why. The audit (2026-11-04, three scouts) produced a per-ADR verdict list and a pattern inventory; those are the worklist. Constraints that shape the approach:

- ADRs are a fixed architecture source (`AGENTS.md` Start here). Edits must never weaken a decision record; the rot lives only in amendment blocks and status sentences.
- `openspec/specs/` is the synced source of current behavior. ADRs that need to state "what is true now" must point there, not restate it — any restatement recreates today's problem one refresh cycle later.
- Repo conventions forbid bespoke checkers; the rule and the fixes are prose and one-time edits, enforced by review, not by a CI lint.
- The `openspec` archive workflow dates change directories (`archive/YYYY-MM-DD-<name>/`). Undated refs break when a change archives; dated refs survive.

## Goals / Non-Goals

**Goals:**
- Every checkable claim in `docs/adr/` and `CONTEXT.md` is either still true, a decision statement, or a dated-historical statement that is unambiguous about being historical.
- A written amendment rule that stops the rot class from re-entering.
- The five audit ambiguities resolved by verification before any text that depends on them is written.

**Non-Goals:**
- No ADR renumbering, no reformatting of untouched ADRs, no rewriting of decision bodies or rationale.
- No spec deltas (skip_specs). The ADR 0027 / hero-layout spec correction is its own change, pending the user's decision.
- No build-file cleanup (libmpv2 manifest residue), no code changes, no CI additions.

## Decisions

**D1. The amendment rule lives in `docs/adr/README.md` (new, short), with a one-line pointer added to the ADR bullet in `AGENTS.md` Start here.**
The rule: an ADR records a decision and its rationale; an amendment records a decision *change*. An amendment never states the state of `main` (versions, capabilities shipped or not), never states PR/issue/milestone status, and never carries file paths with line numbers. When the surrounding world changes, the amendment names the superseding ADR or the owning spec — the sources that are actually kept current. Alternative considered: put the rule in `docs/standards/` — rejected, standards hold code rules and are keyed to review of code changes; ADR authorship conventions belong beside the ADRs where an author will see them. Alternative considered: no written rule, just fix the text — rejected, that guarantees re-rot; the same class already re-entered after the 2026-08-14 wave.

**D2. Status sentences are deleted, not refreshed.**
Where an amendment says "on main today X", "PR #N open", "planned in changes/Y", the edit deletes the status sentence and, only where a reader needs the pointer, adds a reference to the owning spec or a code symbol (`CTRL_PROTOCOL_VERSION`, `daemon_admits`). No new dated "refreshed" stamps: a fresh stamp is just the next thing that rots. Superseded banners (`> Superseded by ADR …`) and dated historical statements are kept verbatim — they are decision history, not status. Alternative considered: rewrite each amendment as a full current-state summary — rejected, that is the rot pattern itself.

**D3. Anchor policy: crate/symbol names yes, file:line no, undated change refs no.**
Prose that must say where something lives names the crate or type (`crates/mbv-ctrl`, `daemon_admits`), never `path.rs:NNN`; a path without a line number is acceptable where the location itself is the fact, and line numbers are deleted everywhere. Every `openspec/changes/<name>/` reference carries its dated `archive/` prefix. Mechanical; decision text otherwise untouched.

**D4. Verify-then-edit for every non-mechanical fix.**
Each stale claim is re-verified against the tree at edit time before its replacement text is written (the scouts' evidence is a starting point, not a substitute — main moves). The five proposal ambiguities are explicit verification tasks whose outcome decides the text; a task that verifies "still open" writes nothing.

**D5. The 2026-08-14 cluster is one coordinated edit, reviewed as one commit.**
0014, 0015, 0017, 0018, 0019, and `CONTEXT.md` (Owner admission §, mbvd §, control-credential § pending verification) tell one story (PR #529, protocol versions, ABS admission); editing them atomically keeps the story consistent and reviewable. Mechanical anchor fixes in other files are separate commits.

## Risks / Trade-offs

- [A rewrite deletes rationale along with status] → D2's delete-only-status rule; every edited amendment is diffed against the original and reviewed for lost decision content before commit.
- [Replacement text introduces a new inaccuracy] → D4 verify-then-edit; replacement text cites a spec requirement or code symbol, which the reviewer can check mechanically.
- [The rule in `docs/adr/README.md` goes unread] → the `AGENTS.md` Start-here pointer is the discovery path agents actually use; the README is the canonical text.
- [Deleting line anchors loses useful locators] → accepted: the audit showed they drift within months; symbol names survive the same moves.

## Migration Plan

Prose-only. One commit for the rule + README, one commit for the 2026-08-14 cluster, one commit per mechanical group (modularization paths, dispatch-split paths, archive-prefix refs, futures-that-arrived). Rollback is `git revert` per commit. No code, no specs, no build impact; gates reduce to `openspec validate` and a final read-through of every touched file.

## Open Questions

None. The five audit ambiguities are verification tasks inside the change (their outcomes select replacement text, not approach); the 0027 spec-sync decision is the user's, recorded in the proposal as a separate change.
