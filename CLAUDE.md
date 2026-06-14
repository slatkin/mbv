# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`mbv` is a terminal UI client for [Emby](https://emby.media) media servers. It embeds mpv for playback, syncs position with Emby, and supports full remote control via Emby's websocket API. Written in Rust with ratatui for the TUI.

## Commands

```sh
cargo build --release      # release build
cargo build                # debug build (faster compile)
cargo test                 # run all tests
cargo test config          # run tests matching "config"
cargo test -- --nocapture  # see println! output in tests
make install               # build release + install to ~/.local/bin/mbv
```

There is no linter configured; `cargo build` catches everything relevant.

## Architecture

All code lives in `src/`. The app is single-binary with these modules:

| Module | Role |
|--------|------|
| `main.rs` | Entry point: auth, daemon/remote mode selection, launches `App::run()` |
| `app.rs` | Everything UI — the `App` struct, event loop, all rendering, all input handling |
| `api.rs` | `EmbyClient` — all HTTP calls to Emby, `MediaItem` type, `parse_item()` |
| `config.rs` | Config file parsing (`~/.config/mbv/config.toml`) and credential storage |
| `player.rs` | `Player` — wraps `libmpv2`, runs in its own thread, sends `PlayerEvent` back via mpsc |
| `ws.rs` | WebSocket thread: receives Emby remote-control messages, sends `WsEvent` to App |
| `daemon.rs` | Daemon mode: headless background player, exposes a Unix socket for `remote_player` |
| `remote_player.rs` | Client side of the daemon socket — lets a TUI instance drive a running daemon |
| `mpris.rs` | MPRIS2 D-Bus interface (media keys, playerctl) |
| `login.rs` | Full-screen TUI login flow |
| `applog.rs` | In-process log ring buffer displayed on the Log tab |

### Event loop (`app.rs`)

`App::run()` is a standard ratatui loop. Each iteration:
1. Drain `lib_rx` (background library loads via `LibEvent`)
2. Drain `player_rx` (`PlayerEvent` from mpv thread)
3. Drain `ws_rx` (`WsEvent` from websocket thread)
4. Drain `card_image_rx` (decoded image bytes from image fetch threads)
5. Handle `crossterm` keyboard/mouse events
6. Render frame

Background work (library fetching, image fetching, album year lookups) is always done by spawning `std::thread::spawn` and sending results back via mpsc channels to the main loop. Nothing async except MPRIS (which uses tokio in its own thread).

### Key App state

- `libs: Vec<LibraryTab>` — one entry per Emby library visible in the tab bar. Each has a `nav_stack: Vec<BrowseLevel>` (browsing history) and optional `search: LibSearch`.
- `player_tab: PlayerTab` — the queue (items + playlist cursor).
- `home: HomePane` — continue-watching + latest rows.
- `card_image_states: HashMap<String, Option<StatefulProtocol>>` — decoded images keyed by `"{item_id}:{slot}"` (e.g. `"abc123:lib"`, `"abc123:A"`).
- `music_levels: Vec<String>` — from config, describes the music folder layout (e.g. `["group", "album"]`). Drives `is_album_level()` / `is_viewing_album_folders()`.

### MediaItem

`MediaItem` (`api.rs`) is the universal item type used everywhere. Key fields for music:
- `production_year` — parsed from `ProductionYear` then `Year` JSON fields (Emby uses `Year` for audio items)
- `album_id` / `album` / `artist` — populated for audio tracks
- `is_folder` — forced `true` for `MusicAlbum`, `MusicArtist`, `Series`, etc. regardless of Emby's `IsFolder` field
- `total_count` — from `ChildCount` (non-Series) or `RecursiveItemCount` (Series)

### Library rendering

`render_library()` → checks special cases → dispatches to:
- `render_album_view()` — when `is_album_level()` is true (inside a music album). Presentation-style: left card panel + right track list.
- `render_season_grid()` — when the current level contains only Season items.
- `render_library_table()` — the standard row-per-item view for everything else.

In `render_library_table`, album folder rows (`is_album_folder = at_album_folders && item.is_folder`) get special treatment: 3-line height, always fetch/show album art, background year fetch via `fetch_album_year()`.

### Image handling

Images are fetched by `fetch_card_image()` which spawns a thread, downloads bytes, and sends them to `card_image_rx`. The special `"AudioChild"` image type fetches the first Audio child of an album folder then grabs its Primary image (workaround for Emby's image API for MusicAlbum items). Images are enabled only when `config.image_protocol` is `Some(_)`; the value controls the ratatui-image protocol (`kitty`, `sixel`, `halfblocks`, etc.).

### Music library specifics

- `music_levels` (e.g. `["group", "album"]`) maps nav stack depth to folder semantics.
- `is_album_level(lib_idx)` — true when you're inside an album (looking at tracks). Triggers `render_album_view` and auto-enqueue-album-on-play behavior.
- `is_viewing_album_folders(lib_idx)` — true when you're at the album list level. Triggers the expanded album row display with art and year.
- `album_year_cache: HashMap<String, u32>` — lazily populated by `fetch_album_year()` which fetches the first Audio child of an album to read its year (Emby doesn't always propagate year to MusicAlbum container items).

### Playback sync

`Player` (player.rs) wraps libmpv2. It receives `PlayerCommand` via mpsc and sends `PlayerEvent` back. The App handles `PlayerEvent` to update played status, advance the queue, and report progress to Emby via `api.rs` (`report_start`, `report_progress_ws`, `report_stopped`). Ticks use Emby's `TICKS_PER_SECOND = 10_000_000`.

### Daemon mode

`mbv -d` spawns `mbv --daemon-inner` as a detached process. The inner daemon runs `daemon::run()` which holds a `Player` and exposes a Unix socket at `$XDG_RUNTIME_DIR/mbv-ctl.sock`. A subsequent TUI invocation that detects the daemon connects via `remote_player::RemotePlayer` instead of creating its own `Player`.
