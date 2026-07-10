`mbv` is a terminal UI client for [Emby](https://emby.media) media servers. It embeds mpv for playback, syncs position with Emby, and supports full remote control via Emby's websocket API. Written in Rust with ratatui for the TUI.

## Rules
- Use Serena if installed for code exploration and targeted writes.
- If creating or working in a git worktree, read docs/WORKTREES.md first.
- Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.
- Adding debugging and conducting tests to get more information about an issue is preferred. See docs/DEBUG.md for for troubleshooting strategies.
- Any live Emby API calls (curl, item lookups, endpoint research) must be done through an `emby-research` subagent, not in the main thread.
- See docs/CHECKIN.md for pre-commit steps.
- For releases, run `scripts/release.sh X.Y.Z "one-line summary"` instead of reading a separate release checklist.
- Always commit any edited `.md` file immediately after editing it — don't batch documentation edits with unrelated code changes or leave them uncommitted.
- Input handling has one front door: `src/app/input_resolver.rs`'s context-priority registry (keyboard) and the shared `Command`/`dispatch` seam (mouse). Add new shortcuts as `Command`s + bindings there, not as ad hoc key/click checks in view or panel code. Exceptions: text-entry contexts (search boxes, the save-name dialog) and external setup flows (e.g. login), which own local state. See CONTEXT.md's "Input handling" section and `docs/adr/0002-centralized-input-handling.md`.
- This is a single-user repo — never open a pull request unless explicitly asked to do so. Push commits directly to the branch (or `main`) as instructed; don't run `gh pr create` unless requested.

## Agent skills

### Issue tracker

Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the default label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **mbv** (2973 symbols, 13060 relationships, 263 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

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
