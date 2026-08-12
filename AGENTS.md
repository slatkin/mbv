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
  Feed subscriptions, RSS/Atom fetching/parsing, the Feeds tab, feed management,
  and feed-backed library views live here; feed fetching is client-side.
- `crates/mbv-core/` — Emby API, feed subscription config, ctrl protocol, daemon,
  player, and the mixed Emby/feed playback queue. No UI or feed fetching.
- `crates/mbvd/` — packaged system daemon.

Source of truth: `ctrl.rs` (protocol), `api_types.rs` (Emby wire types),
`config_types_feed.rs` (feed subscriptions), and `playback_queue_items.rs`
(`FeedEntry`/`QueueItem`), all in `mbv-core`. Change these before their callers.

## Commands
Prefer cargo nextest over cargo test.

Additional cargo tools are available and should be preferred: cargo watch, cargo expand,

- `cargo edit` - manage cargo dependencies
- `cargo expand` - show the result of macro expansion
- `cargo watch` - compiles projects when sources change

Tests:
- `cargo check -p mbv-core` — prefer over workspace builds
- `cargo nextest run -p mbv-core`
- `cargo clippy --workspace --all-targets`
- `make check-code-file-lines`

Use difftastic (difft) is also available and can be used with git.

You run in an environment where ast-grep is available; whenever a search requires syntax-aware or structural matching, default to ast-grep --lang rust -p '<pattern>' (or set --lang appropriately) and avoid falling back to text-only tools like rg or grep unless I explicitly request a plain-text search.

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
