# Conventions

## General
- No emoji anywhere (UI or code).
- No `Co-Authored-By` in commits.
- No comments unless WHY is non-obvious (hidden constraint, workaround, subtle invariant). No docstrings/multi-line comment blocks.
- No `#[allow(...)]` to suppress warnings — delete unused code.
- Ask before committing or pushing.

## UI
- Plain Unicode geometric/box-drawing or ASCII only — no emoji.
- Color palette: all constants in `src/app/palette.rs`. Key: FOAM=Emby blue, IRIS=Emby green, YELLOW=in-progress/paused, PINE=dark green folders.

## Code structure
- Keep files small and modular. No monolithic files.
- `pub(super)` for methods called by sibling modules; `pub(crate)` only when needed outside `app/`.
- All `impl App` blocks can be split across files freely.
- New `App` field: add to struct in mod.rs → default in `build()` → `AppInit` only if the two constructors need different values.

## Input handling (input.rs) priority order
Context menu → F-keys → Alt+search-filter → home/queue search text input → lib search text input → confirm dialogs → playback keys → tab dispatch.
Each search input block guards: `&& self.context_menu.is_none()`.

## Search (home_search)
- Active on tab_idx 0 (Home) and tab_idx 1 (Queue).
- `HomeSearch` methods: `available_types()`, `filtered_results()`, `filtered_count()`.
- `current_home_item()` uses `filtered_results()`.
- When `home_search.is_some()`, context menu uses search result item (not queue cursor item).
- `NavigateTo` event clears `home_search`.

## Emby API quirks
- Ancestors endpoint: `GET /Items/{id}/Ancestors` (NOT `/Users/{uid}/Items/{id}/Ancestors`).
- CollectionFolder IDs are virtual — never appear in ancestor chains. Map items to libraries via `item_type` → `collection_type` (Series/Episode/Season→tvshows, Movie→movies, Audio/MusicAlbum/MusicArtist→music).
- `get_items_sorted(lib_id)` transparently traverses physical sub-folders.
- Sync: `parse_audio_info` language table ↔ `lang_code_to_name()` in player.rs.

## Divider indicators
Follow recipe in `docs/divider-indicators.md` when adding a new tab-bar indicator.

## Persistent state
- `~/.local/state/mbv/queue_state.json` — queue (updated on every structural change)
- `~/.config/mbv/config.toml` — user config
- `~/.local/share/mbv/mbv.pid` — daemon PID
