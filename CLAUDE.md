# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Rules

## Code Exploration Policy
Always use jCodemunch-MCP tools — never fall back to Read, Grep, Glob, or Bash for code exploration.
- Before reading a file: use get_file_outline or get_file_content
- Before searching: use search_symbols or search_text
- Before exploring structure: use get_file_tree or get_repo_outline
- Call resolve_repo with the current directory first; if not indexed, call index_folder.

**USE SERENA AND JCODEMUNCH FOR CODE EXPLORATION** I mean please use these are they are meant to be used rather than grepping all over the code.

**NEVER ADD CO-AUTHORED-BY.** Do not add `Co-Authored-By` trailers to commit messages.

**NEVER GUESS.** Read the source before assuming anything. Use Grep, Glob, or Read first.

**DEBUG AND TROUBLESHOOT, DON'T SPIN YOUR WHEELS SPECULATING** Being direct and adding debugging and conducting tests to get more information about an issue is preferred over staring at the code for extended periods of time trying to speculate what might be happening.

**DO ONLY WHAT WAS ASKED.** No extra borders, styles, classes, or behaviours

**NO MONOLITHS** Make code files small and modular, and logically separated by function. No giant files of code that fills up Claude's context when trying to read and locate code.

**DELEGATE TO SUBAGENTS** Try to delegate simple code reads to subagents to avoid growing context in the main thread.

**EMBY API QUERIES GO IN SUBAGENTS** Any live Emby API calls (curl, item lookups, endpoint research) must be done inside an `emby-research` subagent, not in the main thread.

**DON'T BE A DICK** When a bug fix does not resolve the issue, do NOT suspect user error. Assume the fix is wrong or incomplete and investigate the code further.

## What this is

`mbv` is a terminal UI client for [Emby](https://emby.media) media servers. It embeds mpv for playback, syncs position with Emby, and supports full remote control via Emby's websocket API. Written in Rust with ratatui for the TUI.

## Commands

```sh
cargo build --release      # release build
cargo build                # debug build (faster compile)
cargo test                 # run all tests
cargo test config          # run tests matching "config"
cargo test -- --nocapture  # see println! output in tests
```

Use `cargo clippy` as the linter. See CHECKIN.md for pre-commit steps.

**Before committing or pushing: always ask the user for permission first** (CHECKIN.md requirement).

## Releasing

See RELEASE.md for the full release checklist.

## Code quality

Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.

### MediaItem gotchas

`MediaItem` (`api.rs`) is the universal item type. Emby-specific quirks baked into parsing:
- `production_year` — parsed from `ProductionYear` then `Year` (Emby uses `Year` for audio items)
- `is_folder` — forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc., regardless of Emby's `IsFolder` field
- `total_count` — from `ChildCount` (non-Series) or `RecursiveItemCount` (Series)

### Persistent state files

- `~/.local/state/mbv/queue_state.json` — current queue item IDs, cursor, last-played item, `QueueSource`. Updated immediately on every structural queue change, not just on quit.
- `~/.config/mbv/config.toml` — user config (parsed in `config.rs`).
- `~/.local/share/mbv/mbv.pid` — daemon PID file; checked by `daemon_running()` in `main.rs`.

### Log files

- `~/.local/state/mbv/mbv.log` — main application log (Rust `log` crate output). Check this first when debugging.
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics.
- `~/.local/state/mbv/mbv.log.old` — previous session's log (rotated on startup).

**When debugging issues**: read the log files directly (`tail`, `grep`) rather than asking the user to report what they see. Lua script messages logged with `msg.warn(...)` appear in `mbv.log` as `source=mpv` lines.
