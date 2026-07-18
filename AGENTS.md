# General Rules

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```
Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## 5. Unit Testing is Not Real Testing

Unit tests are an extremely imperfect tool with baked-in assumptions. They are meant to capture intended code behaviour and not an actual test of product functionality or if a feature actually works. They are not a source of truth for anything.

For reported regressions, follow `docs/agents/debugging-regressions.md` before editing code or writing tests.

# Repo

## Development

Read `docs/agents/worktrees.md` before beginning development. Read `docs/agents/repo.md` for repo setup and configuration. See `docs/CHECKIN.md` for pre-commit steps.

## Issue tracker

Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

## Triage labels

Uses the default label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `test`, `wontfix`). See `docs/agents/triage-labels.md`.

## Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Repo Rules
- Use Serena if installed for code exploration and targeted writes.
- For releases, run `scripts/release.sh X.Y.Z "one-line summary"` instead of reading a separate release checklist.
- Input handling has one front door: `src/app/input_resolver.rs`'s context-priority registry (keyboard) and the shared `Command`/`dispatch` seam (mouse). Add new shortcuts as `Command`s + bindings there, not as ad hoc key/click checks in view or panel code. Exceptions: text-entry contexts (search boxes, the save-name dialog) and external setup flows (e.g. login), which own local state. See CONTEXT.md's "Input handling" section and `docs/adr/0002-centralized-input-handling.md`.
- Treat local `main` as a read-only mirror of `origin/main`: only pull/fetch/reset it to match the remote. Do not commit on `main`, merge into `main`, rebase `main`, or use `main` as an integration branch.
- Do all code, docs, and workflow edits in isolated worktree branches created from `origin/main`. Push the branch and open a pull request when requested; never merge the branch back locally.
- Install repo-owned branch hygiene in each clone with `scripts/install-branch-hygiene.sh`. It activates tracked hooks that block commits/merges/rebases on `main` and auto-restore the root checkout to `main` if someone checks out a feature branch there. Use `scripts/reset-root-checkout.sh` when you want a destructive reset back to one clean `main` checkout with no local worktrees or feature branches.
- Never open a pull request unless explicitly asked to do so; don't run `gh pr create` unless requested.
- Documentation cleanup is agent-owned work in this repo: keep `CONTEXT.md`, `docs/adr/`, and related domain docs current as part of implementation changes, do not hand that cleanup back to the user, and do not leave doc edits sitting uncommitted at the end of the task.
- Before starting a new ADR, run `ls docs/adr/ | sort -t- -k1 -n | tail -1` against the target merge branch (not just your worktree) at plan-authoring time, and note the reserved number in the plan's header (`**ADR:** 00NN, reserved <date>`) so a concurrently-written sibling plan can grep for it. Two independently-authored plans claiming the same ADR number is a known failure mode here (#222/#223).


