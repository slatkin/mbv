# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Rules

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

## Working in This Repo

**Explore**: Use the Explore subagent for any open-ended search — finding where something is defined, what calls a function, where a pattern is used. Only use Read/grep directly when the relevant file is already known.

## Code quality

Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.

## UI conventions

- **No emoji, ever.** Use plain Unicode geometric/box-drawing characters or ASCII only.

## Architecture

All code lives in `src/`. Single-binary app:

| Module | Role |
|--------|------|
| `main.rs` | Entry point: auth, daemon/remote mode selection, launches `App::run()` |
| `app/` | Everything UI — see below |
| `api.rs` | `EmbyClient` — all HTTP calls to Emby, `MediaItem` type, `parse_item()`, `fetch_items()` |
| `config.rs` | Config file parsing (`~/.config/mbv/config.toml`) and credential storage |
| `player.rs` | `Player` — wraps `libmpv2`, runs in its own thread, sends `PlayerEvent` back via mpsc |
| `ws.rs` | WebSocket thread: receives Emby remote-control messages, sends `WsEvent` to App |
| `daemon.rs` | Daemon mode: headless background player, exposes a Unix socket for `remote_player` |
| `remote_player.rs` | Client side of the daemon socket — lets a TUI instance drive a running daemon |
| `mpris.rs` | MPRIS2 D-Bus interface (media keys, playerctl) |
| `login.rs` | Full-screen TUI login flow |
| `applog.rs` | In-process log ring buffer displayed on the Log tab |
| `ctrl.rs` | Wire types (`CtrlCmd`, `CtrlEvent`, `CtrlState`) shared between daemon and remote_player |

### `src/app/` module tree

The `App` struct and all UI code live here. All modules define `impl App` blocks — Rust allows splitting `impl` across files freely. Methods are `pub(super)` when called by sibling modules, `pub(crate)` only when needed outside `app/`.

| File | Contents |
|------|----------|
| `mod.rs` | `App` struct definition, `AppInit`/`build()` constructors, `run()` event loop, type/enum definitions, tests |
| `input.rs` | `handle_key`, `handle_mouse`, and all input dispatch methods |
| `actions.rs` | State-changing methods: navigation, playback, queue management, `handle_lib_event`, `handle_ws_event` |
| `images.rs` | `fetch_card_image`, `render_card_slot`, `fetch_album_year`, `images_enabled`, `magick_resize` |
| `render/mod.rs` | `render()` main method, `render_playback_controls`, divider indicators, volume bar |
| `render/library.rs` | `render_library`, `render_library_table`, `render_album_view`, `render_season_grid` |
| `render/home.rs` | `render_combined`, `render_home_panel`, `render_home_cards_section` |
| `render/playlist.rs` | `render_playlist_*` (list, filmstrip, cards, presentation views) |
| `render/overlays.rs` | Settings panel, playlists panel, sessions overlay, context menu, all modals |
| `render/log.rs` | Log tab rendering |
| `ui_util.rs` | Pure functions: `fmt_duration`, `natural_sort_key`, `item_text_and_style`, `trunc_str`, etc. |
| `settings.rs` | `setting_label`, `setting_value`, `settings_total_rows`, `settings_cursor_to_key` |
| `palette.rs` | All color constants (`FOAM` = Emby blue, `IRIS` = Emby green, etc.) |

Background work (library fetching, image fetching, album year lookups) is always done via `std::thread::spawn`, results sent back to the main loop over mpsc channels (`lib_rx`, `player_rx`, `ws_rx`, `card_image_rx`). Nothing else is async except MPRIS (tokio, own thread).

### Constructors

`App::new()` and `App::new_remote()` both call the private `App::build(AppInit)` which holds all common field defaults. When adding a new field to `App`: add it to the struct, set its default in `build()`, and add it to `AppInit` only if the two constructors need different values for it.

### Key App state (non-obvious fields only)

- `queue_source: QueueSource` — tracks how the current queue was loaded (`Playlist`, `Album`, `Series`, `Shuffle`, `Remote`, `Collection { collection_type }`, `Unknown`). Persisted in `queue_state.json`. Set after every queue-replacing operation; cleared by `on_queue_replace_silent()`.
- `card_image_states: HashMap<String, Option<StatefulProtocol>>` — keyed by `"{item_id}:{slot}"` (e.g. `"abc123:lib"`, `"abc123:A"`).
- `image_protocol_enabled: bool` — cached from `client.config.image_protocol.is_some()`; updated in the settings handler when the user cycles the image protocol. Read via `images_enabled()`, not by checking the config directly.
- `music_levels: Vec<String>` — from config (e.g. `["group", "album"]`), maps nav stack depth to folder semantics. Drives `is_album_level()` / `is_viewing_album_folders()`.

