# mbv

Rust terminal media client for Emby, Audiobookshelf, and Feeds. It embeds mpv;
playback belongs to the terminal, Local daemon, or packaged `mbvd`.

## Read first

- `CONTEXT.md`: domain vocabulary; *Avoid* means incorrect terminology.
- Current (not superseded) ADRs in `docs/adr/` before architecture changes.
- `openspec` contains current and archived specs for major implementations.

## Boundaries

- `src/`: interactive binary and TUI; `src/local_daemon.rs` bootstraps the
  user-owned Local daemon without Remote Service authentication (ADR 0018).
- `crates/mbv-core/`: Service/runtime, provider APIs, config, ctrl/shared
  protocols, queue, source preparation, and mpv projection. No UI/feed fetch.
- `crates/mbvd/`: separately packaged daemon, persistent state, and sockets.
- Change source-of-truth types before callers.

## Commands

Prefix every command in a chain with `rtk`; use `rtk proxy <cmd>` only to
bypass filtering. `rtk grep` format flags (`-c/-l/-L/-o/-Z`) run raw.

- Check: `rtk cargo check -p <package>`
- Test: `rtk cargo nextest run -p <package>` (prefer nextest over `cargo test`)
- Lint: `rtk cargo clippy --workspace --all-targets`
- File-size check: `rtk make check-code-file-lines`

## Constraints

- Source files cap at 800 lines; split over-limit files in the same PR.
- Ctrl changes add capability strings, never bump protocol version; see the
  rule above `CTRL_PROTOCOL_VERSION`.
- Replace flaky tests; do not troubleshoot them.
- Put symbol-specific rules above the symbol.

## Project process

- Design changes live in `openspec/changes/<name>/`; otherwise use GitHub
  Issues/discussions (`slatkin/mbv`) and gists for ad-hoc notes.
- Commit specs/plans/docs with code; merge applied deltas into `openspec/specs/`
  and archive completed changes.

## Tool routing

- For code discovery or impact, use JCodeMunch `plan_turn`; use JCodeMunch for
  source retrieval, references, and impact. Honor negative evidence and
  `budget_warning`; do not use native Read/Grep/Glob/Bash for exploration.
- Use ast-grep only for a concrete AST predicate, preferably scoped to known
  files. It does not replace reference or impact analysis. Exclude duplicate
  `.worktrees/` and `.opencode/` trees from manual/structural scans.
- Use Serena for code edits only when the OpenCode session began in the target
  worktree. Never create/switch worktrees mid-session or edit a child worktree
  from a parent session. Before its first edit, call
  `serena_initial_instructions` and `serena_get_current_config`; proceed only
  when the active project is that worktree.
- Prefer Serena symbol edits/renames/deletes; use normal edits for non-code,
  unresolved symbols, or a Serena worktree mismatch.
- After source edits, call JCodeMunch `register_edit` unless a hook reindexed
  the files.
