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

## Agent skills

### Issue tracker

Issues live in GitHub Issues (slatkin/mbv), managed via the `gh` CLI. External PRs are not pulled into triage. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the default label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