### `api.rs` structure

Methods grouped by section comments (`// ── HTTP infrastructure ──`, `// ── Authentication ──`, `// ── Browse / fetch ──`, `// ── Library actions ──`, `// ── Playback reporting ──`, `// ── Playlists ──`, `// ── Series / episodes / chapters ──`, `// ── Remote session control ──`).

Browse methods returning `Vec<MediaItem>` from a paginated `{"Items": [...]}` response use `fetch_items(path, &[("Key", "value"), ...])`. Methods returning a top-level JSON array (`get_latest`, `get_ancestors`) or needing a `total_count` alongside results (`get_items_sorted`) don't use `fetch_items` — that's intentional, not an oversight.

**Sync requirement:** the language table in `parse_audio_info` must be kept in sync with `lang_code_to_name()` in `player.rs` — they map the same ISO 639-1/2 codes to the same English names. Nothing enforces this at compile time; if you touch one, check the other.

### MediaItem gotchas

`MediaItem` (`api.rs`) is the universal item type. Emby-specific quirks baked into parsing:
- `production_year` — parsed from `ProductionYear` then `Year` (Emby uses `Year` for audio items)
- `is_folder` — forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc., regardless of Emby's `IsFolder` field
- `total_count` — from `ChildCount` (non-Series) or `RecursiveItemCount` (Series)

### Library rendering, images, music

See [docs/library-rendering.md](docs/library-rendering.md) when touching `render/library.rs`, `images.rs`, or music album year logic.

### Playback sync

`Player` (`player.rs`) wraps libmpv2, receives `PlayerCommand` via mpsc, sends `PlayerEvent` back. The App handles `PlayerEvent` to update played status, advance the queue, and report progress to Emby (`report_start`, `report_progress_ws`, `report_stopped`). Ticks use Emby's `TICKS_PER_SECOND = 10_000_000`.

See [docs/player-internals.md](docs/player-internals.md) for session structs (`SingleSession`, `PlaylistSession`, `SessionReporter`) and critical invariants when touching `player.rs`.

### Daemon mode

`mbv -d` spawns `mbv --daemon-inner` as a detached process. The inner daemon runs `daemon::run()`, holds a `Player`, exposes a Unix socket at `$XDG_RUNTIME_DIR/mbv-ctl.sock`. A subsequent TUI invocation that detects the daemon connects via `remote_player::RemotePlayer` instead of creating its own `Player`. Running `mbv -d` when a daemon is already running exits with an error.

### Divider status indicators

The tab-bar divider line renders right-aligned bracketed indicators in `render/mod.rs`. See [docs/divider-indicators.md](docs/divider-indicators.md) for the step-by-step recipe when adding a new indicator.

### Persistent state files

- `~/.local/state/mbv/queue_state.json` — current queue item IDs, cursor, last-played item, `QueueSource`. Updated immediately on every structural queue change, not just on quit.
- `~/.config/mbv/config.toml` — user config (parsed in `config.rs`).
- `~/.local/share/mbv/mbv.pid` — daemon PID file; checked by `daemon_running()` in `main.rs`.

### Log files

- `~/.local/state/mbv/mbv.log` — main application log (Rust `log` crate output). Check this first when debugging.
- `~/.local/state/mbv/player-diag.log` — mpv/player diagnostics.
- `~/.local/state/mbv/mbv.log.old` — previous session's log (rotated on startup).

**When debugging issues**: read the log files directly (`tail`, `grep`) rather than asking the user to report what they see. Lua script messages logged with `msg.warn(...)` appear in `mbv.log` as `source=mpv` lines.

### Lua script deployment for `cargo run`

`osc_script_path()` checks `~/.local/share/mbv/scripts/mbv.lua` first — this installed copy shadows the source file. When debugging Lua changes with `cargo run`, always copy after editing:

```sh
cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua
```

## Code Exploration Policy
Always use jCodemunch-MCP tools — never fall back to Read, Grep, Glob, or Bash for code exploration.
- Before reading a file: use get_file_outline or get_file_content
- Before searching: use search_symbols or search_text
- Before exploring structure: use get_file_tree or get_repo_outline
- Call resolve_repo with the current directory first; if not indexed, call index_folder.
