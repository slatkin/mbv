# app/ Module

All UI code. All files define `impl App` blocks — Rust allows splitting `impl` across files freely.

| File | Contents |
|------|----------|
| `mod.rs` | `App` struct def, `AppInit`/`build()` constructors, `run()` event loop, type/enum defs, tests |
| `input.rs` | `handle_key`, `handle_mouse`, all input dispatch |
| `actions.rs` | State-changing methods: navigation, playback, queue mgmt, `handle_lib_event`, `handle_ws_event` |
| `images.rs` | `fetch_card_image`, `render_card_slot`, `fetch_album_year`, `images_enabled`, `magick_resize` |
| `render/mod.rs` | `render()` main, `render_playback_controls`, divider indicators, volume bar |
| `render/library.rs` | `render_library`, `render_library_table`, `render_album_view`, `render_season_grid` |
| `render/home.rs` | `render_combined`, `render_home_panel`, `render_home_cards_section` |
| `render/playlist.rs` | `render_playlist_*` (list, filmstrip, cards, presentation views) |
| `render/overlays.rs` | Settings panel, playlists panel, sessions overlay, context menu, all modals |
| `render/log.rs` | Log tab rendering |
| `ui_util.rs` | Pure fns: `fmt_duration`, `natural_sort_key`, `item_text_and_style`, `trunc_str`, etc. |
| `settings.rs` | `setting_label`, `setting_value`, `settings_total_rows`, `settings_cursor_to_key` |
| `palette.rs` | All color constants (`FOAM` = Emby blue, `IRIS` = Emby green, etc.) |

See `docs/library-rendering.md` when touching `render/library.rs`, `images.rs`, or music album year logic.
See `docs/divider-indicators.md` when adding a new divider indicator.
