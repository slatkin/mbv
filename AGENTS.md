`mbv` is a terminal UI client for [Emby](https://emby.media) media servers. It embeds mpv for playback, syncs position with Emby, and supports full remote control via Emby's websocket API. Written in Rust with ratatui for the TUI.

Relevant information about the project is in .serena/memories. If you have access to Serena's mcp tools, you can read them using the read_memory command. Otherwise you can just read them using normal file reading tools.

## Rules
- Use Serena for code exploration and targeted writes.
- If creating or working in a git worktree, read docs/WORKTREES.md first.
- Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.
- Adding debugging and conducting tests to get more information about an issue is preferred over staring at the code for extended periods of time trying to speculate what might be happening. See docs/DEBUG.md for for troubleshooting strategies.
- Any live Emby API calls (curl, item lookups, endpoint research) must be done inside an `emby-research` subagent, not in the main thread.
- When a bug fix does not resolve the issue, do NOT suspect user error. Assume the fix is wrong or incomplete and investigate the code further.
- See docs/CHECKIN.md for pre-commit steps.
- For releases, run `scripts/release.sh X.Y.Z "one-line summary"` instead of reading a separate release checklist.

## Agent skills

### Issue tracker

Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the default label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
