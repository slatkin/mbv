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
```

Use `cargo clippy` as the linter. See CHECKIN.md for pre-commit steps.

**Before committing or pushing: always ask the user for permission first** (CHECKIN.md requirement).

## Releasing

1. Follow CHECKIN.md steps
2. Bump `version` in `Cargo.toml`
3. `cargo build` to update `Cargo.lock`
4. Commit: `Release X.Y.Z: <one-line summary>`
5. Push — a GitHub Action automatically updates the PKGBUILD sha256
6. Monitor CI for green

## Code quality

Always fix compile warnings — delete unused code rather than suppressing with `#[allow]`.

## UI conventions

- **No emoji, ever.** This is a terminal UI. Emoji render inconsistently across fonts and terminals. Use plain Unicode geometric/box-drawing characters or ASCII only.

## Architecture

All code lives in `src/`. The app is single-binary with these modules:

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

The `App` struct and all UI code live here. All modules in this tree define `impl App` blocks — Rust allows splitting `impl` across files freely. Methods are `pub(super)` when called by sibling modules (e.g. render calling actions), `pub(crate)` only when needed outside `app/`.

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

### Event loop (`app/mod.rs`)

`App::run()` is a standard ratatui loop. Each iteration:
1. Drain `lib_rx` (background library loads via `LibEvent`)
2. Drain `player_rx` (`PlayerEvent` from mpv thread)
3. Drain `ws_rx` (`WsEvent` from websocket thread)
4. Drain `card_image_rx` (decoded image bytes from image fetch threads)
5. Handle `crossterm` keyboard/mouse events
6. Render frame

Background work (library fetching, image fetching, album year lookups) is always done by spawning `std::thread::spawn` and sending results back via mpsc channels to the main loop. Nothing async except MPRIS (which uses tokio in its own thread).

### Constructors

`App::new()` and `App::new_remote()` both call the private `App::build(AppInit)` which holds all the common field defaults. When adding a new field to `App`: add it to `App` struct, set its default in `build()`, and add it to `AppInit` only if the two constructors need different values for it.

### Key App state

- `libs: Vec<LibraryTab>` — one entry per Emby library visible in the tab bar. Each has a `nav_stack: Vec<BrowseLevel>` (browsing history) and optional `search: LibSearch`.
- `player_tab: PlayerTab` — the queue (items + playlist cursor).
- `queue_source: QueueSource` — enum tracking how the current queue was loaded (`Playlist`, `Album`, `Series`, `Shuffle`, `Remote`, `Collection { collection_type }`, `Unknown`). Persisted in `queue_state.json`. Set after every queue-replacing operation; cleared by `on_queue_replace_silent()`.
- `queue_restored: bool` — true when the queue was loaded from `queue_state.json` on startup rather than built interactively.
- `home: HomePane` — continue-watching + latest rows.
- `card_image_states: HashMap<String, Option<StatefulProtocol>>` — decoded images keyed by `"{item_id}:{slot}"` (e.g. `"abc123:lib"`, `"abc123:A"`).
- `music_levels: Vec<String>` — from config, describes the music folder layout (e.g. `["group", "album"]`). Drives `is_album_level()` / `is_viewing_album_folders()`.
- `image_protocol_enabled: bool` — cached from `client.config.image_protocol.is_some()`; updated in the settings handler when the user cycles the image protocol. Use `images_enabled()` to read it.
- `use_nerd_fonts: bool` — from config; when true, single-glyph Nerd Font code points are used in place of ASCII fallbacks (e.g. in the divider status indicators).

### `api.rs` structure

`EmbyClient` methods are grouped by section comments (`// ── HTTP infrastructure ──`, `// ── Authentication ──`, `// ── Browse / fetch ──`, `// ── Library actions ──`, `// ── Playback reporting ──`, `// ── Playlists ──`, `// ── Series / episodes / chapters ──`, `// ── Remote session control ──`).

Browse methods that return `Vec<MediaItem>` from a paginated `{"Items": [...]}` response use `fetch_items(path, &[("Key", "value"), ...])`. Methods that return a top-level JSON array (`get_latest`, `get_ancestors`) or need a `total_count` alongside results (`get_items_sorted`) don't use `fetch_items` — that's intentional.

`parse_item()` delegates video/audio stream formatting to `parse_video_info(&[Value])` and `parse_audio_info(&[Value])`. The language table in `parse_audio_info` must be kept in sync with `lang_code_to_name()` in player.rs — they map the same ISO 639-1/2 codes to the same English names.

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

