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

---

# Repo Rules
- Use Serena if installed for code exploration and targeted writes.
- For releases, run `scripts/release.sh X.Y.Z "one-line summary"` instead of reading a separate release checklist.
- Input handling has one front door: `src/app/input_resolver.rs`'s context-priority registry (keyboard) and the shared `Command`/`dispatch` seam (mouse). Add new shortcuts as `Command`s + bindings there, not as ad hoc key/click checks in view or panel code. Exceptions: text-entry contexts (search boxes, the save-name dialog) and external setup flows (e.g. login), which own local state. See CONTEXT.md's "Input handling" section and `docs/adr/0002-centralized-input-handling.md`.
- Treat local `main` as a read-only mirror of `origin/main`: only pull/fetch/reset it to match the remote. Do not commit on `main`, merge into `main`, rebase `main`, or use `main` as an integration branch.
- Do all code, docs, and workflow edits in isolated worktree branches created from `origin/main`. Push the branch and open a pull request when requested; never merge the branch back locally.
- Never open a pull request unless explicitly asked to do so; don't run `gh pr create` unless requested.

## Issue tracker

Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

## Triage labels

Uses the default label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `test`, `wontfix`). See `docs/agents/triage-labels.md`.

## Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

- Documentation cleanup is agent-owned work in this repo: keep `CONTEXT.md`, `docs/adr/`, and related domain docs current as part of implementation changes, do not hand that cleanup back to the user, and do not leave doc edits sitting uncommitted at the end of the task.
- Before starting a new ADR, run `ls docs/adr/ | sort -t- -k1 -n | tail -1` against the target merge branch (not just your worktree) at plan-authoring time, and note the reserved number in the plan's header (`**ADR:** 00NN, reserved <date>`) so a concurrently-written sibling plan can grep for it. Two independently-authored plans claiming the same ADR number is a known failure mode here (#222/#223).

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **mbv** (3479 symbols, 14656 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol, except for small ad-hoc UI changes.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user. For small ad-hoc UI polish requested interactively, GitNexus is optional; use judgment and prefer direct code inspection plus focused UI/render checks.
- **MUST run `detect_changes()` before committing, except for small ad-hoc UI changes.** Use it to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit non-trivial or non-UI-polish changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/mbv/context` | Codebase overview, check index freshness |
| `gitnexus://repo/mbv/clusters` | All functional areas |
| `gitnexus://repo/mbv/processes` | All execution flows |
| `gitnexus://repo/mbv/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
