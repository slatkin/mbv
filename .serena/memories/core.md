# mbv — Core

Terminal UI Emby client. Rust + ratatui + libmpv2. Single binary.

## Source map

| Path | Role |
|------|------|
| `src/main.rs` | Entry: auth, daemon/remote mode selection, launches `App::run()` |
| `src/app/mod.rs` | `App` struct, constructors (`build(AppInit)`), `run()` event loop |
| `src/app/input.rs` | `handle_key`, `handle_mouse`, input dispatch |
| `src/app/actions.rs` | State mutations: nav, playback, queue, `handle_lib_event`, `handle_ws_event` |
| `src/app/render/mod.rs` | `render()`, playback controls, divider indicators, volume bar |
| `src/app/render/library.rs` | Library table, album view, season grid |
| `src/app/render/home.rs` | Home/combined panel, home cards |
| `src/app/render/playlist.rs` | Playlist views (list, filmstrip, cards, presentation) |
| `src/app/render/overlays.rs` | Settings panel, playlists panel, sessions overlay, context menu, modals |
| `src/app/render/log.rs` | Log tab |
| `src/app/images.rs` | Image fetch, card rendering, album year fetch |
| `src/app/ui_util.rs` | Pure helpers: `fmt_duration`, `natural_sort_key`, `trunc_str`, etc. |
| `src/app/settings.rs` | Settings key/label/value helpers |
| `src/app/palette.rs` | Color constants (`FOAM`=Emby blue, `IRIS`=Emby green) |
| `src/api.rs` | `EmbyClient`, `MediaItem`, `parse_item()`, `fetch_items()` |
| `src/config.rs` | Config parsing (`~/.config/mbv/config.toml`) + credential storage |
| `src/player.rs` | `Player` wraps libmpv2; own thread; sends `PlayerEvent` via mpsc |
| `src/ws.rs` | WebSocket thread: Emby remote-control → `WsEvent` |
| `src/daemon.rs` | Headless background player, Unix socket at `$XDG_RUNTIME_DIR/mbv-ctl.sock` |
| `src/remote_player.rs` | Client side of daemon socket |
| `src/mpris.rs` | MPRIS2 D-Bus (media keys, playerctl) |
| `src/login.rs` | Full-screen TUI login flow |
| `src/applog.rs` | In-process log ring buffer (Log tab) |
| `src/ctrl.rs` | Wire types (`CtrlCmd`, `CtrlEvent`, `CtrlState`) for daemon↔remote_player |
| `scripts/mbv.lua` | mpv OSC Lua script; deploy to `~/.local/share/mbv/scripts/` for `cargo run` |

## Key invariants

- All `impl App` blocks split across files in `src/app/`; methods `pub(super)` by default.
- Background work via `std::thread::spawn`; results over mpsc channels (`lib_rx`, `player_rx`, `ws_rx`, `card_image_rx`).
- `App::new()` / `App::new_remote()` both call `App::build(AppInit)` for shared defaults.
- `card_image_states` keyed `"{item_id}:{slot}"`.
- `music_levels: Vec<String>` from config maps nav depth to folder semantics.
- Language table in `parse_audio_info` (`api.rs`) **must stay in sync** with `lang_code_to_name()` in `player.rs`.
- Playback ticks: `TICKS_PER_SECOND = 10_000_000`.

## Docs

- `docs/library-rendering.md` — touch before editing `render/library.rs`, `images.rs`, album year logic.
- `docs/player-internals.md` — touch before editing `player.rs` session structs.
- `docs/divider-indicators.md` — recipe for adding a new divider indicator.

## Persistent state

- `~/.local/state/mbv/queue_state.json` — queue item IDs, cursor, `QueueSource`
- `~/.config/mbv/config.toml` — user config
- `~/.local/share/mbv/mbv.pid` — daemon PID

## Logs

- `~/.local/state/mbv/mbv.log` — main log (check first when debugging)
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics
- Lua `msg.warn(...)` appears as `source=mpv` lines in mbv.log

Related: `mem:tech_stack`, `mem:conventions`, `mem:task_completion`, `mem:suggested_commands`
