
- read the log files directly (`tail`, `grep`) rather than asking the user to report what they see.
- `~/.local/state/mbv/queue_state.json` — current queue item IDs, cursor, last-played item, `QueueSource`. Updated immediately on every structural queue change, not just on quit.
- `~/.config/mbv/config.toml` — user config (parsed in `config.rs`).
- `~/.local/share/mbv/mbv.pid` — daemon PID file; checked by `daemon_running()` in `main.rs`.
- Lua script messages logged with `msg.warn(...)` appear in `mbv.log` as `source=mpv` lines.
- `~/.local/state/mbv/mbv.log` — main application log (Rust `log` crate output). Check this first when debugging.
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics.
- `~/.local/state/mbv/mbv.log.old` — previous session's log (rotated on startup).
