# Project: mbv

Terminal UI client for Emby, in Rust. Embeds mpv; playback runs in the terminal
process or in a background local daemon.

Paths move often. Verify before trusting one. Where this file and the code
disagree, the code wins — fix this file in the same PR.

## Read first
- `CONTEXT.md` — domain vocabulary. Its *Avoid* lists are wrong terms, not style.
- `docs/adr/` — decisions already made; check before changing architecture.
  Superseded ADRs carry a dated banner. No banner means current.

## Shape
- `src/` — binary. Entry, login, tray, mpris, single-instance, daemon supervision.
- `src/app/` — TUI state. Prefixes are the index: `input_*` keys and mouse,
  `*_actions.rs` state transitions, `render/` drawing, `*_tests.rs` its sibling.
- `crates/mbv-core/` — Emby API, ctrl protocol, daemon, player, queue. No UI.
- `crates/mbvd/` — packaged system daemon.

Source of truth: `ctrl.rs` (protocol), `api_types.rs` (Emby wire types), both in
`mbv-core`. Change these before their callers.

## Commands
Builds and tests run_in_background.
- `cargo check -p mbv-core` — prefer over workspace builds
- `cargo test -p mbv-core`
- `cargo clippy --workspace --all-targets`
- `make check-code-file-lines`

Prefix every command with `rtk`. It filters when it has a filter and passes
through unchanged when it doesn't, so it is always safe. Prefix each command in
a chain, not just the first. `rtk grep` with a format flag (`-c`, `-l`, `-L`,
`-o`, `-Z`) runs raw; `rtk proxy <cmd>` bypasses filtering.

## Constraints
- 800-line cap per source file, pre-commit enforced. Over the line means split
  it in the same PR.
- Ctrl protocol: additive changes get a capability string, not a version bump.
  Rule sits above `CTRL_PROTOCOL_VERSION` in `mbv-core/src/ctrl.rs`.
- Flaky test: delete it, write a new one. Don't troubleshoot.
- Symbol-specific rules go above the symbol, not here. Rules at the edit site
  can't rot unnoticed.

## Planning
- Design work: `openspec/changes/<name>/`. The one local exception to
  GitHub-first.
- Issues and discussion: GitHub Issues (slatkin/mbv), via `gh`. Ad-hoc notes:
  gists, not loose markdown.
- Specs, plans and docs commit with their code. Shipped plans get deleted —
  stale ones read as current intent.

## Workflow
- Gather → plan → execute. Past ~3 files, plan first and execute in fresh context.
- Delegate context-heavy exploration to a subagent; ingest only the summary.
- Search `src/ crates/ docs/`. `.worktrees/` and `.opencode/` hold duplicates
  and give false hits.
- After a PR from a worktree, switch back to main.
