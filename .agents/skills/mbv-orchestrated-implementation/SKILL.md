---
name: mbv-orchestrated-implementation
description: Drive or participate in mbv's bounded multi-agent OpenSpec implementation workflow — an orchestrator feeds exact nested `tasks.md` rows to implementer agents that start from a clean HEAD, commit one new commit, and are verified with `git show`. Use when a request names a specific nested `tasks.md` row + one bounded family, says "starting from clean HEAD <sha>", "do not amend or push", "mbv-implementer", or "continuing this implementation as orchestrator". Do NOT use for a single-agent full apply (use openspec-apply-change) or a read-only review (use openspec-readonly-review).
---

# mbv-orchestrated-implementation

mbv runs large OpenSpec changes (e.g. `migrate-tui-to-tuirealm`) as a campaign of
**bounded sub-units** across several agents, coordinated by an orchestrator and a
handoff document rather than one agent doing everything. This skill encodes the
contract so future agents follow it without re-deriving it.

The authoritative, campaign-specific state lives in the change's handoff files
(`openspec/handoffs/orchestration-*.md` and the active
`/tmp/mbv-next-orchestrator-handoff.md`). Read those first; this skill supplies the
rules that are easy to violate.

## Roles

- **Orchestrator** (usually the maintainer or a lead agent): reads the handoff +
  `tasks.md`, issues one bounded prompt at a time, verifies each commit, updates
  the handoff.
- **Implementer** (e.g. the `mbv-implementer` subagent, or a focused agent):
  receives one bounded prompt, implements it from a given commit, commits once.

## Orchestrator rules

1. **One unit per prompt.** Name the *exact* nested `tasks.md` row (e.g.
   `5.3d > Mirrors and framework`) **and one bounded behaviour/ownership family**
   (e.g. "Home wheel-scroll ownership"). Prefer ~3–6 production files per unit.
2. **Give a start point.** Specify `starting from clean HEAD <sha>` (or
   `from commit <sha>`). Never ask the implementer to pick its own base.
3. **Plain prose only.** Prompts are plain text — not fenced, quoted, or slash
   commands.
4. **Do not combine concerns.** One unit = one family. Do not mix effects, mouse
   handling, state deletion, underpaint removal, synchronization teardown,
   framework deletion, and documentation in a single unit.
5. **Respect campaign gates.** Some checks are intentionally deferred (e.g.
   `check-code-file-lines` until a named later task). Honor the gate stated in the
   handoff; do not run forbidden checks early.
6. **While an implementer is in flight, do not** inspect its dirty diff, edit
   files, or issue another implementation prompt. Assume it is in flight until the
   maintainer reports a commit.
7. **Verify with `git show <sha>`.** When a unit is reported done, inspect the
   exact commit. Do not rerun the checks/suites the agent already reported.

## Implementer rules

1. **Start from the given commit.** Base work on the exact `<sha>` the orchestrator
   named. Do not continue from an arbitrary or stale HEAD.
2. **One new commit.** Commit the unit as a *single new commit*. **Do not amend**
   and **do not push**.
3. **Stay in your lane.** Implement only the named nested row + bounded family. If
   the unit accidentally requires touching another family, stop and report rather
   than silently expanding scope.
4. **Leave handoff files alone.** There are intentionally-untracked handoff
   markdown files (e.g. `openspec/handoffs/orchestration-*.md`). Do not edit or
   delete them unless explicitly asked.
5. **Report the commit SHA.** End with the committed `<sha>` so the orchestrator
   can `git show` it.

## Trigger vs. neighbour tasks

- **Triggers this skill:** "implement only the nested `tasks.md` row 5.3d > Mirrors,
  bounded sub-unit Home wheel-scroll, from clean HEAD `<sha>`", "continue the exact
  nested `tasks.md` row …, from commit `<sha>`", "implement OpenSpec task 5.4 only,
  starting from `<sha>`; commit as a new commit; do not amend or push", "you are the
  `mbv-implementer`, implement every task in the change …", "continue this
  implementation as orchestrator".
- **Does NOT trigger (use the other skill):** "implement the whole change" / a
  single-agent end-to-end apply → `openspec-apply-change`; "review/verify this WIP
  change read-only" → `openspec-readonly-review`.

This is mbv-specific. In another repo the same words may mean an ordinary
single-agent task; only apply this contract when working inside an mbv OpenSpec
campaign that uses the handoff-file orchestration.
