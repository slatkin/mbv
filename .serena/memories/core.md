# mbv — Core

Terminal UI Emby media-server client. Embeds mpv for playback, syncs position with Emby, full remote control via Emby WebSocket. Single Rust binary. **Emby only — never say Jellyfin.**

## Source map (`src/`)

| Path | Role |
|------|------|
| `main.rs` | Entry: auth, daemon/remote mode selection, launches `App::run()` |
| `api.rs` | `EmbyClient` + `MediaItem`; all HTTP; `parse_item()`, `fetch_items()` |
| `config.rs` | Config parsing (`~/.config/mbv/config.toml`), credential storage |
| `player.rs` | `Player` wraps libmpv2; own thread; sends `PlayerEvent` via mpsc |
| `ws.rs` | WebSocket thread → sends `WsEvent` to App |
| `daemon.rs` | Headless player mode; exposes Unix socket |
| `remote_player.rs` | Client side of daemon socket |
| `mpris.rs` | MPRIS2 D-Bus (tokio, own thread) |
| `login.rs` | Full-screen TUI login flow |
| `applog.rs` | In-process log ring buffer (Log tab) |
| `ctrl.rs` | Wire types: `CtrlCmd`, `CtrlEvent`, `CtrlState` |
| `app/` | All UI — see `mem:app_module` |

## Key invariants

- Background work: always `std::thread::spawn` + mpsc channels (`lib_rx`, `player_rx`, `ws_rx`, `card_image_rx`). Nothing async except MPRIS.
- Playback ticks: `TICKS_PER_SECOND = 10_000_000` (Emby format).
- `App::new()` and `App::new_remote()` both call `App::build(AppInit)` for shared defaults. New `App` fields: add to struct, set default in `build()`, add to `AppInit` only if constructors need different values.

## Persistent state files

- `~/.local/state/mbv/queue_state.json` — queue item IDs, cursor, last-played, `QueueSource`; updated on every structural queue change
- `~/.config/mbv/config.toml` — user config
- `~/.local/share/mbv/mbv.pid` — daemon PID

## Log files (check these first when debugging)

- `~/.local/state/mbv/mbv.log` — main log (Rust `log` crate); mpv Lua `msg.warn()` appears as `source=mpv` lines
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics
- `~/.local/state/mbv/mbv.log.old` — previous session

## Daemon mode

`mbv -d` spawns `mbv --daemon-inner` detached. Inner daemon runs `daemon::run()`, holds a `Player`, listens on `$XDG_RUNTIME_DIR/mbv-ctl.sock`. A TUI that detects the daemon connects via `RemotePlayer` instead of creating its own `Player`.

## Lua script

`osc_script_path()` checks `~/.local/share/mbv/scripts/mbv.lua` first — shadows source. When debugging with `cargo run`, copy after editing: `cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua`

## Further memories

- App module detail: `mem:app_module`
- api.rs structure/gotchas: `mem:api`
- Playback sync / player internals: `mem:player`
