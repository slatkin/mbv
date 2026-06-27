# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`mbv` is a terminal UI client for [Emby](https://emby.media) media servers. It embeds mpv for playback, syncs position with Emby, and supports full remote control via Emby's websocket API. Written in Rust with ratatui for the TUI.

## Rules
- Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.
- Use Serena for code exploration and targeted writes.
- Do not add `Co-Authored-By` trailers to commit messages.
- Ading debugging and conducting tests to get more information about an issue is preferred over staring at the code for extended periods of time trying to speculate what might be happening.
- No monolithic files. Make code files small and modular, and logically separated by function.
- Try to delegate simple code reads to subagents to avoid growing context in the main thread.
- Any live Emby API calls (curl, item lookups, endpoint research) must be done inside an `emby-research` subagent, not in the main thread.
- When a bug fix does not resolve the issue, do NOT suspect user error. Assume the fix is wrong or incomplete and investigate the code further.
- Use `cargo clippy` as the linter.
- See CHECKIN.md for pre-commit steps. 
- See RELEASE.md for the full release checklist.

### MediaItem gotchas

`MediaItem` (`api.rs`) is the universal item type. Emby-specific quirks baked into parsing:
- `production_year` — parsed from `ProductionYear` then `Year` (Emby uses `Year` for audio items)
- `is_folder` — forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc., regardless of Emby's `IsFolder` field
- `total_count` — from `ChildCount` (non-Series) or `RecursiveItemCount` (Series)

### Debugging Info

- read the log files directly (`tail`, `grep`) rather than asking the user to report what they see.

- `~/.local/state/mbv/queue_state.json` — current queue item IDs, cursor, last-played item, `QueueSource`. Updated immediately on every structural queue change, not just on quit.
- `~/.config/mbv/config.toml` — user config (parsed in `config.rs`).
- `~/.local/share/mbv/mbv.pid` — daemon PID file; checked by `daemon_running()` in `main.rs`.
- Lua script messages logged with `msg.warn(...)` appear in `mbv.log` as `source=mpv` lines.
- `~/.local/state/mbv/mbv.log` — main application log (Rust `log` crate output). Check this first when debugging.
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics.
- `~/.local/state/mbv/mbv.log.old` — previous session's log (rotated on startup).