Images are fetched by `fetch_card_image()` which spawns a thread, downloads bytes, and sends them to `card_image_rx`. The special `"AudioChild"` image type fetches the first Audio child of an album folder then grabs its Primary image (workaround for Emby's image API for MusicAlbum items). Images are enabled only when `config.image_protocol` is `Some(_)`; check via `self.images_enabled()` (backed by the `image_protocol_enabled` field, not a mutex lock).

`magick_resize` (in `images.rs`) tries `magick convert` then falls back to `convert` — each via a for-loop with `let Ok(...) else { continue }`, not `?`, so failure of the first command tries the second.

### Music library specifics

- `music_levels` (e.g. `["group", "album"]`) maps nav stack depth to folder semantics.
- `is_album_level(lib_idx)` — true when you're inside an album (looking at tracks). Triggers `render_album_view` and auto-enqueue-album-on-play behavior.
- `is_viewing_album_folders(lib_idx)` — true when you're at the album list level. Triggers the expanded album row display with art and year.
- `album_year_cache: HashMap<String, u32>` — lazily populated by `fetch_album_year()` which fetches the first Audio child of an album to read its year (Emby doesn't always propagate year to MusicAlbum container items).

### Playback sync

`Player` (player.rs) wraps libmpv2. It receives `PlayerCommand` via mpsc and sends `PlayerEvent` back. The App handles `PlayerEvent` to update played status, advance the queue, and report progress to Emby via `api.rs` (`report_start`, `report_progress_ws`, `report_stopped`). Ticks use Emby's `TICKS_PER_SECOND = 10_000_000`.

#### player.rs internals

`play()` and `play_playlist()` are thin setup functions (~50 lines each). The real logic lives in session structs that own all loop state:

- **`SingleSession`** — handles a single-file playback session. Owns `quit_at`, `stop_reported`, `pending_load` (bool), intro state, next-up threshold, etc. Key methods: `handle_command`, `on_time_pos`, `on_playback_restart`, `on_end_file`, `on_shutdown`, `run`.
- **`PlaylistSession`** — handles a multi-file mpv playlist. Same shape but with `items: Vec<MediaItem>`, `current_idx`, `forced_idx`, `pending_load: u8` (counts in-flight EndFile events from ReplacePlaylist / initial jump), `stop_reported`, etc.
- **`SessionReporter`** — cloneable struct shared between the event-loop thread and the progress reporter thread. Holds `ids: Arc<Mutex<(item_id, msid, sid)>>` (a single lock for all three so transitions are never torn), `is_audio: Arc<AtomicBool>`, and `status: Arc<Mutex<PlayerStatus>>`. Key methods: `report_progress`, `report_stopped` (zeroes position for audio), `report_ping`, `start_item`, `transition_to`.
- **`ProgressGuard`** — owns the background progress-reporter thread. `stop_and_join()` signals it and waits.
- **`MpvSessionConfig`** — plain struct carrying headless/script/intro flags into the session.

**Critical invariant — `pending_load` in `PlaylistSession`**: always assign with `=`, never `+=` in `ReplacePlaylist`. The count must exactly equal the EndFiles mpv will emit for that operation (1, or 2 if `start_idx > 0`). When `pending_load` drains to 0 in `on_end_file`, `stop_reported` is reset to `false` for the new item.

**Critical invariant — `SessionReporter.ids`**: all three of `item_id`, `msid`, `sid` are updated atomically inside a single `Mutex::lock` in `start_item`. Never add separate per-field locks — the progress thread reads all three in one lock acquisition to avoid sending torn (new item_id, old sid) reports to Emby.

`effective_playback_state()` returns `(active, active_idx, pos_ticks, runtime_ticks, is_paused)` — for remote sessions it extrapolates position forward from the last-polled value, but only when `!remote.is_paused`.

### Daemon mode

`mbv -d` spawns `mbv --daemon-inner` as a detached process. The inner daemon runs `daemon::run()` which holds a `Player` and exposes a Unix socket at `$XDG_RUNTIME_DIR/mbv-ctl.sock`. A subsequent TUI invocation that detects the daemon connects via `remote_player::RemotePlayer` instead of creating its own `Player`. Running `mbv -d` when a daemon is already running exits with an error.

### Divider status indicators

The tab-bar divider line (`gap_area` in `render()`) renders right-aligned bracketed indicators: `[字]` subtitle, `[↯]` remote-control, `[>]/[||]/[ ]` playback. The rendering block is in `render/mod.rs` just below the `// Thin underline below tab row` comment.

To add a new indicator:

1. Compute `(text: &str, color: Color)` from whatever app state you need.
2. Add it to the `ind_w` sum in `dash_count`:
   ```rust
   let dash_count = gap_area.width.saturating_sub(ind_w(new_text) + ind_w(rc_text) + ...) as usize;
   ```
3. Insert the bracket/glyph/bracket spans in order (left-to-right = left-of-existing to right):
   ```rust
   Span::styled("[", bracket),
   Span::styled(new_text, Style::default().fg(new_color).add_modifier(Modifier::BOLD)),
   Span::styled("]", bracket),
   Span::styled("─", dash_style),
   ```

Rules:
- `ind_w(text)` = `1 + text.width() + 1 + 1` (`[` + display-width + `]` + `─`). Use `text.width()` (from `UnicodeWidthStr`) not `.chars().count()` — CJK and some nerd font glyphs are double-width.
- Brackets `[` `]` use the `bracket` style (white). The trailing `─` uses `dash_style` (muted). Never combine them into one span or the dash turns white.
- Nerd font glyphs go behind `if self.use_nerd_fonts { ... } else { ascii_fallback }`.
- The `dash_count` dashes fill the remaining width; the total of all `ind_w` values plus `dash_count` must equal `gap_area.width`.

### Persistent state files

- `~/.local/state/mbv/queue_state.json` — current queue item IDs, cursor, last-played item, and `QueueSource`. Updated immediately on every structural queue change (not just on quit).
- `~/.config/mbv/config.toml` — user config (parsed in `config.rs`).
- `~/.local/share/mbv/mbv.pid` — daemon PID file; checked by `daemon_running()` in `main.rs`.
