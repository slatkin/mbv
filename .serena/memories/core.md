# mbv — Emby TUI client

Single Rust binary. Embeds mpv for playback, syncs position with Emby, supports remote control via WebSocket.

## Source map

```
src/
  main.rs           — auth, daemon/remote mode selection, launches App::run()
  api.rs            — EmbyClient (all HTTP), MediaItem, parse_item(), fetch_items()
  config.rs         — ~/.config/mbv/config.toml parsing + credential storage
  player.rs         — Player wrapping libmpv2, own thread, sends PlayerEvent via mpsc
  ws.rs             — WebSocket thread → WsEvent to App
  daemon.rs         — headless player, Unix socket at $XDG_RUNTIME_DIR/mbv-ctl.sock
  remote_player.rs  — client side of daemon socket
  mpris.rs          — MPRIS2 D-Bus (tokio thread)
  login.rs          — full-screen TUI login flow
  applog.rs         — in-process log ring buffer (Log tab)
  ctrl.rs           — wire types (CtrlCmd, CtrlEvent, CtrlState)
  app/
    mod.rs          — App struct, AppInit/build(), run() event loop, enums, tests
    input.rs        — handle_key, handle_mouse, all input dispatch
    actions.rs      — state-changing methods, handle_lib_event, handle_ws_event
    images.rs       — card image fetch/render
    palette.rs      — all color constants
    settings.rs     — settings panel logic
    ui_util.rs      — pure helpers: fmt_duration, natural_sort_key, trunc_str, …
    render/
      mod.rs        — render() entry, playback controls, divider indicators, volume bar
      home.rs       — render_combined, render_home_panel, render_home_search
      library.rs    — render_library, render_album_view, render_season_grid
      playlist.rs   — render_playlist_* (list, filmstrip, cards, presentation, power)
      overlays.rs   — settings, playlists, sessions, context menu, modals
      log.rs        — log tab
```

## Key invariants

- All `impl App` blocks spread across files; methods are `pub(super)` for siblings, `pub(crate)` only if needed outside `app/`.
- Background work via `std::thread::spawn` + mpsc channels (`lib_rx`, `player_rx`, `ws_rx`, `card_image_rx`, `search_rx`).
- MPRIS runs on tokio; everything else is sync.
- `App::build(AppInit)` holds all field defaults. New fields: add to struct → default in `build()` → `AppInit` only if constructors differ.
- `parse_item()` in api.rs: `is_folder` forced true for Series/Season/MusicAlbum/MusicArtist/etc. regardless of Emby's IsFolder. `production_year` falls back to `Year` for audio items.
- Emby uses numeric item IDs (not GUIDs).
- CollectionFolder IDs (library roots) are VIRTUAL — they never appear in `/Items/{id}/Ancestors` responses. Ancestors traverse the physical folder tree (AggregateFolder root id=2 at top). Match items to libraries by `collection_type`, not by ancestor ID.
- Ancestors endpoint: `GET /Items/{itemId}/Ancestors` (no `/Users/{userId}/` prefix).
- Language table in `parse_audio_info` (api.rs) must stay in sync with `lang_code_to_name()` (player.rs).

See `mem:conventions`, `mem:tech_stack`, `mem:suggested_commands`, `mem:task_completion`.
Docs in `docs/`: `mem:docs/library-rendering`, `mem:docs/player-internals`, `mem:docs/divider-indicators`.
